use {
    super::price_estimation::{
        self,
        native::NativePriceEstimating,
        PriceEstimating,
        PriceEstimationError,
    },
    crate::{
        db_order_conversions::order_kind_from,
        fee_subsidy::{FeeParameters, FeeSubsidizing, Subsidy, SubsidyParameters},
        order_validation::{
            AppDataValidationError,
            OrderValidating,
            PartialValidationError,
            PreOrderData,
        },
        price_estimation::Verification,
        trade_finding,
    },
    anyhow::{Context, Result},
    chrono::{DateTime, Duration, TimeZone as _, Utc},
    database::quotes::{Quote as QuoteRow, QuoteKind},
    ethcontract::{H160, U256},
    futures::TryFutureExt as _,
    gas_estimation::GasPriceEstimating,
    model::{
        order::{OrderClass, OrderCreationAppData, OrderKind},
        quote::{
            OrderQuote,
            OrderQuoteRequest,
            OrderQuoteResponse,
            OrderQuoteSide,
            PriceQuality,
            QuoteId,
            QuoteSigningScheme,
            SellAmount,
        },
    },
    number::conversions::big_decimal_to_u256,
    std::sync::Arc,
    thiserror::Error,
};

/// A high-level interface for handling API quote requests.
pub struct QuoteHandler {
    order_validator: Arc<dyn OrderValidating>,
    optimal_quoter: Arc<dyn OrderQuoting>,
    fast_quoter: Arc<dyn OrderQuoting>,
}

impl QuoteHandler {
    pub fn new(order_validator: Arc<dyn OrderValidating>, quoter: Arc<dyn OrderQuoting>) -> Self {
        Self {
            order_validator,
            optimal_quoter: quoter.clone(),
            fast_quoter: quoter,
        }
    }

    pub fn with_fast_quoter(mut self, fast_quoter: Arc<dyn OrderQuoting>) -> Self {
        self.fast_quoter = fast_quoter;
        self
    }
}

impl QuoteHandler {
    pub async fn calculate_quote(
        &self,
        request: &OrderQuoteRequest,
    ) -> Result<OrderQuoteResponse, OrderQuoteError> {
        tracing::debug!(?request, "calculating quote");

        let app_data = self.order_validator.validate_app_data(
            &request.app_data,
            &None,
            request.partially_fillable,
        )?;

        let order = PreOrderData::from(request);
        let valid_to = order.valid_to;
        self.order_validator.partial_validate(order).await?;

        let params = {
            let verification = match request.price_quality {
                PriceQuality::Verified => Some(Verification {
                    from: request.from,
                    receiver: request.receiver.unwrap_or(request.from),
                    sell_token_source: request.sell_token_balance,
                    buy_token_destination: request.buy_token_balance,
                    pre_interactions: trade_finding::map_interactions(&app_data.interactions.pre),
                    post_interactions: trade_finding::map_interactions(&app_data.interactions.post),
                }),
                PriceQuality::Fast | PriceQuality::Optimal => None,
            };

            QuoteParameters {
                sell_token: request.sell_token,
                buy_token: request.buy_token,
                side: request.side,
                verification,
                signing_scheme: request.signing_scheme,
                additional_gas: app_data.inner.protocol.hooks.gas_limit(),
            }
        };

        let quote = match request.price_quality {
            PriceQuality::Optimal | PriceQuality::Verified => {
                let quote = self.optimal_quoter.calculate_quote(params).await?;
                self.optimal_quoter
                    .store_quote(quote)
                    .await
                    .map_err(CalculateQuoteError::Other)?
            }
            PriceQuality::Fast => {
                let mut quote = self.fast_quoter.calculate_quote(params).await?;
                // We maintain an API guarantee that fast quotes always have an expiry of zero,
                // because they're not very accurate and can be considered to
                // expire immediately.
                quote.data.expiration = Utc.timestamp_millis_opt(0).unwrap();
                quote
            }
        };

        let response = OrderQuoteResponse {
            quote: OrderQuote {
                sell_token: request.sell_token,
                buy_token: request.buy_token,
                receiver: request.receiver,
                sell_amount: quote.sell_amount,
                buy_amount: quote.buy_amount,
                valid_to,
                app_data: match &request.app_data {
                    OrderCreationAppData::Full { full } => OrderCreationAppData::Both {
                        full: full.clone(),
                        expected: request.app_data.hash(),
                    },
                    app_data => app_data.clone(),
                },
                fee_amount: quote.fee_amount,
                kind: quote.data.kind,
                partially_fillable: request.partially_fillable,
                sell_token_balance: request.sell_token_balance,
                buy_token_balance: request.buy_token_balance,
                signing_scheme: request.signing_scheme.into(),
            },
            from: request.from,
            expiration: quote.data.expiration,
            id: quote.id,
        };

        tracing::debug!(?response, "finished computing quote");
        Ok(response)
    }
}

/// Result from handling a quote request.
#[derive(Debug, Error)]
pub enum OrderQuoteError {
    #[error("error validating app data: {0:?}")]
    AppData(AppDataValidationError),

    #[error("error validating order data: {0:?}")]
    Order(PartialValidationError),

    #[error("error calculating quote: {0}")]
    CalculateQuote(#[from] CalculateQuoteError),
}

impl From<AppDataValidationError> for OrderQuoteError {
    fn from(err: AppDataValidationError) -> Self {
        Self::AppData(err)
    }
}

impl From<PartialValidationError> for OrderQuoteError {
    fn from(err: PartialValidationError) -> Self {
        Self::Order(err)
    }
}

/// Order parameters for quoting.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct QuoteParameters {
    pub sell_token: H160,
    pub buy_token: H160,
    pub side: OrderQuoteSide,
    pub verification: Option<Verification>,
    pub signing_scheme: QuoteSigningScheme,
    pub additional_gas: u64,
}

impl QuoteParameters {
    fn to_price_query(&self) -> price_estimation::Query {
        let (kind, in_amount) = match self.side {
            OrderQuoteSide::Sell {
                sell_amount:
                    SellAmount::BeforeFee { value: sell_amount }
                    | SellAmount::AfterFee { value: sell_amount },
            } => (OrderKind::Sell, sell_amount),
            OrderQuoteSide::Buy {
                buy_amount_after_fee,
            } => (OrderKind::Buy, buy_amount_after_fee),
        };

        price_estimation::Query {
            verification: self.verification.clone(),
            sell_token: self.sell_token,
            buy_token: self.buy_token,
            in_amount,
            kind,
        }
    }

    fn additional_cost(&self) -> u64 {
        self.signing_scheme
            .additional_gas_amount()
            .saturating_add(self.additional_gas)
    }
}

/// A calculated order quote.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Quote {
    pub id: Option<QuoteId>,
    pub data: QuoteData,
    /// The final computed sell amount for the quote.
    ///
    /// Note that this is different than the `QuoteData::quoted_sell_amount` for
    /// quotes computed with `SellAmount::BeforeFee` (specifically, it will be
    /// `quoted_sell_amount - fee_amount` in those cases).
    pub sell_amount: U256,
    /// The final computed buy amount for the quote.
    ///
    /// Note that this is different than the `QuoteData::quoted_buy_amount` for
    /// quotes computed with `SellAmount::BeforeFee` (specifically, it will be
    /// scaled down to account for the computed `fee_amount`).
    pub buy_amount: U256,
    /// The final minimum subsidized fee amount for any order created for this
    /// quote. The fee is denoted in the sell token.
    pub fee_amount: U256,
    /// The actual fee amount that is esimated to be required in order to settle
    /// the order on chain. This is the fee in full without any subsidies. The
    /// fee is denoted in the sell token.
    pub full_fee_amount: U256,
}

impl Quote {
    /// Creates a new `Quote`.
    pub fn new(id: Option<QuoteId>, data: QuoteData) -> Self {
        Self {
            id,
            sell_amount: data.quoted_sell_amount,
            buy_amount: data.quoted_buy_amount,
            fee_amount: data.fee_parameters.unsubsidized(),
            full_fee_amount: data.fee_parameters.unsubsidized(),
            data,
        }
    }

    /// Applies a subsidy to the quote **with** the
    /// `QuoteSigningScheme.verification_gas_limit`
    pub fn with_subsidy_and_additional_cost(
        mut self,
        subsidy: &Subsidy,
        additional_cost: u64,
    ) -> Self {
        // Be careful not to modify `self.data` as this represents the actual
        // quote data that is stored in the database. Instead, update the
        // computed fee fields.
        self.fee_amount = self
            .data
            .fee_parameters
            .subsidized_with_additional_cost(subsidy, additional_cost);
        self.full_fee_amount = self
            .data
            .fee_parameters
            .unsubsidized_with_additional_cost(additional_cost);

        self
    }

    /// Scales order buy amount to the specified sell amount.
    ///
    /// This allows scaling the quoted amounts to some lower sell amount
    /// accounting for fees (for quotes with sell amounts **before** fees for
    /// example).
    ///
    /// Since this method is indented for **scaling down** buy and sell amounts,
    /// it assumes that the final buy amount will never overflow a `U256` and
    /// _will saturate_ to `U256::MAX` if this is used to scale up past the
    /// maximum value.
    pub fn with_scaled_sell_amount(mut self, sell_amount: U256) -> Self {
        self.sell_amount = sell_amount;
        // Use `full_mul: (U256, U256) -> U512` to avoid any overflow
        // errors computing the initial product.
        self.buy_amount = (self.data.quoted_buy_amount.full_mul(sell_amount)
            / self.data.quoted_sell_amount)
            .try_into()
            .unwrap_or(U256::MAX);

        self
    }
}

/// Detailed data for a computed order quote.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct QuoteData {
    pub sell_token: H160,
    pub buy_token: H160,
    /// The sell amount used when computing the exchange rate for this quote.
    ///
    /// For buy orders, this will be the expected sell amount for some fixed
    /// buy amount. For sell order of the `SellAmount::BeforeFee` variant, this
    /// will be the total `sell_amount + fee_amount`. For sell orders of the
    /// `SellAmount::AfterFee` variant, this will be the fixed sell amount of
    /// the order used for price estimates.
    pub quoted_sell_amount: U256,
    /// The buy amount used when computing the exchange rate for this quote.
    ///
    /// For buy orders, this will be the fixed buy amount. For sell order of the
    /// `SellAmount::BeforeFee` variant, this will be the expected buy amount if
    /// `sell_amount + fee_amount` were traded. For sell orders of the
    /// `SellAmount::AfterFee` variant, this will be the expected sell amount if
    /// exactly `sell_amount` were traded.
    pub quoted_buy_amount: U256,
    pub fee_parameters: FeeParameters,
    pub kind: OrderKind,
    pub expiration: DateTime<Utc>,
    /// Since different quote kinds have different expirations,
    /// we need to store the quote kind to prevent missuse of quotes.
    pub quote_kind: QuoteKind,
    /// The address of the solver that provided the quote.
    pub solver: H160,
}

impl TryFrom<QuoteRow> for QuoteData {
    type Error = anyhow::Error;

    fn try_from(row: QuoteRow) -> Result<QuoteData> {
        Ok(QuoteData {
            sell_token: H160(row.sell_token.0),
            buy_token: H160(row.buy_token.0),
            quoted_sell_amount: big_decimal_to_u256(&row.sell_amount)
                .context("quoted sell amount is not a valid U256")?,
            quoted_buy_amount: big_decimal_to_u256(&row.buy_amount)
                .context("quoted buy amount is not a valid U256")?,
            fee_parameters: FeeParameters {
                gas_amount: row.gas_amount,
                gas_price: row.gas_price,
                sell_token_price: row.sell_token_price,
            },
            kind: order_kind_from(row.order_kind),
            expiration: row.expiration_timestamp,
            quote_kind: row.quote_kind,
            solver: H160(row.solver.0),
        })
    }
}

#[mockall::automock]
#[async_trait::async_trait]
pub trait OrderQuoting: Send + Sync {
    /// Computes a quote for the specified order parameters. Doesn't store the
    /// quote.
    async fn calculate_quote(
        &self,
        parameters: QuoteParameters,
    ) -> Result<Quote, CalculateQuoteError>;

    /// Stores a quote.
    async fn store_quote(&self, quote: Quote) -> Result<Quote>;

    /// Finds an existing quote.
    async fn find_quote(
        &self,
        id: Option<QuoteId>,
        parameters: QuoteSearchParameters,
    ) -> Result<Quote, FindQuoteError>;
}

#[derive(Error, Debug)]
pub enum CalculateQuoteError {
    #[error("sell amount does not cover fee")]
    SellAmountDoesNotCoverFee { fee_amount: U256 },

    #[error("failed to estimate price")]
    Price(#[from] PriceEstimationError),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Error, Debug)]
pub enum FindQuoteError {
    #[error("quote not found")]
    NotFound(Option<QuoteId>),

    #[error("quote does not match parameters")]
    ParameterMismatch(QuoteData),

    #[error("quote expired")]
    Expired(DateTime<Utc>),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Fields for searching stored quotes.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct QuoteSearchParameters {
    pub sell_token: H160,
    pub buy_token: H160,
    pub sell_amount: U256,
    pub buy_amount: U256,
    pub fee_amount: U256,
    pub kind: OrderKind,
    pub signing_scheme: QuoteSigningScheme,
    pub additional_gas: u64,
    /// If this is `Some` the quotes are expected to pass simulations using the
    /// contained parameters.
    pub verification: Option<Verification>,
}

impl QuoteSearchParameters {
    /// Returns true if the search parameter instance matches the specified
    /// quote data.
    fn matches(&self, data: &QuoteData) -> bool {
        let amounts_match = match self.kind {
            OrderKind::Buy => self.buy_amount == data.quoted_buy_amount,
            OrderKind::Sell => {
                self.sell_amount == data.quoted_sell_amount
                    || self.sell_amount + self.fee_amount == data.quoted_sell_amount
            }
        };

        amounts_match
            && (self.sell_token, self.buy_token, self.kind)
                == (data.sell_token, data.buy_token, data.kind)
    }

    /// Returns additional gas costs incurred by the quote.
    fn additional_cost(&self) -> u64 {
        self.signing_scheme
            .additional_gas_amount()
            .saturating_add(self.additional_gas)
    }
}

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait QuoteStoring: Send + Sync {
    /// Saves a quote and returns its ID.
    ///
    /// This storage implementation should return `None` to indicate that it
    /// will not store the quote.
    async fn save(&self, data: QuoteData) -> Result<QuoteId>;

    /// Retrieves an existing quote by ID.
    async fn get(&self, id: QuoteId) -> Result<Option<QuoteData>>;

    /// Retrieves an existing quote by ID.
    async fn find(
        &self,
        parameters: QuoteSearchParameters,
        expiration: DateTime<Utc>,
    ) -> Result<Option<(QuoteId, QuoteData)>>;
}

#[cfg_attr(test, mockall::automock)]
pub trait Now: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

impl<F> Now for F
where
    F: Fn() -> DateTime<Utc> + Send + Sync,
{
    fn now(&self) -> DateTime<Utc> {
        (self)()
    }
}

impl Now for DateTime<Utc> {
    fn now(&self) -> DateTime<Utc> {
        *self
    }
}

/// Standard validity for a quote: Quotes are stored only as long as they are
/// valid.
const STANDARD_QUOTE_VALIDITY_SECONDS: i64 = 60;

/// An order quoter implementation that relies
pub struct OrderQuoter {
    price_estimator: Arc<dyn PriceEstimating>,
    native_price_estimator: Arc<dyn NativePriceEstimating>,
    gas_estimator: Arc<dyn GasPriceEstimating>,
    fee_subsidy: Arc<dyn FeeSubsidizing>,
    storage: Arc<dyn QuoteStoring>,
    now: Arc<dyn Now>,
    eip1271_onchain_quote_validity_seconds: Duration,
    presign_onchain_quote_validity_seconds: Duration,
}

impl OrderQuoter {
    pub fn new(
        price_estimator: Arc<dyn PriceEstimating>,
        native_price_estimator: Arc<dyn NativePriceEstimating>,
        gas_estimator: Arc<dyn GasPriceEstimating>,
        fee_subsidy: Arc<dyn FeeSubsidizing>,
        storage: Arc<dyn QuoteStoring>,
        eip1271_onchain_quote_validity_seconds: Duration,
        presign_onchain_quote_validity_seconds: Duration,
    ) -> Self {
        Self {
            price_estimator,
            native_price_estimator,
            gas_estimator,
            fee_subsidy,
            storage,
            now: Arc::new(Utc::now),
            eip1271_onchain_quote_validity_seconds,
            presign_onchain_quote_validity_seconds,
        }
    }

    async fn compute_quote_data(
        &self,
        parameters: &QuoteParameters,
    ) -> Result<QuoteData, CalculateQuoteError> {
        let expiration = match parameters.signing_scheme {
            QuoteSigningScheme::Eip1271 {
                onchain_order: true,
                ..
            } => self.now.now() + self.eip1271_onchain_quote_validity_seconds,
            QuoteSigningScheme::PreSign {
                onchain_order: true,
            } => self.now.now() + self.presign_onchain_quote_validity_seconds,
            _ => self.now.now() + chrono::Duration::seconds(STANDARD_QUOTE_VALIDITY_SECONDS),
        };

        let trade_query = Arc::new(parameters.to_price_query());
        let (gas_estimate, trade_estimate, sell_token_price, _) = futures::try_join!(
            self.gas_estimator
                .estimate()
                .map_err(PriceEstimationError::ProtocolInternal),
            self.price_estimator.estimate(trade_query.clone()),
            self.native_price_estimator
                .estimate_native_price(parameters.sell_token),
            // We don't care about the native price of the buy_token for the quote but we need it
            // when we build the auction. To prevent creating orders which we can't settle later on
            // we make the native buy_token price a requirement here as well.
            self.native_price_estimator
                .estimate_native_price(parameters.buy_token),
        )?;

        let (quoted_sell_amount, quoted_buy_amount) = match &parameters.side {
            OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee { value: sell_amount },
            }
            | OrderQuoteSide::Sell {
                sell_amount: SellAmount::AfterFee { value: sell_amount },
            } => (sell_amount.get(), trade_estimate.out_amount),
            OrderQuoteSide::Buy {
                buy_amount_after_fee: buy_amount,
            } => (trade_estimate.out_amount, buy_amount.get()),
        };
        let fee_parameters = FeeParameters {
            gas_amount: trade_estimate.gas as _,
            gas_price: gas_estimate.effective_gas_price(),
            sell_token_price,
        };

        let quote_kind = quote_kind_from_signing_scheme(&parameters.signing_scheme);
        let quote = QuoteData {
            sell_token: parameters.sell_token,
            buy_token: parameters.buy_token,
            quoted_sell_amount,
            quoted_buy_amount,
            fee_parameters,
            kind: trade_query.kind,
            expiration,
            quote_kind,
            solver: trade_estimate.solver,
        };

        Ok(quote)
    }
}

#[async_trait::async_trait]
impl OrderQuoting for OrderQuoter {
    async fn calculate_quote(
        &self,
        parameters: QuoteParameters,
    ) -> Result<Quote, CalculateQuoteError> {
        let (data, subsidy) = futures::try_join!(
            self.compute_quote_data(&parameters),
            self.fee_subsidy
                .subsidy(SubsidyParameters {
                    from: parameters
                        .verification
                        .as_ref()
                        .map(|v| v.from)
                        .unwrap_or_default(),
                })
                .map_err(From::from),
        )?;

        let mut quote = Quote::new(Default::default(), data)
            .with_subsidy_and_additional_cost(&subsidy, parameters.additional_cost());

        // Make sure to scale the sell and buy amounts for quotes for sell
        // amounts before fees.
        if let OrderQuoteSide::Sell {
            sell_amount:
                SellAmount::BeforeFee {
                    value: sell_amount_before_fee,
                },
        } = &parameters.side
        {
            let sell_amount =
                Into::<U256>::into(*sell_amount_before_fee).saturating_sub(quote.fee_amount);
            if sell_amount == U256::zero() {
                // We want a sell_amount of at least 1!
                return Err(CalculateQuoteError::SellAmountDoesNotCoverFee {
                    fee_amount: quote.fee_amount,
                });
            }

            quote = quote.with_scaled_sell_amount(sell_amount);
        }

        tracing::debug!(?quote, ?subsidy, "computed quote");
        Ok(quote)
    }

    async fn store_quote(&self, quote: Quote) -> Result<Quote> {
        let id = self.storage.save(quote.data.clone()).await?;
        Ok(Quote {
            id: Some(id),
            ..quote
        })
    }

    async fn find_quote(
        &self,
        id: Option<QuoteId>,
        parameters: QuoteSearchParameters,
    ) -> Result<Quote, FindQuoteError> {
        let scaled_sell_amount = match parameters.kind {
            OrderKind::Sell => Some(parameters.sell_amount),
            OrderKind::Buy => None,
        };

        let subsidy = SubsidyParameters {
            from: parameters
                .verification
                .as_ref()
                .map(|v| v.from)
                .unwrap_or_default(),
        };

        let now = self.now.now();
        let additional_cost = parameters.additional_cost();
        let quote = async {
            let (id, data) = match id {
                Some(id) => {
                    let data = self
                        .storage
                        .get(id)
                        .await?
                        .ok_or(FindQuoteError::NotFound(Some(id)))?;

                    if !parameters.matches(&data) {
                        return Err(FindQuoteError::ParameterMismatch(data));
                    }
                    if data.expiration < now {
                        return Err(FindQuoteError::Expired(data.expiration));
                    }

                    (id, data)
                }
                None => self
                    .storage
                    .find(parameters, now)
                    .await?
                    .ok_or(FindQuoteError::NotFound(None))?,
            };
            Ok(Quote::new(Some(id), data))
        };

        let (quote, subsidy) = futures::try_join!(
            quote,
            self.fee_subsidy
                .subsidy(subsidy)
                .map_err(FindQuoteError::from)
        )?;

        let quote = quote.with_subsidy_and_additional_cost(&subsidy, additional_cost);
        let quote = match scaled_sell_amount {
            Some(sell_amount) => quote.with_scaled_sell_amount(sell_amount),
            None => quote,
        };

        tracing::debug!(?quote, ?subsidy, "found quote");
        Ok(quote)
    }
}

impl From<&OrderQuoteRequest> for PreOrderData {
    fn from(quote_request: &OrderQuoteRequest) -> Self {
        let owner = quote_request.from;
        Self {
            owner,
            sell_token: quote_request.sell_token,
            buy_token: quote_request.buy_token,
            receiver: quote_request.receiver.unwrap_or(owner),
            valid_to: quote_request.validity.actual_valid_to(),
            partially_fillable: quote_request.partially_fillable,
            buy_token_balance: quote_request.buy_token_balance,
            sell_token_balance: quote_request.sell_token_balance,
            signing_scheme: quote_request.signing_scheme.into(),
            class: OrderClass::Market,
        }
    }
}

pub fn quote_kind_from_signing_scheme(scheme: &QuoteSigningScheme) -> QuoteKind {
    match scheme {
        QuoteSigningScheme::Eip1271 {
            onchain_order: true,
            ..
        } => QuoteKind::Eip1271OnchainOrder,
        QuoteSigningScheme::PreSign {
            onchain_order: true,
        } => QuoteKind::PreSignOnchainOrder,
        _ => QuoteKind::Standard,
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            fee_subsidy::Subsidy,
            gas_price_estimation::FakeGasPriceEstimator,
            price_estimation::{native::MockNativePriceEstimating, MockPriceEstimating},
        },
        chrono::Utc,
        ethcontract::H160,
        futures::FutureExt,
        gas_estimation::GasPrice1559,
        mockall::{predicate::eq, Sequence},
        model::{quote::Validity, time},
        number::nonzero::U256 as NonZeroU256,
        std::sync::Mutex,
    };

    #[test]
    fn pre_order_data_from_quote_request() {
        let quote_request = OrderQuoteRequest {
            validity: Validity::To(0),
            ..Default::default()
        };
        let result = PreOrderData::from(&quote_request);
        let expected = PreOrderData::default();
        assert_eq!(result, expected);
    }

    #[test]
    fn pre_order_data_from_quote_request_with_valid_for() {
        let quote_request = OrderQuoteRequest {
            validity: Validity::For(100),
            ..Default::default()
        };
        let result = PreOrderData::from(&quote_request);

        let valid_duration = result.valid_to - time::now_in_epoch_seconds();

        // use time-range to make sure test isn't flaky.
        assert!((95..=105).contains(&valid_duration));
    }

    #[tokio::test]
    async fn compute_sell_before_fee_quote() {
        let now = Utc::now();
        let parameters = QuoteParameters {
            sell_token: H160([1; 20]),
            buy_token: H160([2; 20]),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: NonZeroU256::try_from(100).unwrap(),
                },
            },
            verification: Some(Verification {
                from: H160([3; 20]),
                ..Default::default()
            }),
            signing_scheme: QuoteSigningScheme::Eip712,
            additional_gas: 0,
        };
        let gas_price = GasPrice1559 {
            base_fee_per_gas: 1.5,
            max_fee_per_gas: 3.0,
            max_priority_fee_per_gas: 0.5,
        };

        let mut price_estimator = MockPriceEstimating::new();
        price_estimator
            .expect_estimate()
            .withf(|q| {
                **q == price_estimation::Query {
                    verification: Some(Verification {
                        from: H160([3; 20]),
                        ..Default::default()
                    }),
                    sell_token: H160([1; 20]),
                    buy_token: H160([2; 20]),
                    in_amount: NonZeroU256::try_from(100).unwrap(),
                    kind: OrderKind::Sell,
                }
            })
            .returning(|_| {
                async {
                    Ok(price_estimation::Estimate {
                        out_amount: 42.into(),
                        gas: 3,
                        solver: H160([1; 20]),
                    })
                }
                .boxed()
            });

        let mut native_price_estimator = MockNativePriceEstimating::new();
        native_price_estimator
            .expect_estimate_native_price()
            .withf({
                let sell_token = parameters.sell_token;
                move |q| q == &sell_token
            })
            .returning(|_| async { Ok(0.2) }.boxed());
        native_price_estimator
            .expect_estimate_native_price()
            .withf({
                let buy_token = parameters.buy_token;
                move |q| q == &buy_token
            })
            .returning(|_| async { Ok(0.2) }.boxed());

        let gas_estimator = FakeGasPriceEstimator(Arc::new(Mutex::new(gas_price)));

        let mut storage = MockQuoteStoring::new();
        storage
            .expect_save()
            .with(eq(QuoteData {
                sell_token: H160([1; 20]),
                buy_token: H160([2; 20]),
                quoted_sell_amount: 100.into(),
                quoted_buy_amount: 42.into(),
                fee_parameters: FeeParameters {
                    gas_amount: 3.,
                    gas_price: 2.,
                    sell_token_price: 0.2,
                },
                kind: OrderKind::Sell,
                expiration: now + Duration::seconds(STANDARD_QUOTE_VALIDITY_SECONDS),
                quote_kind: QuoteKind::Standard,
                solver: H160([1; 20]),
            }))
            .returning(|_| Ok(1337));

        let quoter = OrderQuoter {
            price_estimator: Arc::new(price_estimator),
            native_price_estimator: Arc::new(native_price_estimator),
            gas_estimator: Arc::new(gas_estimator),
            fee_subsidy: Arc::new(Subsidy::default()),
            storage: Arc::new(storage),
            now: Arc::new(now),
            eip1271_onchain_quote_validity_seconds: Duration::seconds(60i64),
            presign_onchain_quote_validity_seconds: Duration::seconds(60i64),
        };

        let quote = quoter.calculate_quote(parameters).await.unwrap();
        let quote = quoter.store_quote(quote).await.unwrap();

        assert_eq!(
            quote,
            Quote {
                id: Some(1337),
                data: QuoteData {
                    sell_token: H160([1; 20]),
                    buy_token: H160([2; 20]),
                    quoted_sell_amount: 100.into(),
                    quoted_buy_amount: 42.into(),
                    fee_parameters: FeeParameters {
                        gas_amount: 3.,
                        gas_price: 2.,
                        sell_token_price: 0.2,
                    },
                    kind: OrderKind::Sell,
                    expiration: now + chrono::Duration::seconds(STANDARD_QUOTE_VALIDITY_SECONDS),
                    quote_kind: QuoteKind::Standard,
                    solver: H160([1; 20]),
                },
                sell_amount: 70.into(),
                buy_amount: 29.into(),
                fee_amount: 30.into(),
                full_fee_amount: 30.into(),
            }
        );
    }

    #[tokio::test]
    async fn compute_sell_after_fee_quote() {
        let now = Utc::now();
        let parameters = QuoteParameters {
            sell_token: H160([1; 20]),
            buy_token: H160([2; 20]),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::AfterFee {
                    value: NonZeroU256::try_from(100).unwrap(),
                },
            },
            verification: Some(Verification {
                from: H160([3; 20]),
                ..Default::default()
            }),
            signing_scheme: QuoteSigningScheme::Eip1271 {
                onchain_order: false,
                verification_gas_limit: 1,
            },
            additional_gas: 2,
        };
        let gas_price = GasPrice1559 {
            base_fee_per_gas: 1.5,
            max_fee_per_gas: 3.0,
            max_priority_fee_per_gas: 0.5,
        };

        let mut price_estimator = MockPriceEstimating::new();
        price_estimator
            .expect_estimate()
            .withf(|q| {
                **q == price_estimation::Query {
                    verification: Some(Verification {
                        from: H160([3; 20]),
                        ..Default::default()
                    }),
                    sell_token: H160([1; 20]),
                    buy_token: H160([2; 20]),
                    in_amount: NonZeroU256::try_from(100).unwrap(),
                    kind: OrderKind::Sell,
                }
            })
            .returning(|_| {
                async {
                    Ok(price_estimation::Estimate {
                        out_amount: 42.into(),
                        gas: 3,
                        solver: H160([1; 20]),
                    })
                }
                .boxed()
            });

        let mut native_price_estimator = MockNativePriceEstimating::new();
        native_price_estimator
            .expect_estimate_native_price()
            .withf({
                let sell_token = parameters.sell_token;
                move |q| q == &sell_token
            })
            .returning(|_| async { Ok(0.2) }.boxed());
        native_price_estimator
            .expect_estimate_native_price()
            .withf({
                let buy_token = parameters.buy_token;
                move |q| q == &buy_token
            })
            .returning(|_| async { Ok(0.2) }.boxed());

        let gas_estimator = FakeGasPriceEstimator(Arc::new(Mutex::new(gas_price)));

        let mut storage = MockQuoteStoring::new();
        storage
            .expect_save()
            .with(eq(QuoteData {
                sell_token: H160([1; 20]),
                buy_token: H160([2; 20]),
                quoted_sell_amount: 100.into(),
                quoted_buy_amount: 42.into(),
                fee_parameters: FeeParameters {
                    gas_amount: 3.,
                    gas_price: 2.,
                    sell_token_price: 0.2,
                },
                kind: OrderKind::Sell,
                expiration: now + chrono::Duration::seconds(STANDARD_QUOTE_VALIDITY_SECONDS),
                quote_kind: QuoteKind::Standard,
                solver: H160([1; 20]),
            }))
            .returning(|_| Ok(1337));

        let quoter = OrderQuoter {
            price_estimator: Arc::new(price_estimator),
            native_price_estimator: Arc::new(native_price_estimator),
            gas_estimator: Arc::new(gas_estimator),
            fee_subsidy: Arc::new(Subsidy {
                factor: 0.5,
                ..Default::default()
            }),
            storage: Arc::new(storage),
            now: Arc::new(now),
            eip1271_onchain_quote_validity_seconds: Duration::seconds(60i64),
            presign_onchain_quote_validity_seconds: Duration::seconds(60i64),
        };

        let quote = quoter.calculate_quote(parameters).await.unwrap();
        let quote = quoter.store_quote(quote).await.unwrap();

        assert_eq!(
            quote,
            Quote {
                id: Some(1337),
                data: QuoteData {
                    sell_token: H160([1; 20]),
                    buy_token: H160([2; 20]),
                    quoted_sell_amount: 100.into(),
                    quoted_buy_amount: 42.into(),
                    fee_parameters: FeeParameters {
                        gas_amount: 3.,
                        gas_price: 2.,
                        sell_token_price: 0.2,
                    },
                    kind: OrderKind::Sell,
                    expiration: now + chrono::Duration::seconds(STANDARD_QUOTE_VALIDITY_SECONDS),
                    quote_kind: QuoteKind::Standard,
                    solver: H160([1; 20]),
                },
                sell_amount: 100.into(),
                buy_amount: 42.into(),
                fee_amount: 30.into(),
                full_fee_amount: 60.into(),
            }
        );
    }

    #[tokio::test]
    async fn compute_buy_quote() {
        let now = Utc::now();
        let parameters = QuoteParameters {
            sell_token: H160([1; 20]),
            buy_token: H160([2; 20]),
            side: OrderQuoteSide::Buy {
                buy_amount_after_fee: NonZeroU256::try_from(42).unwrap(),
            },
            verification: Some(Verification {
                from: H160([3; 20]),
                ..Default::default()
            }),
            signing_scheme: QuoteSigningScheme::Eip712,
            additional_gas: 0,
        };
        let gas_price = GasPrice1559 {
            base_fee_per_gas: 1.5,
            max_fee_per_gas: 3.0,
            max_priority_fee_per_gas: 0.5,
        };

        let mut price_estimator = MockPriceEstimating::new();
        price_estimator
            .expect_estimate()
            .withf(|q| {
                **q == price_estimation::Query {
                    verification: Some(Verification {
                        from: H160([3; 20]),
                        ..Default::default()
                    }),
                    sell_token: H160([1; 20]),
                    buy_token: H160([2; 20]),
                    in_amount: NonZeroU256::try_from(42).unwrap(),
                    kind: OrderKind::Buy,
                }
            })
            .returning(|_| {
                async {
                    Ok(price_estimation::Estimate {
                        out_amount: 100.into(),
                        gas: 3,
                        solver: H160([1; 20]),
                    })
                }
                .boxed()
            });

        let mut native_price_estimator = MockNativePriceEstimating::new();
        native_price_estimator
            .expect_estimate_native_price()
            .withf({
                let sell_token = parameters.sell_token;
                move |q| q == &sell_token
            })
            .returning(|_| async { Ok(0.2) }.boxed());
        native_price_estimator
            .expect_estimate_native_price()
            .withf({
                let buy_token = parameters.buy_token;
                move |q| q == &buy_token
            })
            .returning(|_| async { Ok(0.2) }.boxed());

        let gas_estimator = FakeGasPriceEstimator(Arc::new(Mutex::new(gas_price)));

        let mut storage = MockQuoteStoring::new();
        storage
            .expect_save()
            .with(eq(QuoteData {
                sell_token: H160([1; 20]),
                buy_token: H160([2; 20]),
                quoted_sell_amount: 100.into(),
                quoted_buy_amount: 42.into(),
                fee_parameters: FeeParameters {
                    gas_amount: 3.,
                    gas_price: 2.,
                    sell_token_price: 0.2,
                },
                kind: OrderKind::Buy,
                expiration: now + chrono::Duration::seconds(STANDARD_QUOTE_VALIDITY_SECONDS),
                quote_kind: QuoteKind::Standard,
                solver: H160([1; 20]),
            }))
            .returning(|_| Ok(1337));

        let quoter = OrderQuoter {
            price_estimator: Arc::new(price_estimator),
            native_price_estimator: Arc::new(native_price_estimator),
            gas_estimator: Arc::new(gas_estimator),
            fee_subsidy: Arc::new(Subsidy {
                discount: 5.,
                min_discounted: 2.,
                factor: 0.9,
            }),
            storage: Arc::new(storage),
            now: Arc::new(now),
            eip1271_onchain_quote_validity_seconds: Duration::seconds(60i64),
            presign_onchain_quote_validity_seconds: Duration::seconds(60i64),
        };

        let quote = quoter.calculate_quote(parameters).await.unwrap();
        let quote = quoter.store_quote(quote).await.unwrap();

        assert_eq!(
            quote,
            Quote {
                id: Some(1337),
                data: QuoteData {
                    sell_token: H160([1; 20]),
                    buy_token: H160([2; 20]),
                    quoted_sell_amount: 100.into(),
                    quoted_buy_amount: 42.into(),
                    fee_parameters: FeeParameters {
                        gas_amount: 3.,
                        gas_price: 2.,
                        sell_token_price: 0.2,
                    },
                    kind: OrderKind::Buy,
                    expiration: now + chrono::Duration::seconds(STANDARD_QUOTE_VALIDITY_SECONDS),
                    quote_kind: QuoteKind::Standard,
                    solver: H160([1; 20]),
                },
                sell_amount: 100.into(),
                buy_amount: 42.into(),
                fee_amount: 9.into(),
                full_fee_amount: 30.into(),
            }
        );
    }

    #[tokio::test]
    async fn compute_sell_before_fee_quote_insufficient_amount_error() {
        let parameters = QuoteParameters {
            sell_token: H160([1; 20]),
            buy_token: H160([2; 20]),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: NonZeroU256::try_from(100).unwrap(),
                },
            },
            verification: Some(Verification {
                from: H160([3; 20]),
                ..Default::default()
            }),
            signing_scheme: QuoteSigningScheme::Eip712,
            additional_gas: 0,
        };
        let gas_price = GasPrice1559 {
            base_fee_per_gas: 1.,
            max_fee_per_gas: 2.,
            max_priority_fee_per_gas: 0.,
        };

        let mut price_estimator = MockPriceEstimating::new();
        price_estimator.expect_estimate().returning(|_| {
            async {
                Ok(price_estimation::Estimate {
                    out_amount: 100.into(),
                    gas: 200,
                    solver: H160([1; 20]),
                })
            }
            .boxed()
        });

        let mut native_price_estimator = MockNativePriceEstimating::new();
        native_price_estimator
            .expect_estimate_native_price()
            .withf({
                let sell_token = parameters.sell_token;
                move |q| q == &sell_token
            })
            .returning(|_| async { Ok(1.) }.boxed());
        native_price_estimator
            .expect_estimate_native_price()
            .withf({
                let buy_token = parameters.buy_token;
                move |q| q == &buy_token
            })
            .returning(|_| async { Ok(1.) }.boxed());

        let gas_estimator = FakeGasPriceEstimator(Arc::new(Mutex::new(gas_price)));

        let quoter = OrderQuoter {
            price_estimator: Arc::new(price_estimator),
            native_price_estimator: Arc::new(native_price_estimator),
            gas_estimator: Arc::new(gas_estimator),
            fee_subsidy: Arc::new(Subsidy::default()),
            storage: Arc::new(MockQuoteStoring::new()),
            now: Arc::new(Utc::now),
            eip1271_onchain_quote_validity_seconds: Duration::seconds(60i64),
            presign_onchain_quote_validity_seconds: Duration::seconds(60i64),
        };

        assert!(matches!(
            quoter.calculate_quote(parameters).await.unwrap_err(),
            CalculateQuoteError::SellAmountDoesNotCoverFee { fee_amount } if fee_amount == U256::from(200),
        ));
    }

    #[tokio::test]
    async fn require_native_price_for_buy_token() {
        let parameters = QuoteParameters {
            sell_token: H160([1; 20]),
            buy_token: H160([2; 20]),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: NonZeroU256::try_from(100_000).unwrap(),
                },
            },
            verification: Some(Verification {
                from: H160([3; 20]),
                ..Default::default()
            }),
            signing_scheme: QuoteSigningScheme::Eip712,
            additional_gas: 0,
        };
        let gas_price = GasPrice1559 {
            base_fee_per_gas: 1.,
            max_fee_per_gas: 2.,
            max_priority_fee_per_gas: 0.,
        };

        let mut price_estimator = MockPriceEstimating::new();
        price_estimator.expect_estimate().returning(|_| {
            async {
                Ok(price_estimation::Estimate {
                    out_amount: 100.into(),
                    gas: 200,
                    solver: H160([1; 20]),
                })
            }
            .boxed()
        });

        let mut native_price_estimator = MockNativePriceEstimating::new();
        native_price_estimator
            .expect_estimate_native_price()
            .withf({
                let sell_token = parameters.sell_token;
                move |q| q == &sell_token
            })
            .returning(|_| async { Ok(1.) }.boxed());
        native_price_estimator
            .expect_estimate_native_price()
            .withf({
                let buy_token = parameters.buy_token;
                move |q| q == &buy_token
            })
            .returning(|_| async { Err(PriceEstimationError::NoLiquidity) }.boxed());

        let gas_estimator = FakeGasPriceEstimator(Arc::new(Mutex::new(gas_price)));

        let quoter = OrderQuoter {
            price_estimator: Arc::new(price_estimator),
            native_price_estimator: Arc::new(native_price_estimator),
            gas_estimator: Arc::new(gas_estimator),
            fee_subsidy: Arc::new(Subsidy::default()),
            storage: Arc::new(MockQuoteStoring::new()),
            now: Arc::new(Utc::now),
            eip1271_onchain_quote_validity_seconds: Duration::seconds(60i64),
            presign_onchain_quote_validity_seconds: Duration::seconds(60i64),
        };

        assert!(matches!(
            quoter.calculate_quote(parameters).await.unwrap_err(),
            CalculateQuoteError::Price(PriceEstimationError::NoLiquidity),
        ));
    }

    #[tokio::test]
    async fn finds_quote_by_id() {
        let now = Utc::now();
        let quote_id = 42;
        let parameters = QuoteSearchParameters {
            sell_token: H160([1; 20]),
            buy_token: H160([2; 20]),
            sell_amount: 85.into(),
            buy_amount: 40.into(),
            fee_amount: 15.into(),
            kind: OrderKind::Sell,
            signing_scheme: QuoteSigningScheme::Eip712,
            additional_gas: 0,
            verification: Some(Verification {
                from: H160([3; 20]),
                ..Default::default()
            }),
        };

        let mut storage = MockQuoteStoring::new();
        storage.expect_get().with(eq(42)).returning(move |_| {
            Ok(Some(QuoteData {
                sell_token: H160([1; 20]),
                buy_token: H160([2; 20]),
                quoted_sell_amount: 100.into(),
                quoted_buy_amount: 42.into(),
                fee_parameters: FeeParameters {
                    gas_amount: 3.,
                    gas_price: 2.,
                    sell_token_price: 0.2,
                },
                kind: OrderKind::Sell,
                expiration: now + chrono::Duration::seconds(10),
                quote_kind: QuoteKind::Standard,
                solver: H160([1; 20]),
            }))
        });

        let quoter = OrderQuoter {
            price_estimator: Arc::new(MockPriceEstimating::new()),
            native_price_estimator: Arc::new(MockNativePriceEstimating::new()),
            gas_estimator: Arc::new(FakeGasPriceEstimator::default()),
            fee_subsidy: Arc::new(Subsidy {
                factor: 0.25,
                ..Default::default()
            }),
            storage: Arc::new(storage),
            now: Arc::new(now),
            eip1271_onchain_quote_validity_seconds: Duration::seconds(60i64),
            presign_onchain_quote_validity_seconds: Duration::seconds(60i64),
        };

        assert_eq!(
            quoter.find_quote(Some(quote_id), parameters).await.unwrap(),
            Quote {
                id: Some(42),
                data: QuoteData {
                    sell_token: H160([1; 20]),
                    buy_token: H160([2; 20]),
                    quoted_sell_amount: 100.into(),
                    quoted_buy_amount: 42.into(),
                    fee_parameters: FeeParameters {
                        gas_amount: 3.,
                        gas_price: 2.,
                        sell_token_price: 0.2,
                    },
                    kind: OrderKind::Sell,
                    expiration: now + chrono::Duration::seconds(10),
                    quote_kind: QuoteKind::Standard,
                    solver: H160([1; 20]),
                },
                sell_amount: 85.into(),
                // Allows for "out-of-price" buy amounts. This means that order
                // be used for providing liquidity at a premium over current
                // market price.
                buy_amount: 35.into(),
                // Allows for different subsidized fee amounts. This means that
                // order created with legacy APIs or incomplete quote data (for
                // example `from` is specified as a random address) can still
                // create orders with fees that aren't fully subsidized.
                fee_amount: 8.into(),
                full_fee_amount: 30.into(),
            }
        );
    }

    #[tokio::test]
    async fn finds_quote_with_sell_amount_after_fee() {
        let now = Utc::now();
        let quote_id = 42;
        let parameters = QuoteSearchParameters {
            sell_token: H160([1; 20]),
            buy_token: H160([2; 20]),
            sell_amount: 100.into(),
            buy_amount: 40.into(),
            fee_amount: 30.into(),
            kind: OrderKind::Sell,
            signing_scheme: QuoteSigningScheme::Eip712,
            additional_gas: 0,
            verification: Some(Verification {
                from: H160([3; 20]),
                ..Default::default()
            }),
        };

        let mut storage = MockQuoteStoring::new();
        storage.expect_get().with(eq(42)).returning(move |_| {
            Ok(Some(QuoteData {
                sell_token: H160([1; 20]),
                buy_token: H160([2; 20]),
                quoted_sell_amount: 100.into(),
                quoted_buy_amount: 42.into(),
                fee_parameters: FeeParameters {
                    gas_amount: 3.,
                    gas_price: 2.,
                    sell_token_price: 0.2,
                },
                kind: OrderKind::Sell,
                expiration: now + chrono::Duration::seconds(10),
                quote_kind: QuoteKind::Standard,
                solver: H160([1; 20]),
            }))
        });

        let quoter = OrderQuoter {
            price_estimator: Arc::new(MockPriceEstimating::new()),
            native_price_estimator: Arc::new(MockNativePriceEstimating::new()),
            gas_estimator: Arc::new(FakeGasPriceEstimator::default()),
            fee_subsidy: Arc::new(Subsidy::default()),
            storage: Arc::new(storage),
            now: Arc::new(now),
            eip1271_onchain_quote_validity_seconds: Duration::seconds(60i64),
            presign_onchain_quote_validity_seconds: Duration::seconds(60i64),
        };

        assert_eq!(
            quoter.find_quote(Some(quote_id), parameters).await.unwrap(),
            Quote {
                id: Some(42),
                data: QuoteData {
                    sell_token: H160([1; 20]),
                    buy_token: H160([2; 20]),
                    quoted_sell_amount: 100.into(),
                    quoted_buy_amount: 42.into(),
                    fee_parameters: FeeParameters {
                        gas_amount: 3.,
                        gas_price: 2.,
                        sell_token_price: 0.2,
                    },
                    kind: OrderKind::Sell,
                    expiration: now + chrono::Duration::seconds(10),
                    quote_kind: QuoteKind::Standard,
                    solver: H160([1; 20]),
                },
                sell_amount: 100.into(),
                buy_amount: 42.into(),
                fee_amount: 30.into(),
                full_fee_amount: 30.into(),
            }
        );
    }

    #[tokio::test]
    async fn finds_quote_by_parameters() {
        let now = Utc::now();
        let parameters = QuoteSearchParameters {
            sell_token: H160([1; 20]),
            buy_token: H160([2; 20]),
            sell_amount: 110.into(),
            buy_amount: 42.into(),
            fee_amount: 30.into(),
            kind: OrderKind::Buy,
            signing_scheme: QuoteSigningScheme::Eip712,
            additional_gas: 0,
            verification: Some(Verification {
                from: H160([3; 20]),
                ..Default::default()
            }),
        };

        let mut storage = MockQuoteStoring::new();
        storage
            .expect_find()
            .with(eq(parameters.clone()), eq(now))
            .returning(move |_, _| {
                Ok(Some((
                    42,
                    QuoteData {
                        sell_token: H160([1; 20]),
                        buy_token: H160([2; 20]),
                        quoted_sell_amount: 100.into(),
                        quoted_buy_amount: 42.into(),
                        fee_parameters: FeeParameters {
                            gas_amount: 3.,
                            gas_price: 2.,
                            sell_token_price: 0.2,
                        },
                        kind: OrderKind::Buy,
                        expiration: now + chrono::Duration::seconds(10),
                        quote_kind: QuoteKind::Standard,
                        solver: H160([1; 20]),
                    },
                )))
            });

        let quoter = OrderQuoter {
            price_estimator: Arc::new(MockPriceEstimating::new()),
            native_price_estimator: Arc::new(MockNativePriceEstimating::new()),
            gas_estimator: Arc::new(FakeGasPriceEstimator::default()),
            fee_subsidy: Arc::new(Subsidy::default()),
            storage: Arc::new(storage),
            now: Arc::new(now),
            eip1271_onchain_quote_validity_seconds: Duration::seconds(60i64),
            presign_onchain_quote_validity_seconds: Duration::seconds(60i64),
        };

        assert_eq!(
            quoter.find_quote(None, parameters).await.unwrap(),
            Quote {
                id: Some(42),
                data: QuoteData {
                    sell_token: H160([1; 20]),
                    buy_token: H160([2; 20]),
                    quoted_sell_amount: 100.into(),
                    quoted_buy_amount: 42.into(),
                    fee_parameters: FeeParameters {
                        gas_amount: 3.,
                        gas_price: 2.,
                        sell_token_price: 0.2,
                    },
                    kind: OrderKind::Buy,
                    expiration: now + chrono::Duration::seconds(10),
                    quote_kind: QuoteKind::Standard,
                    solver: H160([1; 20]),
                },
                sell_amount: 100.into(),
                buy_amount: 42.into(),
                fee_amount: 30.into(),
                full_fee_amount: 30.into(),
            }
        );
    }

    #[tokio::test]
    async fn find_invalid_quote_error() {
        let now = Utc::now();
        let parameters = QuoteSearchParameters {
            sell_token: H160([1; 20]),
            ..Default::default()
        };

        let mut storage = MockQuoteStoring::new();
        let mut sequence = Sequence::new();
        storage
            .expect_get()
            .times(1)
            .in_sequence(&mut sequence)
            .returning(move |_| {
                Ok(Some(QuoteData {
                    sell_token: H160([2; 20]),
                    expiration: now,
                    ..Default::default()
                }))
            });
        storage
            .expect_get()
            .times(1)
            .in_sequence(&mut sequence)
            .returning(move |_| {
                Ok(Some(QuoteData {
                    sell_token: H160([1; 20]),
                    expiration: now - chrono::Duration::seconds(1),
                    ..Default::default()
                }))
            });

        let quoter = OrderQuoter {
            price_estimator: Arc::new(MockPriceEstimating::new()),
            native_price_estimator: Arc::new(MockNativePriceEstimating::new()),
            gas_estimator: Arc::new(FakeGasPriceEstimator::default()),
            fee_subsidy: Arc::new(Subsidy::default()),
            storage: Arc::new(storage),
            now: Arc::new(now),
            eip1271_onchain_quote_validity_seconds: Duration::seconds(60i64),
            presign_onchain_quote_validity_seconds: Duration::seconds(60i64),
        };

        assert!(matches!(
            quoter
                .find_quote(Some(0), parameters.clone())
                .await
                .unwrap_err(),
            FindQuoteError::ParameterMismatch(_),
        ));
        assert!(matches!(
            quoter.find_quote(Some(0), parameters).await.unwrap_err(),
            FindQuoteError::Expired(_),
        ));
    }

    #[tokio::test]
    async fn find_quote_error_when_not_found() {
        let mut storage = MockQuoteStoring::new();
        storage.expect_get().returning(move |_| Ok(None));
        storage.expect_find().returning(move |_, _| Ok(None));

        let quoter = OrderQuoter {
            price_estimator: Arc::new(MockPriceEstimating::new()),
            native_price_estimator: Arc::new(MockNativePriceEstimating::new()),
            gas_estimator: Arc::new(FakeGasPriceEstimator::default()),
            fee_subsidy: Arc::new(Subsidy::default()),
            storage: Arc::new(storage),
            now: Arc::new(Utc::now),
            eip1271_onchain_quote_validity_seconds: Duration::seconds(60i64),
            presign_onchain_quote_validity_seconds: Duration::seconds(60i64),
        };

        assert!(matches!(
            quoter
                .find_quote(Some(0), Default::default())
                .await
                .unwrap_err(),
            FindQuoteError::NotFound(Some(0)),
        ));
        assert!(matches!(
            quoter
                .find_quote(None, Default::default())
                .await
                .unwrap_err(),
            FindQuoteError::NotFound(None),
        ));
    }
}
