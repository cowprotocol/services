use {
    super::price_estimation::{
        self, PriceEstimating, PriceEstimationError, native::NativePriceEstimating,
    },
    crate::{
        account_balances::{BalanceFetching, Query},
        db_order_conversions::order_kind_from,
        fee::FeeParameters,
        gas_price_estimation::GasPriceEstimating,
        order_simulation::OrderExecutionSimulating,
        order_validation::PreOrderData,
        price_estimation::{Estimate, QuoteVerificationMode, Verification},
        trade_finding::external::dto,
    },
    alloy::primitives::{Address, U256, U512, ruint::UintTryFrom},
    anyhow::{Context, Result},
    chrono::{DateTime, Duration, Utc},
    database::quotes::{Quote as QuoteRow, QuoteKind},
    futures::TryFutureExt,
    model::{
        interaction::InteractionData,
        order::{OrderClass, OrderKind},
        quote::{OrderQuoteRequest, OrderQuoteSide, QuoteId, QuoteSigningScheme, SellAmount},
    },
    num::FromPrimitive,
    number::conversions::big_decimal_to_u256,
    std::sync::Arc,
    thiserror::Error,
    tracing::instrument,
};

/// Order parameters for quoting.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct QuoteParameters {
    pub sell_token: Address,
    pub buy_token: Address,
    pub side: OrderQuoteSide,
    pub verification: Verification,
    pub signing_scheme: QuoteSigningScheme,
    pub additional_gas: u64,
    pub timeout: Option<std::time::Duration>,
}

impl QuoteParameters {
    fn to_price_query(
        &self,
        default_quote_timeout: std::time::Duration,
    ) -> price_estimation::Query {
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

        let timeout = self
            .timeout
            .unwrap_or(default_quote_timeout)
            .min(default_quote_timeout);

        price_estimation::Query {
            verification: self.verification.clone(),
            sell_token: self.sell_token,
            buy_token: self.buy_token,
            in_amount,
            kind,
            block_dependent: true,
            timeout,
        }
    }

    pub fn additional_cost(&self) -> u64 {
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
    /// The fee amount for any order created for this quote. The fee is
    /// denoted in the sell token.
    pub fee_amount: U256,
}

impl Quote {
    /// Creates a new `Quote`.
    pub fn new(id: Option<QuoteId>, data: QuoteData) -> Self {
        Self {
            id,
            sell_amount: data.quoted_sell_amount,
            buy_amount: data.quoted_buy_amount,
            fee_amount: data.fee_parameters.fee(),
            data,
        }
    }

    /// Adjusts the quote fee to include arbitrary additional costs.
    pub fn with_additional_cost(mut self, additional_cost: u64) -> Self {
        // Be careful not to modify `self.data` as this represents the actual
        // quote data that is stored in the database. Instead, update the
        // computed fee fields.
        self.fee_amount = self
            .data
            .fee_parameters
            .fee_with_additional_cost(additional_cost);

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

        self.buy_amount = U256::uint_try_from(
            self.data
                .quoted_buy_amount
                .widening_mul::<_, _, 512, 8>(sell_amount)
                / U512::from(self.data.quoted_sell_amount),
        )
        .unwrap_or(U256::MAX);

        self
    }

    /// Converts this Quote to model OrderQuote.
    pub fn try_to_model_order_quote(&self) -> Result<model::order::OrderQuote> {
        Ok(model::order::OrderQuote {
            gas_amount: bigdecimal::BigDecimal::from_f64(self.data.fee_parameters.gas_amount)
                .context("gas amount is not a valid BigDecimal")?,
            gas_price: bigdecimal::BigDecimal::from_f64(self.data.fee_parameters.gas_price)
                .context("gas price is not a valid BigDecimal")?,
            sell_token_price: bigdecimal::BigDecimal::from_f64(
                self.data.fee_parameters.sell_token_price,
            )
            .context("sell token price is not a valid BigDecimal")?,
            sell_amount: self.sell_amount,
            buy_amount: self.buy_amount,
            fee_amount: self.fee_amount,
            solver: self.data.solver,
            verified: self.data.verified,
            metadata: serde_json::to_value(&self.data.metadata)?,
        })
    }
}

/// Detailed data for a computed order quote.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct QuoteData {
    pub sell_token: Address,
    pub buy_token: Address,
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
    pub solver: Address,
    /// Were we able to verify that this quote is accurate?
    pub verified: bool,
    /// Additional data associated with the quote.
    pub metadata: QuoteMetadata,
}

impl TryFrom<QuoteRow> for QuoteData {
    type Error = anyhow::Error;

    fn try_from(row: QuoteRow) -> Result<QuoteData> {
        Ok(QuoteData {
            sell_token: Address::from_slice(&row.sell_token.0),
            buy_token: Address::from_slice(&row.buy_token.0),
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
            solver: Address::from_slice(&row.solver.0),
            verified: row.verified,
            metadata: row.metadata.try_into()?,
        })
    }
}

#[cfg_attr(any(test, feature = "test-util"), mockall::automock)]
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
    #[error("sell amount does not cover fee {fee_amount:?}")]
    SellAmountDoesNotCoverFee { fee_amount: U256 },

    #[error("{estimator_kind:?} estimator failed: {source}")]
    Price {
        estimator_kind: EstimatorKind,
        source: PriceEstimationError,
    },

    #[error("failed to verify quote")]
    QuoteNotVerified,

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<(EstimatorKind, PriceEstimationError)> for CalculateQuoteError {
    fn from((estimator_kind, source): (EstimatorKind, PriceEstimationError)) -> Self {
        Self::Price {
            estimator_kind,
            source,
        }
    }
}

#[derive(Debug)]
pub enum EstimatorKind {
    /// The gas price estimator.
    Gas,
    /// Estimator for calculating the token pair price of an order.
    Regular,
    /// Estimator for calculating the native token price of order's sell token.
    NativeSell,
    /// Estimator for calculating the native token price of order's buy token.
    NativeBuy,
}

#[derive(Error, Debug)]
pub enum FindQuoteError {
    #[error("quote not found")]
    NotFound(Option<QuoteId>),

    #[error("quote does not match parameters")]
    ParameterMismatch(Box<QuoteData>),

    #[error("quote expired")]
    Expired(DateTime<Utc>),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Fields for searching stored quotes.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct QuoteSearchParameters {
    pub sell_token: Address,
    pub buy_token: Address,
    pub sell_amount: U256,
    pub buy_amount: U256,
    pub fee_amount: U256,
    pub kind: OrderKind,
    pub signing_scheme: QuoteSigningScheme,
    pub additional_gas: u64,
    pub verification: Verification,
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
    pub fn additional_cost(&self) -> u64 {
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

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Validity {
    pub eip1271_onchain_quote: Duration,
    pub presign_onchain_quote: Duration,
    pub standard_quote: Duration,
}

impl Default for Validity {
    fn default() -> Self {
        Self {
            eip1271_onchain_quote: Duration::seconds(600),
            presign_onchain_quote: Duration::seconds(600),
            standard_quote: Duration::seconds(60),
        }
    }
}

/// An order quoter implementation that relies
pub struct OrderQuoter {
    price_estimator: Arc<dyn PriceEstimating>,
    native_price_estimator: Arc<dyn NativePriceEstimating>,
    gas_estimator: Arc<dyn GasPriceEstimating>,
    storage: Arc<dyn QuoteStoring>,
    now: Arc<dyn Now>,
    validity: Validity,
    balance_fetcher: Arc<dyn BalanceFetching>,
    quote_verification: QuoteVerificationMode,
    #[allow(dead_code)]
    order_execution_simulator: Arc<dyn OrderExecutionSimulating>,
    default_quote_timeout: std::time::Duration,
}

impl OrderQuoter {
    #[expect(clippy::too_many_arguments)]
    pub fn new(
        price_estimator: Arc<dyn PriceEstimating>,
        native_price_estimator: Arc<dyn NativePriceEstimating>,
        gas_estimator: Arc<dyn GasPriceEstimating>,
        storage: Arc<dyn QuoteStoring>,
        validity: Validity,
        balance_fetcher: Arc<dyn BalanceFetching>,
        quote_verification: QuoteVerificationMode,
        order_execution_simulator: Arc<dyn OrderExecutionSimulating>,
        default_quote_timeout: std::time::Duration,
    ) -> Self {
        Self {
            price_estimator,
            native_price_estimator,
            gas_estimator,
            storage,
            now: Arc::new(Utc::now),
            validity,
            balance_fetcher,
            quote_verification,
            order_execution_simulator,
            default_quote_timeout,
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
            } => self.now.now() + self.validity.eip1271_onchain_quote,
            QuoteSigningScheme::PreSign {
                onchain_order: true,
            } => self.now.now() + self.validity.presign_onchain_quote,
            _ => self.now.now() + self.validity.standard_quote,
        };

        let trade_query = Arc::new(parameters.to_price_query(self.default_quote_timeout));
        let (effective_gas_price, trade_estimate, sell_token_price, _) = futures::try_join!(
            self.gas_estimator
                .effective_gas_price()
                .map_err(|err| CalculateQuoteError::from((
                    EstimatorKind::Gas,
                    PriceEstimationError::ProtocolInternal(err)
                ))),
            self.price_estimator
                .estimate(trade_query.clone())
                .map_err(|err| (EstimatorKind::Regular, err).into()),
            self.native_price_estimator
                .estimate_native_price(parameters.sell_token, trade_query.timeout)
                .map_err(|err| (EstimatorKind::NativeSell, err).into()),
            // We don't care about the native price of the buy_token for the quote but we need it
            // when we build the auction. To prevent creating orders which we can't settle later on
            // we make the native buy_token price a requirement here as well.
            self.native_price_estimator
                .estimate_native_price(parameters.buy_token, trade_query.timeout)
                .map_err(|err| (EstimatorKind::NativeBuy, err).into()),
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
            gas_price: effective_gas_price as f64,
            sell_token_price,
        };

        self.verify_quote(&trade_estimate, parameters, quoted_sell_amount)
            .await?;

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
            verified: trade_estimate.verified,
            metadata: QuoteMetadataV1 {
                interactions: trade_estimate.execution.interactions,
                pre_interactions: trade_estimate.execution.pre_interactions,
                jit_orders: trade_estimate.execution.jit_orders,
            }
            .into(),
        };

        Ok(quote)
    }

    /// Makes sure a quote was verified according to the configured rule.
    async fn verify_quote(
        &self,
        estimate: &Estimate,
        parameters: &QuoteParameters,
        sell_amount: U256,
    ) -> Result<(), CalculateQuoteError> {
        if estimate.verified
            || !matches!(
                self.quote_verification,
                QuoteVerificationMode::EnforceWhenPossible
            )
        {
            // verification was successful or not strictly required
            return Ok(());
        }

        let balance = match self
            .get_balance(&parameters.verification, parameters.sell_token)
            .await
        {
            Ok(balance) => balance,
            Err(err) => {
                tracing::warn!(?err, "could not fetch balance for verification");
                return Err(CalculateQuoteError::QuoteNotVerified);
            }
        };

        if balance >= sell_amount {
            // Quote could not be verified although user has the required balance.
            // This likely indicates a weird token that solvers are not able to handle.
            return Err(CalculateQuoteError::QuoteNotVerified);
        }

        Ok(())
    }

    async fn get_balance(&self, verification: &Verification, token: Address) -> Result<U256> {
        let query = Query {
            owner: verification.from,
            token,
            source: verification.sell_token_source,
            interactions: verification
                .pre_interactions
                .iter()
                .map(|i| InteractionData {
                    target: i.target,
                    value: i.value,
                    call_data: i.data.clone(),
                })
                .collect(),
            // quote verification already tries to auto-fake missing balances
            balance_override: None,
        };
        let mut balances = self.balance_fetcher.get_balances(&[query]).await;
        balances.pop().context("missing balance result")?
    }
}

#[async_trait::async_trait]
impl OrderQuoting for OrderQuoter {
    #[instrument(skip_all)]
    async fn calculate_quote(
        &self,
        parameters: QuoteParameters,
    ) -> Result<Quote, CalculateQuoteError> {
        let data = self.compute_quote_data(&parameters).await?;
        let mut quote =
            Quote::new(Default::default(), data).with_additional_cost(parameters.additional_cost());

        // Make sure to scale the sell and buy amounts for quotes for sell
        // amounts before fees.
        if let OrderQuoteSide::Sell {
            sell_amount:
                SellAmount::BeforeFee {
                    value: sell_amount_before_fee,
                },
        } = &parameters.side
        {
            let sell_amount = sell_amount_before_fee
                .get()
                .saturating_sub(quote.fee_amount);
            if sell_amount.is_zero() {
                // We want a sell_amount of at least 1!
                return Err(CalculateQuoteError::SellAmountDoesNotCoverFee {
                    fee_amount: quote.fee_amount,
                });
            }

            quote = quote.with_scaled_sell_amount(sell_amount);
        }

        tracing::debug!(?quote, "computed quote");
        Ok(quote)
    }

    #[instrument(skip_all)]
    async fn store_quote(&self, quote: Quote) -> Result<Quote> {
        let id = self.storage.save(quote.data.clone()).await?;
        Ok(Quote {
            id: Some(id),
            ..quote
        })
    }

    #[instrument(skip_all)]
    async fn find_quote(
        &self,
        id: Option<QuoteId>,
        parameters: QuoteSearchParameters,
    ) -> Result<Quote, FindQuoteError> {
        let scaled_sell_amount = match parameters.kind {
            OrderKind::Sell => Some(parameters.sell_amount),
            OrderKind::Buy => None,
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
                        return Err(FindQuoteError::ParameterMismatch(Box::new(data)));
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
        }
        .await?
        .with_additional_cost(additional_cost);

        let quote = match scaled_sell_amount {
            Some(sell_amount) => quote.with_scaled_sell_amount(sell_amount),
            None => quote,
        };

        tracing::debug!(?quote, "found quote");
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
            partially_fillable: false,
            buy_token_balance: quote_request.buy_token_balance,
            sell_token_balance: quote_request.sell_token_balance,
            signing_scheme: quote_request.signing_scheme.into(),
            class: OrderClass::Market,
            kind: match quote_request.side {
                OrderQuoteSide::Buy { .. } => OrderKind::Buy,
                OrderQuoteSide::Sell { .. } => OrderKind::Sell,
            },
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

/// Used to store quote metadata in the database.
/// Versioning is used for the backward compatibility.
/// In case new metadata needs to be associated with a quote create a new
/// variant version and apply serde rename attribute with proper number.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", tag = "version")]
pub enum QuoteMetadata {
    #[serde(rename = "1.0")]
    V1(QuoteMetadataV1),
}

// Handles deserialization of empty json value {} in metadata column.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
#[serde(untagged)]
enum QuoteMetadataDeserializationHelper {
    Data(QuoteMetadata),
    Empty {},
}

impl TryInto<serde_json::Value> for QuoteMetadata {
    type Error = serde_json::Error;

    fn try_into(self) -> std::result::Result<serde_json::Value, Self::Error> {
        serde_json::to_value(self)
    }
}

impl TryFrom<serde_json::Value> for QuoteMetadata {
    type Error = serde_json::Error;

    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        Ok(match serde_json::from_value(value)? {
            QuoteMetadataDeserializationHelper::Data(value) => value,
            QuoteMetadataDeserializationHelper::Empty {} => Default::default(),
        })
    }
}

impl Default for QuoteMetadata {
    fn default() -> Self {
        Self::V1(Default::default())
    }
}

impl From<QuoteMetadataV1> for QuoteMetadata {
    fn from(val: QuoteMetadataV1) -> Self {
        QuoteMetadata::V1(val)
    }
}

#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteMetadataV1 {
    /// Data provided by the solver in response to /quote request.
    pub interactions: Vec<InteractionData>,
    /// The onchain calls to run before sending user funds to the settlement
    /// contract.
    pub pre_interactions: Vec<InteractionData>,
    /// Orders that were settled outside of the auction.
    pub jit_orders: Vec<dto::JitOrder>,
}

#[cfg(test)]
mod tests {
    use crate::order_simulation::SimulationOptions;

    use {
        super::*,
        crate::{
            account_balances::MockBalanceFetching,
            gas_price_estimation::FakeGasPriceEstimator,
            price_estimation::{
                HEALTHY_PRICE_ESTIMATION_TIME, MockPriceEstimating,
                native::MockNativePriceEstimating,
            },
        },
        Address, U256 as AlloyU256,
        alloy::eips::eip1559::Eip1559Estimation,
        chrono::Utc,
        futures::FutureExt,
        mockall::{Sequence, predicate::eq},
        model::time,
        number::nonzero::NonZeroU256,
    };

    struct FakeOrderExecutionSimulator;
    #[async_trait::async_trait]
    impl OrderExecutionSimulating for FakeOrderExecutionSimulator {
        async fn simulate_order_execution(
            &self,
            _: &model::order::Order,
            _: &model::DomainSeparator,
            _: SimulationOptions,
        ) -> Result<()> {
            Ok(())
        }
    }

    fn mock_balance_fetcher() -> Arc<dyn BalanceFetching> {
        let mut mock = MockBalanceFetching::new();
        mock.expect_get_balances()
            .returning(|addresses| addresses.iter().map(|_| Ok(U256::MAX)).collect());
        Arc::new(mock)
    }

    #[test]
    fn pre_order_data_from_quote_request() {
        let quote_request = OrderQuoteRequest {
            validity: model::quote::Validity::To(0),
            ..Default::default()
        };
        let result = PreOrderData::from(&quote_request);
        let expected = PreOrderData::default();
        assert_eq!(result, expected);
    }

    #[test]
    fn pre_order_data_from_quote_request_with_valid_for() {
        let quote_request = OrderQuoteRequest {
            validity: model::quote::Validity::For(100),
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
            sell_token: Address::from([1; 20]),
            buy_token: Address::from([2; 20]),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: NonZeroU256::try_from(100).unwrap(),
                },
            },
            verification: Verification {
                from: Address::from([3; 20]),
                ..Default::default()
            },
            signing_scheme: QuoteSigningScheme::Eip712,
            additional_gas: 0,
            timeout: None,
        };
        let gas_price = Eip1559Estimation {
            max_fee_per_gas: 2,
            max_priority_fee_per_gas: 1,
        };

        let mut price_estimator = MockPriceEstimating::new();
        price_estimator
            .expect_estimate()
            .withf(|q| {
                **q == price_estimation::Query {
                    verification: Verification {
                        from: Address::from([3; 20]),
                        ..Default::default()
                    },
                    sell_token: Address::repeat_byte(1),
                    buy_token: Address::new([2; 20]),
                    in_amount: NonZeroU256::try_from(100).unwrap(),
                    kind: OrderKind::Sell,
                    block_dependent: true,
                    timeout: HEALTHY_PRICE_ESTIMATION_TIME,
                }
            })
            .returning(|_| {
                async {
                    Ok(price_estimation::Estimate {
                        out_amount: AlloyU256::from(42),
                        gas: 3,
                        solver: Address::repeat_byte(1),
                        verified: false,
                        execution: Default::default(),
                    })
                }
                .boxed()
            });

        let mut native_price_estimator = MockNativePriceEstimating::new();
        native_price_estimator
            .expect_estimate_native_price()
            .withf({
                let sell_token = parameters.sell_token;
                move |q, _| *q == sell_token
            })
            .returning(|_, _| async { Ok(0.2) }.boxed());
        native_price_estimator
            .expect_estimate_native_price()
            .withf({
                let buy_token = parameters.buy_token;
                move |q, _| *q == buy_token
            })
            .returning(|_, _| async { Ok(0.2) }.boxed());

        let gas_estimator = FakeGasPriceEstimator::new(gas_price);

        let mut storage = MockQuoteStoring::new();
        storage
            .expect_save()
            .with(eq(QuoteData {
                sell_token: Address::repeat_byte(1),
                buy_token: Address::repeat_byte(2),
                quoted_sell_amount: U256::from(100),
                quoted_buy_amount: U256::from(42),
                fee_parameters: FeeParameters {
                    gas_amount: 3.,
                    gas_price: 2.,
                    sell_token_price: 0.2,
                },
                kind: OrderKind::Sell,
                expiration: now + Duration::seconds(60i64),
                quote_kind: QuoteKind::Standard,
                solver: Address::repeat_byte(1),
                verified: false,
                metadata: Default::default(),
            }))
            .returning(|_| Ok(1337));

        let quoter = OrderQuoter {
            price_estimator: Arc::new(price_estimator),
            native_price_estimator: Arc::new(native_price_estimator),
            gas_estimator: Arc::new(gas_estimator),
            storage: Arc::new(storage),
            now: Arc::new(now),
            validity: super::Validity::default(),
            quote_verification: QuoteVerificationMode::Unverified,
            balance_fetcher: mock_balance_fetcher(),
            order_execution_simulator: Arc::new(FakeOrderExecutionSimulator),
            default_quote_timeout: HEALTHY_PRICE_ESTIMATION_TIME,
        };

        let quote = quoter.calculate_quote(parameters).await.unwrap();
        let quote = quoter.store_quote(quote).await.unwrap();

        assert_eq!(
            quote,
            Quote {
                id: Some(1337),
                data: QuoteData {
                    sell_token: Address::repeat_byte(1),
                    buy_token: Address::repeat_byte(2),
                    quoted_sell_amount: U256::from(100),
                    quoted_buy_amount: U256::from(42),
                    fee_parameters: FeeParameters {
                        gas_amount: 3.,
                        gas_price: 2.,
                        sell_token_price: 0.2,
                    },
                    kind: OrderKind::Sell,
                    expiration: now + chrono::Duration::seconds(60i64),
                    quote_kind: QuoteKind::Standard,
                    solver: Address::repeat_byte(1),
                    verified: false,
                    metadata: Default::default(),
                },
                sell_amount: U256::from(70),
                buy_amount: U256::from(29),
                fee_amount: U256::from(30),
            }
        );
    }

    #[tokio::test]
    async fn compute_sell_after_fee_quote() {
        let now = Utc::now();
        let parameters = QuoteParameters {
            sell_token: Address::from([1; 20]),
            buy_token: Address::from([2; 20]),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::AfterFee {
                    value: NonZeroU256::try_from(100).unwrap(),
                },
            },
            verification: Verification {
                from: Address::from([3; 20]),
                ..Default::default()
            },
            signing_scheme: QuoteSigningScheme::Eip1271 {
                onchain_order: false,
                verification_gas_limit: 1,
            },
            additional_gas: 2,
            timeout: None,
        };
        let gas_price = Eip1559Estimation {
            max_fee_per_gas: 2,
            max_priority_fee_per_gas: 1,
        };

        let mut price_estimator = MockPriceEstimating::new();
        price_estimator
            .expect_estimate()
            .withf(|q| {
                **q == price_estimation::Query {
                    verification: Verification {
                        from: Address::from([3; 20]),
                        ..Default::default()
                    },
                    sell_token: Address::repeat_byte(1),
                    buy_token: Address::new([2; 20]),
                    in_amount: NonZeroU256::try_from(100).unwrap(),
                    kind: OrderKind::Sell,
                    block_dependent: true,
                    timeout: HEALTHY_PRICE_ESTIMATION_TIME,
                }
            })
            .returning(|_| {
                async {
                    Ok(price_estimation::Estimate {
                        out_amount: AlloyU256::from(42),
                        gas: 3,
                        solver: Address::repeat_byte(1),
                        verified: false,
                        execution: Default::default(),
                    })
                }
                .boxed()
            });

        let mut native_price_estimator = MockNativePriceEstimating::new();
        native_price_estimator
            .expect_estimate_native_price()
            .withf({
                let sell_token = parameters.sell_token;
                move |q, _| *q == sell_token
            })
            .returning(|_, _| async { Ok(0.2) }.boxed());
        native_price_estimator
            .expect_estimate_native_price()
            .withf({
                let buy_token = parameters.buy_token;
                move |q, _| *q == buy_token
            })
            .returning(|_, _| async { Ok(0.2) }.boxed());

        let gas_estimator = FakeGasPriceEstimator::new(gas_price);

        let mut storage = MockQuoteStoring::new();
        storage
            .expect_save()
            .with(eq(QuoteData {
                sell_token: Address::repeat_byte(1),
                buy_token: Address::repeat_byte(2),
                quoted_sell_amount: U256::from(100),
                quoted_buy_amount: U256::from(42),
                fee_parameters: FeeParameters {
                    gas_amount: 3.,
                    gas_price: 2.,
                    sell_token_price: 0.2,
                },
                kind: OrderKind::Sell,
                expiration: now + chrono::Duration::seconds(60i64),
                quote_kind: QuoteKind::Standard,
                solver: Address::repeat_byte(1),
                verified: false,
                metadata: Default::default(),
            }))
            .returning(|_| Ok(1337));

        let quoter = OrderQuoter {
            price_estimator: Arc::new(price_estimator),
            native_price_estimator: Arc::new(native_price_estimator),
            gas_estimator: Arc::new(gas_estimator),
            storage: Arc::new(storage),
            now: Arc::new(now),
            validity: Validity::default(),
            quote_verification: QuoteVerificationMode::Unverified,
            balance_fetcher: mock_balance_fetcher(),
            order_execution_simulator: Arc::new(FakeOrderExecutionSimulator),
            default_quote_timeout: HEALTHY_PRICE_ESTIMATION_TIME,
        };

        let quote = quoter.calculate_quote(parameters).await.unwrap();
        let quote = quoter.store_quote(quote).await.unwrap();

        assert_eq!(
            quote,
            Quote {
                id: Some(1337),
                data: QuoteData {
                    sell_token: Address::repeat_byte(1),
                    buy_token: Address::repeat_byte(2),
                    quoted_sell_amount: U256::from(100),
                    quoted_buy_amount: U256::from(42),
                    fee_parameters: FeeParameters {
                        gas_amount: 3.,
                        gas_price: 2.,
                        sell_token_price: 0.2,
                    },
                    kind: OrderKind::Sell,
                    expiration: now + chrono::Duration::seconds(60i64),
                    quote_kind: QuoteKind::Standard,
                    solver: Address::repeat_byte(1),
                    verified: false,
                    metadata: Default::default(),
                },
                sell_amount: U256::from(100),
                buy_amount: U256::from(42),
                fee_amount: U256::from(60),
            }
        );
    }

    #[tokio::test]
    async fn compute_buy_quote() {
        let now = Utc::now();
        let parameters = QuoteParameters {
            sell_token: Address::from([1; 20]),
            buy_token: Address::from([2; 20]),
            side: OrderQuoteSide::Buy {
                buy_amount_after_fee: NonZeroU256::try_from(42).unwrap(),
            },
            verification: Verification {
                from: Address::from([3; 20]),
                ..Default::default()
            },
            signing_scheme: QuoteSigningScheme::Eip712,
            additional_gas: 0,
            timeout: None,
        };
        let gas_price = Eip1559Estimation {
            max_fee_per_gas: 2,
            max_priority_fee_per_gas: 1,
        };

        let mut price_estimator = MockPriceEstimating::new();
        price_estimator
            .expect_estimate()
            .withf(|q| {
                **q == price_estimation::Query {
                    verification: Verification {
                        from: Address::from([3; 20]),
                        ..Default::default()
                    },
                    sell_token: Address::repeat_byte(1),
                    buy_token: Address::new([2; 20]),
                    in_amount: NonZeroU256::try_from(42).unwrap(),
                    kind: OrderKind::Buy,
                    block_dependent: true,
                    timeout: HEALTHY_PRICE_ESTIMATION_TIME,
                }
            })
            .returning(|_| {
                async {
                    Ok(price_estimation::Estimate {
                        out_amount: AlloyU256::from(100),
                        gas: 3,
                        solver: Address::repeat_byte(1),
                        verified: false,
                        execution: Default::default(),
                    })
                }
                .boxed()
            });

        let mut native_price_estimator = MockNativePriceEstimating::new();
        native_price_estimator
            .expect_estimate_native_price()
            .withf({
                let sell_token = parameters.sell_token;
                move |q, _| *q == sell_token
            })
            .returning(|_, _| async { Ok(0.2) }.boxed());
        native_price_estimator
            .expect_estimate_native_price()
            .withf({
                let buy_token = parameters.buy_token;
                move |q, _| *q == buy_token
            })
            .returning(|_, _| async { Ok(0.2) }.boxed());

        let gas_estimator = FakeGasPriceEstimator::new(gas_price);

        let mut storage = MockQuoteStoring::new();
        storage
            .expect_save()
            .with(eq(QuoteData {
                sell_token: Address::repeat_byte(1),
                buy_token: Address::repeat_byte(2),
                quoted_sell_amount: U256::from(100),
                quoted_buy_amount: U256::from(42),
                fee_parameters: FeeParameters {
                    gas_amount: 3.,
                    gas_price: 2.,
                    sell_token_price: 0.2,
                },
                kind: OrderKind::Buy,
                expiration: now + chrono::Duration::seconds(60i64),
                quote_kind: QuoteKind::Standard,
                solver: Address::repeat_byte(1),
                verified: false,
                metadata: Default::default(),
            }))
            .returning(|_| Ok(1337));

        let quoter = OrderQuoter {
            price_estimator: Arc::new(price_estimator),
            native_price_estimator: Arc::new(native_price_estimator),
            gas_estimator: Arc::new(gas_estimator),
            storage: Arc::new(storage),
            now: Arc::new(now),
            validity: Validity::default(),
            quote_verification: QuoteVerificationMode::Unverified,
            balance_fetcher: mock_balance_fetcher(),
            order_execution_simulator: Arc::new(FakeOrderExecutionSimulator),
            default_quote_timeout: HEALTHY_PRICE_ESTIMATION_TIME,
        };

        let quote = quoter.calculate_quote(parameters).await.unwrap();
        let quote = quoter.store_quote(quote).await.unwrap();

        assert_eq!(
            quote,
            Quote {
                id: Some(1337),
                data: QuoteData {
                    sell_token: Address::repeat_byte(1),
                    buy_token: Address::repeat_byte(2),
                    quoted_sell_amount: U256::from(100),
                    quoted_buy_amount: U256::from(42),
                    fee_parameters: FeeParameters {
                        gas_amount: 3.,
                        gas_price: 2.,
                        sell_token_price: 0.2,
                    },
                    kind: OrderKind::Buy,
                    expiration: now + chrono::Duration::seconds(60i64),
                    quote_kind: QuoteKind::Standard,
                    solver: Address::repeat_byte(1),
                    verified: false,
                    metadata: Default::default(),
                },
                sell_amount: U256::from(100),
                buy_amount: U256::from(42),
                fee_amount: U256::from(30),
            }
        );
    }

    #[tokio::test]
    async fn compute_sell_before_fee_quote_insufficient_amount_error() {
        let parameters = QuoteParameters {
            sell_token: Address::from([1; 20]),
            buy_token: Address::from([2; 20]),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: NonZeroU256::try_from(100).unwrap(),
                },
            },
            verification: Verification {
                from: Address::from([3; 20]),
                ..Default::default()
            },
            signing_scheme: QuoteSigningScheme::Eip712,
            additional_gas: 0,
            timeout: None,
        };
        let gas_price = Eip1559Estimation {
            max_fee_per_gas: 1,
            max_priority_fee_per_gas: 0,
        };

        let mut price_estimator = MockPriceEstimating::new();
        price_estimator.expect_estimate().returning(|_| {
            async {
                Ok(price_estimation::Estimate {
                    out_amount: AlloyU256::from(100),
                    gas: 200,
                    solver: Address::repeat_byte(1),
                    verified: false,
                    execution: Default::default(),
                })
            }
            .boxed()
        });

        let mut native_price_estimator = MockNativePriceEstimating::new();
        native_price_estimator
            .expect_estimate_native_price()
            .withf({
                let sell_token = parameters.sell_token;
                move |q, _| *q == sell_token
            })
            .returning(|_, _| async { Ok(1.) }.boxed());
        native_price_estimator
            .expect_estimate_native_price()
            .withf({
                let buy_token = parameters.buy_token;
                move |q, _| *q == buy_token
            })
            .returning(|_, _| async { Ok(1.) }.boxed());

        let gas_estimator = FakeGasPriceEstimator::new(gas_price);

        let quoter = OrderQuoter {
            price_estimator: Arc::new(price_estimator),
            native_price_estimator: Arc::new(native_price_estimator),
            gas_estimator: Arc::new(gas_estimator),
            storage: Arc::new(MockQuoteStoring::new()),
            now: Arc::new(Utc::now),
            validity: Validity::default(),
            quote_verification: QuoteVerificationMode::Unverified,
            balance_fetcher: mock_balance_fetcher(),
            order_execution_simulator: Arc::new(FakeOrderExecutionSimulator),
            default_quote_timeout: HEALTHY_PRICE_ESTIMATION_TIME,
        };

        assert!(matches!(
            quoter.calculate_quote(parameters).await.unwrap_err(),
            CalculateQuoteError::SellAmountDoesNotCoverFee { fee_amount } if fee_amount == U256::from(200),
        ));
    }

    #[tokio::test]
    async fn require_native_price_for_buy_token() {
        let parameters = QuoteParameters {
            sell_token: Address::from([1; 20]),
            buy_token: Address::from([2; 20]),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: NonZeroU256::try_from(100_000).unwrap(),
                },
            },
            verification: Verification {
                from: Address::from([3; 20]),
                ..Default::default()
            },
            signing_scheme: QuoteSigningScheme::Eip712,
            additional_gas: 0,
            timeout: None,
        };
        let gas_price = Eip1559Estimation {
            max_fee_per_gas: 2,
            max_priority_fee_per_gas: 0,
        };

        let mut price_estimator = MockPriceEstimating::new();
        price_estimator.expect_estimate().returning(|_| {
            async {
                Ok(price_estimation::Estimate {
                    out_amount: AlloyU256::from(100),
                    gas: 200,
                    solver: Address::repeat_byte(1),
                    verified: false,
                    execution: Default::default(),
                })
            }
            .boxed()
        });

        let mut native_price_estimator = MockNativePriceEstimating::new();
        native_price_estimator
            .expect_estimate_native_price()
            .withf({
                let sell_token = parameters.sell_token;
                move |q, _| *q == sell_token
            })
            .returning(|_, _| async { Ok(1.) }.boxed());
        native_price_estimator
            .expect_estimate_native_price()
            .withf({
                let buy_token = parameters.buy_token;
                move |q, _| *q == buy_token
            })
            .returning(|_, _| async { Err(PriceEstimationError::NoLiquidity) }.boxed());

        let gas_estimator = FakeGasPriceEstimator::new(gas_price);

        let quoter = OrderQuoter {
            price_estimator: Arc::new(price_estimator),
            native_price_estimator: Arc::new(native_price_estimator),
            gas_estimator: Arc::new(gas_estimator),
            storage: Arc::new(MockQuoteStoring::new()),
            now: Arc::new(Utc::now),
            validity: Validity::default(),
            quote_verification: QuoteVerificationMode::Unverified,
            balance_fetcher: mock_balance_fetcher(),
            order_execution_simulator: Arc::new(FakeOrderExecutionSimulator),
            default_quote_timeout: HEALTHY_PRICE_ESTIMATION_TIME,
        };

        assert!(matches!(
            quoter.calculate_quote(parameters).await.unwrap_err(),
            CalculateQuoteError::Price {
                estimator_kind: EstimatorKind::NativeBuy,
                source: PriceEstimationError::NoLiquidity
            },
        ));
    }

    #[tokio::test]
    async fn finds_quote_by_id() {
        let now = Utc::now();
        let quote_id = 42;
        let parameters = QuoteSearchParameters {
            sell_token: Address::repeat_byte(1),
            buy_token: Address::repeat_byte(2),
            sell_amount: U256::from(85),
            buy_amount: U256::from(40),
            fee_amount: U256::from(15),
            kind: OrderKind::Sell,
            signing_scheme: QuoteSigningScheme::Eip712,
            additional_gas: 0,
            verification: Verification {
                from: Address::from([3; 20]),
                ..Default::default()
            },
        };

        let mut storage = MockQuoteStoring::new();
        storage.expect_get().with(eq(42)).returning(move |_| {
            Ok(Some(QuoteData {
                sell_token: Address::repeat_byte(1),
                buy_token: Address::repeat_byte(2),
                quoted_sell_amount: U256::from(100),
                quoted_buy_amount: U256::from(42),
                fee_parameters: FeeParameters {
                    gas_amount: 3.,
                    gas_price: 2.,
                    sell_token_price: 0.2,
                },
                kind: OrderKind::Sell,
                expiration: now + chrono::Duration::seconds(10),
                quote_kind: QuoteKind::Standard,
                solver: Address::repeat_byte(1),
                verified: false,
                metadata: Default::default(),
            }))
        });

        let quoter = OrderQuoter {
            price_estimator: Arc::new(MockPriceEstimating::new()),
            native_price_estimator: Arc::new(MockNativePriceEstimating::new()),
            gas_estimator: Arc::new(FakeGasPriceEstimator::default()),
            storage: Arc::new(storage),
            now: Arc::new(now),
            validity: Validity::default(),
            quote_verification: QuoteVerificationMode::Unverified,
            balance_fetcher: mock_balance_fetcher(),
            order_execution_simulator: Arc::new(FakeOrderExecutionSimulator),
            default_quote_timeout: HEALTHY_PRICE_ESTIMATION_TIME,
        };

        assert_eq!(
            quoter.find_quote(Some(quote_id), parameters).await.unwrap(),
            Quote {
                id: Some(42),
                data: QuoteData {
                    sell_token: Address::repeat_byte(1),
                    buy_token: Address::repeat_byte(2),
                    quoted_sell_amount: U256::from(100),
                    quoted_buy_amount: U256::from(42),
                    fee_parameters: FeeParameters {
                        gas_amount: 3.,
                        gas_price: 2.,
                        sell_token_price: 0.2,
                    },
                    kind: OrderKind::Sell,
                    expiration: now + chrono::Duration::seconds(10),
                    quote_kind: QuoteKind::Standard,
                    solver: Address::repeat_byte(1),
                    verified: false,
                    metadata: Default::default(),
                },
                sell_amount: U256::from(85),
                // Allows for "out-of-price" buy amounts. This means that order
                // be used for providing liquidity at a premium over current
                // market price.
                buy_amount: U256::from(35),
                fee_amount: U256::from(30),
            }
        );
    }

    #[tokio::test]
    async fn finds_quote_with_sell_amount_after_fee() {
        let now = Utc::now();
        let quote_id = 42;
        let parameters = QuoteSearchParameters {
            sell_token: Address::repeat_byte(1),
            buy_token: Address::repeat_byte(2),
            sell_amount: U256::from(100),
            buy_amount: U256::from(40),
            fee_amount: U256::from(30),
            kind: OrderKind::Sell,
            signing_scheme: QuoteSigningScheme::Eip712,
            additional_gas: 0,
            verification: Verification {
                from: Address::from([3; 20]),
                ..Default::default()
            },
        };

        let mut storage = MockQuoteStoring::new();
        storage.expect_get().with(eq(42)).returning(move |_| {
            Ok(Some(QuoteData {
                sell_token: Address::repeat_byte(1),
                buy_token: Address::repeat_byte(2),
                quoted_sell_amount: U256::from(100),
                quoted_buy_amount: U256::from(42),
                fee_parameters: FeeParameters {
                    gas_amount: 3.,
                    gas_price: 2.,
                    sell_token_price: 0.2,
                },
                kind: OrderKind::Sell,
                expiration: now + chrono::Duration::seconds(10),
                quote_kind: QuoteKind::Standard,
                solver: Address::repeat_byte(1),
                verified: false,
                metadata: Default::default(),
            }))
        });

        let quoter = OrderQuoter {
            price_estimator: Arc::new(MockPriceEstimating::new()),
            native_price_estimator: Arc::new(MockNativePriceEstimating::new()),
            gas_estimator: Arc::new(FakeGasPriceEstimator::default()),
            storage: Arc::new(storage),
            now: Arc::new(now),
            validity: Validity::default(),
            quote_verification: QuoteVerificationMode::Unverified,
            balance_fetcher: mock_balance_fetcher(),
            order_execution_simulator: Arc::new(FakeOrderExecutionSimulator),
            default_quote_timeout: HEALTHY_PRICE_ESTIMATION_TIME,
        };

        assert_eq!(
            quoter.find_quote(Some(quote_id), parameters).await.unwrap(),
            Quote {
                id: Some(42),
                data: QuoteData {
                    sell_token: Address::repeat_byte(1),
                    buy_token: Address::repeat_byte(2),
                    quoted_sell_amount: U256::from(100),
                    quoted_buy_amount: U256::from(42),
                    fee_parameters: FeeParameters {
                        gas_amount: 3.,
                        gas_price: 2.,
                        sell_token_price: 0.2,
                    },
                    kind: OrderKind::Sell,
                    expiration: now + chrono::Duration::seconds(10),
                    quote_kind: QuoteKind::Standard,
                    solver: Address::repeat_byte(1),
                    verified: false,
                    metadata: Default::default(),
                },
                sell_amount: U256::from(100),
                buy_amount: U256::from(42),
                fee_amount: U256::from(30),
            }
        );
    }

    #[tokio::test]
    async fn finds_quote_by_parameters() {
        let now = Utc::now();
        let parameters = QuoteSearchParameters {
            sell_token: Address::repeat_byte(1),
            buy_token: Address::repeat_byte(2),
            sell_amount: U256::from(110),
            buy_amount: U256::from(42),
            fee_amount: U256::from(30),
            kind: OrderKind::Buy,
            signing_scheme: QuoteSigningScheme::Eip712,
            additional_gas: 0,
            verification: Verification {
                from: Address::from([3; 20]),
                ..Default::default()
            },
        };

        let mut storage = MockQuoteStoring::new();
        storage
            .expect_find()
            .with(eq(parameters.clone()), eq(now))
            .returning(move |_, _| {
                Ok(Some((
                    42,
                    QuoteData {
                        sell_token: Address::repeat_byte(1),
                        buy_token: Address::repeat_byte(2),
                        quoted_sell_amount: U256::from(100),
                        quoted_buy_amount: U256::from(42),
                        fee_parameters: FeeParameters {
                            gas_amount: 3.,
                            gas_price: 2.,
                            sell_token_price: 0.2,
                        },
                        kind: OrderKind::Buy,
                        expiration: now + chrono::Duration::seconds(10),
                        quote_kind: QuoteKind::Standard,
                        solver: Address::repeat_byte(1),
                        verified: false,
                        metadata: Default::default(),
                    },
                )))
            });

        let quoter = OrderQuoter {
            price_estimator: Arc::new(MockPriceEstimating::new()),
            native_price_estimator: Arc::new(MockNativePriceEstimating::new()),
            gas_estimator: Arc::new(FakeGasPriceEstimator::default()),
            storage: Arc::new(storage),
            now: Arc::new(now),
            validity: Validity::default(),
            quote_verification: QuoteVerificationMode::Unverified,
            balance_fetcher: mock_balance_fetcher(),
            order_execution_simulator: Arc::new(FakeOrderExecutionSimulator),
            default_quote_timeout: HEALTHY_PRICE_ESTIMATION_TIME,
        };

        assert_eq!(
            quoter.find_quote(None, parameters).await.unwrap(),
            Quote {
                id: Some(42),
                data: QuoteData {
                    sell_token: Address::repeat_byte(1),
                    buy_token: Address::repeat_byte(2),
                    quoted_sell_amount: U256::from(100),
                    quoted_buy_amount: U256::from(42),
                    fee_parameters: FeeParameters {
                        gas_amount: 3.,
                        gas_price: 2.,
                        sell_token_price: 0.2,
                    },
                    kind: OrderKind::Buy,
                    expiration: now + chrono::Duration::seconds(10),
                    quote_kind: QuoteKind::Standard,
                    solver: Address::repeat_byte(1),
                    verified: false,
                    metadata: Default::default(),
                },
                sell_amount: U256::from(100),
                buy_amount: U256::from(42),
                fee_amount: U256::from(30),
            }
        );
    }

    #[tokio::test]
    async fn find_invalid_quote_error() {
        let now = Utc::now();
        let parameters = QuoteSearchParameters {
            sell_token: Address::repeat_byte(1),
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
                    sell_token: Address::repeat_byte(2),
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
                    sell_token: Address::repeat_byte(1),
                    expiration: now - chrono::Duration::seconds(1),
                    ..Default::default()
                }))
            });

        let quoter = OrderQuoter {
            price_estimator: Arc::new(MockPriceEstimating::new()),
            native_price_estimator: Arc::new(MockNativePriceEstimating::new()),
            gas_estimator: Arc::new(FakeGasPriceEstimator::default()),
            storage: Arc::new(storage),
            now: Arc::new(now),
            validity: Validity::default(),
            quote_verification: QuoteVerificationMode::Unverified,
            balance_fetcher: mock_balance_fetcher(),
            order_execution_simulator: Arc::new(FakeOrderExecutionSimulator),
            default_quote_timeout: HEALTHY_PRICE_ESTIMATION_TIME,
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
            storage: Arc::new(storage),
            now: Arc::new(Utc::now),
            validity: Validity::default(),
            quote_verification: QuoteVerificationMode::Unverified,
            balance_fetcher: mock_balance_fetcher(),
            order_execution_simulator: Arc::new(FakeOrderExecutionSimulator),
            default_quote_timeout: HEALTHY_PRICE_ESTIMATION_TIME,
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

    #[test]
    fn check_quote_metadata_format() {
        let q: QuoteMetadata = QuoteMetadataV1 {
            interactions: vec![
                InteractionData {
                    target: Address::from_slice(&[1; 20]),
                    value: U256::ONE,
                    call_data: vec![1],
                },
                InteractionData {
                    target: Address::from_slice(&[2; 20]),
                    value: U256::from(2),
                    call_data: vec![2],
                },
            ],
            pre_interactions: vec![
                InteractionData {
                    target: Address::from_slice(&[3; 20]),
                    value: U256::from(3),
                    call_data: vec![3],
                },
                InteractionData {
                    target: Address::from_slice(&[4; 20]),
                    value: U256::from(4),
                    call_data: vec![4],
                },
            ],
            jit_orders: vec![dto::JitOrder {
                buy_token: Address::repeat_byte(4),
                sell_token: Address::repeat_byte(5),
                sell_amount: U256::from(10),
                buy_amount: U256::from(20),
                executed_amount: U256::from(11),
                receiver: Address::repeat_byte(6),
                valid_to: 1734084318,
                app_data: Default::default(),
                side: dto::Side::Sell,
                partially_fillable: false,
                sell_token_source: model::order::SellTokenSource::External,
                buy_token_destination: model::order::BuyTokenDestination::Internal,
                signature: vec![1; 16],
                signing_scheme: model::signature::SigningScheme::Eip712,
            }],
        }
        .into();
        let v = serde_json::to_value(q).unwrap();

        let req: serde_json::Value = serde_json::from_str(
            r#"
        {"version":"1.0",
         "interactions":[
         {"target":"0x0101010101010101010101010101010101010101","value":"1","callData":"0x01"},
         {"target":"0x0202020202020202020202020202020202020202","value":"2","callData":"0x02"}],
         "preInteractions":[
         {"target":"0x0303030303030303030303030303030303030303","value":"3","callData":"0x03"},
         {"target":"0x0404040404040404040404040404040404040404","value":"4","callData":"0x04"}],
         "jitOrders":[{"appData": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "buyAmount": "20", "buyToken": "0x0404040404040404040404040404040404040404", "buyTokenDestination": "internal",
            "executedAmount": "11", "partiallyFillable": false, "receiver": "0x0606060606060606060606060606060606060606",
            "sellAmount": "10", "sellToken": "0x0505050505050505050505050505050505050505", "sellTokenSource": "external",
            "side": "sell", "signature": "0x01010101010101010101010101010101", "signingScheme": "eip712", "validTo": 1734084318}]
        }"#,
        )
        .unwrap();

        assert_eq!(req, v);
    }

    #[test]
    fn check_quote_metadata_deserialize_from_empty_json() {
        let empty_json: serde_json::Value = serde_json::from_str("{}").unwrap();
        let metadata: QuoteMetadata = empty_json.try_into().unwrap();
        // Empty json is converted to QuoteMetadata default value
        assert_eq!(metadata, QuoteMetadata::default());
    }

    #[test]
    fn check_quote_metadata_deserialize_from_v1_json() {
        let v1: serde_json::Value = serde_json::from_str(
            r#"
        {"version":"1.0",
        "interactions":[
        {"target":"0x0101010101010101010101010101010101010101","value":"1","callData":"0x01"},
        {"target":"0x0202020202020202020202020202020202020202","value":"2","callData":"0x02"}],
        "preInteractions":[
        {"target":"0x0303030303030303030303030303030303030303","value":"3","callData":"0x03"},
        {"target":"0x0404040404040404040404040404040404040404","value":"4","callData":"0x04"}],
        "jitOrders":[{"appData": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "buyAmount": "20", "buyToken": "0x0404040404040404040404040404040404040404", "buyTokenDestination": "internal",
            "executedAmount": "11", "partiallyFillable": false, "receiver": "0x0606060606060606060606060606060606060606",
            "sellAmount": "10", "sellToken": "0x0505050505050505050505050505050505050505", "sellTokenSource": "external",
            "side": "sell", "signature": "0x01010101010101010101010101010101", "signingScheme": "eip712", "validTo": 1734084318},
            {"appData": "0x0ddeb6e4a814908832cc25d11311c514e7efe6af3c9bafeb0d241129cf7f4d83",
            "buyAmount": "100", "buyToken": "0x0606060606060606060606060606060606060606", "buyTokenDestination": "erc20",
            "executedAmount": "99", "partiallyFillable": true, "receiver": "0x0303030303030303030303030303030303030303",
            "sellAmount": "10", "sellToken": "0x0101010101010101010101010101010101010101", "sellTokenSource": "erc20",
            "side": "buy", "signature": "0x01010101010101010101010101010101", "signingScheme": "eip1271", "validTo": 1734085109}]
        }"#,
        )
        .unwrap();
        let metadata: QuoteMetadata = v1.try_into().unwrap();

        match metadata {
            QuoteMetadata::V1(v1) => {
                assert_eq!(v1.interactions.len(), 2);
                assert_eq!(v1.pre_interactions.len(), 2);
                assert_eq!(v1.jit_orders.len(), 2);
            }
        }
    }
}
