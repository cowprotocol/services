use {
    crate::{
        account_balances::{self, BalanceFetching, TransferSimulationError},
        app_data::ValidatedAppData,
        bad_token::{BadTokenDetecting, TokenQuality},
        code_fetching::CodeFetching,
        order_quoting::{
            CalculateQuoteError,
            FindQuoteError,
            OrderQuoting,
            Quote,
            QuoteParameters,
            QuoteSearchParameters,
        },
        price_estimation::{PriceEstimationError, Verification},
        signature_validator::{SignatureCheck, SignatureValidating, SignatureValidationError},
        trade_finding,
    },
    anyhow::{anyhow, Result},
    async_trait::async_trait,
    chrono::Utc,
    contracts::{HooksTrampoline, WETH9},
    database::onchain_broadcasted_orders::OnchainOrderPlacementError,
    ethcontract::{Bytes, H160, H256, U256},
    model::{
        app_data::AppDataHash,
        interaction::InteractionData,
        order::{
            AppdataFromMismatch,
            BuyTokenDestination,
            Hook,
            Hooks,
            Interactions,
            Order,
            OrderClass,
            OrderCreation,
            OrderCreationAppData,
            OrderData,
            OrderKind,
            OrderMetadata,
            SellTokenSource,
            VerificationError,
            BUY_ETH_ADDRESS,
        },
        quote::{OrderQuoteSide, QuoteSigningScheme, SellAmount},
        signature::{self, hashed_eip712_message, Signature, SigningScheme},
        time,
        DomainSeparator,
    },
    std::{collections::HashSet, sync::Arc, time::Duration},
};

#[mockall::automock]
#[async_trait::async_trait]
pub trait OrderValidating: Send + Sync {
    /// Partial (aka Pre-) Validation is aimed at catching malformed order data
    /// during the fee & quote phase (i.e. before the order is signed).
    /// Thus, partial validation *doesn't* verify:
    ///     - signatures
    ///     - user sell balances or fee sufficiency.
    ///
    /// Specifically, but *does* verify:
    ///     - if buy token is native asset, receiver is not a smart contract,
    ///     - the sell token is not the native asset,
    ///     - the sender is not a banned user,
    ///     - the order validity is appropriate,
    ///     - buy_token is not the same as sell_token,
    ///     - buy and sell token destination and source are supported.
    ///     - buy & sell tokens passed "bad token" detection,
    async fn partial_validate(&self, order: PreOrderData) -> Result<(), PartialValidationError>;

    /// This validates an order's app-data and returns the parsed
    /// `ProtocolAppData` value along with the corresponding rendered
    /// interactions that were specified in the `app_data`.
    fn validate_app_data(
        &self,
        app_data: &OrderCreationAppData,
        full_app_data_override: &Option<String>,
    ) -> Result<OrderAppData, AppDataValidationError>;

    /// This is the full order validation performed at the time of order
    /// placement (i.e. once all the required fields on an Order are
    /// provided). Specifically, verifying that
    ///     - buy & sell amounts are non-zero,
    ///     - order's signature recovers correctly
    ///     - fee is sufficient,
    ///     - user has sufficient (transferable) funds to execute the order.
    ///
    /// Furthermore, full order validation also calls partial_validate to ensure
    /// that other aspects of the order are not malformed.
    ///
    /// `full_app_data_override` can be used when the order specifies only a
    /// contract app data hash without the full app data. In this case
    /// `full_app_data_override` is used as the full app data and the contract
    /// app data hash is not validated against it (the hash doesn't have to
    /// match). The full app data is still otherwise validated.
    async fn validate_and_construct_order(
        &self,
        order: OrderCreation,
        domain_separator: &DomainSeparator,
        settlement_contract: H160,
        full_app_data_override: Option<String>,
    ) -> Result<(Order, Option<Quote>), ValidationError>;
}

#[derive(Debug)]
pub enum PartialValidationError {
    Forbidden,
    ValidTo(OrderValidToError),
    InvalidNativeSellToken,
    SameBuyAndSellToken,
    UnsupportedBuyTokenDestination(BuyTokenDestination),
    UnsupportedSellTokenSource(SellTokenSource),
    UnsupportedOrderType,
    UnsupportedToken { token: H160, reason: String },
    Other(anyhow::Error),
}

impl From<OrderValidToError> for PartialValidationError {
    fn from(err: OrderValidToError) -> Self {
        Self::ValidTo(err)
    }
}

#[derive(Debug)]
pub enum AppDataValidationError {
    Mismatch {
        provided: AppDataHash,
        actual: AppDataHash,
    },
    Invalid(anyhow::Error),
}

#[derive(Debug)]
pub enum ValidationError {
    Partial(PartialValidationError),
    AppData(AppDataValidationError),
    /// The quote ID specified with the order could not be found.
    QuoteNotFound,
    /// The quote specified by ID is invalid. Either it doesn't match the order
    /// or it has already expired.
    InvalidQuote,
    /// Unable to compute quote because of a price estimation error.
    PriceForQuote(PriceEstimationError),
    InsufficientFee,
    /// Orders with positive signed fee amount are deprecated
    NonZeroFee,
    InsufficientBalance,
    InsufficientAllowance,
    InvalidSignature,
    /// If fee and sell amount overflow u256
    SellAmountOverflow,
    TransferSimulationFailed,
    /// The specified on-chain signature requires the from address of the
    /// order signer.
    MissingFrom,
    /// The signer in the appdata metadata does not match the provided from
    /// value.
    AppdataFromMismatch(AppdataFromMismatch),
    WrongOwner(signature::Recovered),
    /// An invalid EIP-1271 signature, where the on-chain validation check
    /// reverted or did not return the expected value.
    InvalidEip1271Signature(H256),
    ZeroAmount,
    IncompatibleSigningScheme,
    TooManyLimitOrders,
    Other(anyhow::Error),
}

impl From<AppDataValidationError> for ValidationError {
    fn from(value: AppDataValidationError) -> Self {
        Self::AppData(value)
    }
}

pub fn onchain_order_placement_error_from(error: ValidationError) -> OnchainOrderPlacementError {
    match error {
        ValidationError::QuoteNotFound => OnchainOrderPlacementError::QuoteNotFound,
        ValidationError::Partial(_) => OnchainOrderPlacementError::PreValidationError,
        ValidationError::InvalidQuote => OnchainOrderPlacementError::InvalidQuote,
        ValidationError::InsufficientFee => OnchainOrderPlacementError::InsufficientFee,
        _ => OnchainOrderPlacementError::Other,
    }
}

impl From<VerificationError> for ValidationError {
    fn from(err: VerificationError) -> Self {
        match err {
            VerificationError::UnableToRecoverSigner(_) => Self::InvalidSignature,
            VerificationError::UnexpectedSigner(recovered) => Self::WrongOwner(recovered),
            VerificationError::MissingFrom => Self::MissingFrom,
            VerificationError::AppdataFromMismatch(mismatch) => Self::AppdataFromMismatch(mismatch),
        }
    }
}

impl From<FindQuoteError> for ValidationError {
    fn from(err: FindQuoteError) -> Self {
        match err {
            FindQuoteError::NotFound(_) => Self::QuoteNotFound,
            FindQuoteError::ParameterMismatch(_) | FindQuoteError::Expired(_) => Self::InvalidQuote,
            FindQuoteError::Other(err) => Self::Other(err),
        }
    }
}

impl From<CalculateQuoteError> for ValidationError {
    fn from(err: CalculateQuoteError) -> Self {
        match err {
            CalculateQuoteError::Price(PriceEstimationError::UnsupportedToken {
                token,
                reason,
            }) => {
                ValidationError::Partial(PartialValidationError::UnsupportedToken { token, reason })
            }
            CalculateQuoteError::Other(err)
            | CalculateQuoteError::Price(PriceEstimationError::ProtocolInternal(err)) => {
                ValidationError::Other(err)
            }
            CalculateQuoteError::Price(err) => ValidationError::PriceForQuote(err),
            // This should never happen because we only calculate quotes with
            // `SellAmount::AfterFee`, meaning that the sell amount does not
            // need to be higher than the computed fee amount. Don't bubble up
            // and handle these errors in a general way.
            err @ CalculateQuoteError::SellAmountDoesNotCoverFee { .. } => {
                ValidationError::Other(anyhow!(err).context("unexpected quote calculation error"))
            }
        }
    }
}

#[mockall::automock]
#[async_trait]
pub trait LimitOrderCounting: Send + Sync {
    async fn count(&self, owner: H160) -> Result<u64>;
}

#[derive(Clone)]
pub struct OrderValidator {
    /// For Pre/Partial-Validation: performed during fee & quote phase
    /// when only part of the order data is available
    native_token: WETH9,
    banned_users: HashSet<H160>,
    validity_configuration: OrderValidPeriodConfiguration,
    eip1271_skip_creation_validation: bool,
    bad_token_detector: Arc<dyn BadTokenDetecting>,
    hooks: HooksTrampoline,
    /// For Full-Validation: performed time of order placement
    quoter: Arc<dyn OrderQuoting>,
    balance_fetcher: Arc<dyn BalanceFetching>,
    signature_validator: Arc<dyn SignatureValidating>,
    limit_order_counter: Arc<dyn LimitOrderCounting>,
    max_limit_orders_per_user: u64,
    pub code_fetcher: Arc<dyn CodeFetching>,
    app_data_validator: crate::app_data::Validator,
    request_verified_quotes: bool,
    market_orders_deprecation_date: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Eq, PartialEq, Default)]
pub struct PreOrderData {
    pub owner: H160,
    pub sell_token: H160,
    pub buy_token: H160,
    pub receiver: H160,
    pub valid_to: u32,
    pub partially_fillable: bool,
    pub buy_token_balance: BuyTokenDestination,
    pub sell_token_balance: SellTokenSource,
    pub signing_scheme: SigningScheme,
    pub class: OrderClass,
}

fn actual_receiver(owner: H160, order: &OrderData) -> H160 {
    let receiver = order.receiver.unwrap_or_default();
    if receiver == H160::zero() {
        owner
    } else {
        receiver
    }
}

impl PreOrderData {
    pub fn from_order_creation(
        owner: H160,
        order: &OrderData,
        signing_scheme: SigningScheme,
    ) -> Self {
        Self {
            owner,
            sell_token: order.sell_token,
            buy_token: order.buy_token,
            receiver: actual_receiver(owner, order),
            valid_to: order.valid_to,
            partially_fillable: order.partially_fillable,
            buy_token_balance: order.buy_token_balance,
            sell_token_balance: order.sell_token_balance,
            signing_scheme,
            class: match order.fee_amount.is_zero() {
                true => OrderClass::Limit,
                false => OrderClass::Market,
            },
        }
    }
}

pub struct OrderAppData {
    pub inner: ValidatedAppData,
    pub interactions: Interactions,
}

impl OrderValidator {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        native_token: WETH9,
        banned_users: HashSet<H160>,
        validity_configuration: OrderValidPeriodConfiguration,
        eip1271_skip_creation_validation: bool,
        bad_token_detector: Arc<dyn BadTokenDetecting>,
        hooks: HooksTrampoline,
        quoter: Arc<dyn OrderQuoting>,
        balance_fetcher: Arc<dyn BalanceFetching>,
        signature_validator: Arc<dyn SignatureValidating>,
        limit_order_counter: Arc<dyn LimitOrderCounting>,
        max_limit_orders_per_user: u64,
        code_fetcher: Arc<dyn CodeFetching>,
        app_data_validator: crate::app_data::Validator,
        market_orders_deprecation_date: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Self {
        Self {
            native_token,
            banned_users,
            validity_configuration,
            eip1271_skip_creation_validation,
            bad_token_detector,
            hooks,
            quoter,
            balance_fetcher,
            signature_validator,
            limit_order_counter,
            max_limit_orders_per_user,
            code_fetcher,
            app_data_validator,
            request_verified_quotes: false,
            market_orders_deprecation_date,
        }
    }

    pub fn with_verified_quotes(mut self, enable: bool) -> Self {
        self.request_verified_quotes = enable;
        self
    }

    async fn check_max_limit_orders(
        &self,
        owner: H160,
        class: &OrderClass,
    ) -> Result<(), ValidationError> {
        if class.is_limit() {
            let num_limit_orders = self
                .limit_order_counter
                .count(owner)
                .await
                .map_err(ValidationError::Other)?;
            if num_limit_orders >= self.max_limit_orders_per_user {
                return Err(ValidationError::TooManyLimitOrders);
            }
        }
        Ok(())
    }

    fn custom_interactions(&self, hooks: &Hooks) -> Interactions {
        let to_interactions = |hooks: &[Hook]| -> Vec<InteractionData> {
            if hooks.is_empty() {
                vec![]
            } else {
                vec![InteractionData {
                    target: self.hooks.address(),
                    value: U256::zero(),
                    call_data: self
                        .hooks
                        .execute(
                            hooks
                                .iter()
                                .map(|hook| {
                                    (
                                        hook.target,
                                        Bytes(hook.call_data.clone()),
                                        hook.gas_limit.into(),
                                    )
                                })
                                .collect(),
                        )
                        .tx
                        .data
                        .unwrap()
                        .0,
                }]
            }
        };

        Interactions {
            pre: to_interactions(&hooks.pre),
            post: to_interactions(&hooks.post),
        }
    }
}

#[async_trait::async_trait]
impl OrderValidating for OrderValidator {
    async fn partial_validate(&self, order: PreOrderData) -> Result<(), PartialValidationError> {
        if self.banned_users.contains(&order.owner) || self.banned_users.contains(&order.receiver) {
            return Err(PartialValidationError::Forbidden);
        }

        if order.class == OrderClass::Market && order.partially_fillable {
            return Err(PartialValidationError::UnsupportedOrderType);
        }

        if order.buy_token_balance != BuyTokenDestination::Erc20 {
            return Err(PartialValidationError::UnsupportedBuyTokenDestination(
                order.buy_token_balance,
            ));
        }
        if !matches!(
            order.sell_token_balance,
            SellTokenSource::Erc20 | SellTokenSource::External
        ) {
            return Err(PartialValidationError::UnsupportedSellTokenSource(
                order.sell_token_balance,
            ));
        }

        self.validity_configuration.validate_period(&order)?;

        if has_same_buy_and_sell_token(&order, &self.native_token) {
            return Err(PartialValidationError::SameBuyAndSellToken);
        }
        if order.sell_token == BUY_ETH_ADDRESS {
            return Err(PartialValidationError::InvalidNativeSellToken);
        }

        for &token in &[order.sell_token, order.buy_token] {
            if let TokenQuality::Bad { reason } = self
                .bad_token_detector
                .detect(token)
                .await
                .map_err(PartialValidationError::Other)?
            {
                return Err(PartialValidationError::UnsupportedToken { token, reason });
            }
        }

        Ok(())
    }

    fn validate_app_data(
        &self,
        app_data: &OrderCreationAppData,
        full_app_data_override: &Option<String>,
    ) -> Result<OrderAppData, AppDataValidationError> {
        let validate = |app_data: &str| -> Result<_, AppDataValidationError> {
            let app_data = self
                .app_data_validator
                .validate(app_data.as_bytes())
                .map_err(AppDataValidationError::Invalid)?;
            Ok(app_data)
        };

        let app_data = match app_data {
            OrderCreationAppData::Both { full, expected } => {
                let validated = validate(full)?;
                if validated.hash != *expected {
                    return Err(AppDataValidationError::Mismatch {
                        provided: *expected,
                        actual: validated.hash,
                    });
                }
                validated
            }
            OrderCreationAppData::Hash { hash } => {
                // Eventually we're not going to accept orders that set only a
                // hash and where we can't find full app data elsewhere.
                let protocol = if let Some(full) = full_app_data_override {
                    validate(full)?.protocol
                } else {
                    return Err(AppDataValidationError::Invalid(anyhow!(
                        "Unknown pre-image for app data hash {:?}",
                        hash,
                    )));
                };

                ValidatedAppData {
                    hash: *hash,
                    document: String::new(),
                    protocol,
                }
            }
            OrderCreationAppData::Full { full } => validate(full)?,
        };

        let interactions = self.custom_interactions(&app_data.protocol.hooks);

        Ok(OrderAppData {
            inner: app_data,
            interactions,
        })
    }

    async fn validate_and_construct_order(
        &self,
        order: OrderCreation,
        domain_separator: &DomainSeparator,
        settlement_contract: H160,
        full_app_data_override: Option<String>,
    ) -> Result<(Order, Option<Quote>), ValidationError> {
        // Happens before signature verification because a miscalculated app data hash
        // by the API user would lead to being unable to validate the signature below.
        let app_data = self.validate_app_data(&order.app_data, &full_app_data_override)?;
        let app_data_signer = app_data.inner.protocol.signer;

        let owner = order.verify_owner(domain_separator, app_data_signer)?;
        let signing_scheme = order.signature.scheme();
        let data = OrderData {
            app_data: app_data.inner.hash,
            ..order.data()
        };
        let uid = data.uid(domain_separator, &owner);

        let verification_gas_limit = if let Signature::Eip1271(signature) = &order.signature {
            if self.eip1271_skip_creation_validation {
                tracing::debug!(?signature, "skipping EIP-1271 signature validation");
                // We don't care! Because we are skipping validation anyway
                0u64
            } else {
                let hash = hashed_eip712_message(domain_separator, &data.hash_struct());
                self.signature_validator
                    .validate_signature_and_get_additional_gas(SignatureCheck {
                        signer: owner,
                        hash,
                        signature: signature.to_owned(),
                        interactions: app_data.interactions.pre.clone(),
                    })
                    .await
                    .map_err(|err| match err {
                        SignatureValidationError::Invalid => {
                            ValidationError::InvalidEip1271Signature(H256(hash))
                        }
                        SignatureValidationError::Other(err) => ValidationError::Other(err),
                    })?
            }
        } else {
            // in any other case, just apply 0
            0u64
        };

        if data.buy_amount.is_zero() || data.sell_amount.is_zero() {
            return Err(ValidationError::ZeroAmount);
        }

        let pre_order = PreOrderData::from_order_creation(owner, &data, signing_scheme);
        let class = pre_order.class;
        self.partial_validate(pre_order)
            .await
            .map_err(ValidationError::Partial)?;

        let verification = self.request_verified_quotes.then_some(Verification {
            from: owner,
            receiver: order.receiver.unwrap_or(owner),
            sell_token_source: order.sell_token_balance,
            buy_token_destination: order.buy_token_balance,
            pre_interactions: trade_finding::map_interactions(&app_data.interactions.pre),
            post_interactions: trade_finding::map_interactions(&app_data.interactions.post),
        });

        let quote_parameters = QuoteSearchParameters {
            sell_token: data.sell_token,
            buy_token: data.buy_token,
            sell_amount: data.sell_amount,
            buy_amount: data.buy_amount,
            fee_amount: data.fee_amount,
            kind: data.kind,
            signing_scheme: convert_signing_scheme_into_quote_signing_scheme(
                order.signature.scheme(),
                true,
                verification_gas_limit,
            )?,
            additional_gas: app_data.inner.protocol.hooks.gas_limit(),
            verification,
        };
        let quote = match class {
            OrderClass::Market => {
                let fee = Some(data.fee_amount);
                let quote = get_quote_and_check_fee(
                    &*self.quoter,
                    &quote_parameters,
                    order.quote_id,
                    fee,
                    self.market_orders_deprecation_date,
                )
                .await?;
                Some(quote)
            }
            OrderClass::Limit => {
                let quote = get_quote_and_check_fee(
                    &*self.quoter,
                    &quote_parameters,
                    order.quote_id,
                    None,
                    self.market_orders_deprecation_date,
                )
                .await?;
                Some(quote)
            }
            OrderClass::Liquidity => None,
        };

        let min_balance = minimum_balance(&data).ok_or(ValidationError::SellAmountOverflow)?;

        // Fast path to check if transfer is possible with a single node query.
        // If not, run extra queries for additional information.
        match self
            .balance_fetcher
            .can_transfer(
                &account_balances::Query {
                    token: data.sell_token,
                    owner,
                    source: data.sell_token_balance,
                    interactions: app_data.interactions.pre.clone(),
                },
                min_balance,
            )
            .await
        {
            Ok(_) => (),
            Err(
                TransferSimulationError::InsufficientAllowance
                | TransferSimulationError::InsufficientBalance
                | TransferSimulationError::TransferFailed,
            ) if signing_scheme == SigningScheme::PreSign => {
                // We have an exception for pre-sign orders where they do not
                // require sufficient balance or allowance. The idea, is that
                // this allows smart contracts to place orders bundled with
                // other transactions that either produce the required balance
                // or set the allowance. This would, for example, allow a Gnosis
                // Safe to bundle the pre-signature transaction with a WETH wrap
                // and WETH approval to the vault relayer contract.
            }
            Err(err) => match err {
                TransferSimulationError::InsufficientAllowance => {
                    return Err(ValidationError::InsufficientAllowance);
                }
                TransferSimulationError::InsufficientBalance => {
                    return Err(ValidationError::InsufficientBalance);
                }
                TransferSimulationError::TransferFailed => {
                    return Err(ValidationError::TransferSimulationFailed);
                }
                TransferSimulationError::Other(err) => {
                    tracing::warn!("TransferSimulation failed: {:?}", err);
                    return Err(ValidationError::TransferSimulationFailed);
                }
            },
        }

        tracing::debug!(
            ?uid,
            ?order,
            ?quote,
            "checking if order is outside market price"
        );
        // Check if we need to re-classify the market order if it is outside the market
        // price. We consider out-of-price orders as liquidity orders. See
        // <https://github.com/cowprotocol/services/pull/301>.
        let class = match (class, &quote) {
            (OrderClass::Market, Some(quote))
                if is_order_outside_market_price(
                    &Amounts {
                        sell: data.sell_amount,
                        buy: data.buy_amount,
                        fee: data.fee_amount,
                    },
                    &Amounts {
                        sell: quote.sell_amount,
                        buy: quote.buy_amount,
                        fee: quote.fee_amount,
                    },
                ) =>
            {
                tracing::debug!(%uid, ?owner, ?class, "order being flagged as outside market price");
                OrderClass::Liquidity
            }
            (_, _) => class,
        };

        self.check_max_limit_orders(owner, &class).await?;

        let order = Order {
            metadata: OrderMetadata {
                owner,
                creation_date: chrono::offset::Utc::now(),
                uid,
                settlement_contract,
                full_fee_amount: data.fee_amount,
                class,
                full_app_data: match order.app_data {
                    OrderCreationAppData::Both { full, .. }
                    | OrderCreationAppData::Full { full } => Some(full),
                    OrderCreationAppData::Hash { .. } => full_app_data_override,
                },
                ..Default::default()
            },
            signature: order.signature.clone(),
            data,
            interactions: app_data.interactions,
        };

        Ok((order, quote))
    }
}

/// Order validity period configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderValidPeriodConfiguration {
    pub min: Duration,
    pub max_market: Duration,
    pub max_limit: Duration,
}

impl OrderValidPeriodConfiguration {
    /// Creates an configuration where any `validTo` is accepted.
    pub fn any() -> Self {
        Self {
            min: Duration::ZERO,
            max_market: Duration::MAX,
            max_limit: Duration::MAX,
        }
    }

    /// Validates an order's timestamp based on additional data.
    fn validate_period(&self, order: &PreOrderData) -> Result<(), OrderValidToError> {
        let now = time::now_in_epoch_seconds();
        if order.valid_to < time::timestamp_after_duration(now, self.min) {
            return Err(OrderValidToError::Insufficient);
        }
        if order.valid_to > time::timestamp_after_duration(now, self.max(order)) {
            return Err(OrderValidToError::Excessive);
        }

        Ok(())
    }

    /// Returns the maximum valid timestamp for the specified order.
    fn max(&self, order: &PreOrderData) -> Duration {
        // For now, there is no maximum `validTo` for pre-sign orders as a hack
        // for dealing with signature collection times. We should probably
        // revisit this.
        if order.signing_scheme == SigningScheme::PreSign {
            return Duration::MAX;
        }

        match order.class {
            OrderClass::Market => self.max_market,
            OrderClass::Limit => self.max_limit,
            OrderClass::Liquidity => Duration::MAX,
        }
    }
}

#[derive(Debug)]
pub enum OrderValidToError {
    Insufficient,
    Excessive,
}

/// Returns true if the orders have same buy and sell tokens.
///
/// This also checks for orders selling wrapped native token for native token.
fn has_same_buy_and_sell_token(order: &PreOrderData, native_token: &WETH9) -> bool {
    order.sell_token == order.buy_token
        || (order.sell_token == native_token.address() && order.buy_token == BUY_ETH_ADDRESS)
}

/// Min balance user must have in sell token for order to be accepted.
///
/// None when addition overflows.
fn minimum_balance(order: &OrderData) -> Option<U256> {
    // TODO: We might even want to allow 0 balance for partially fillable but we
    // require balance for fok limit orders too so this make some sense and protects
    // against accidentally creating order for token without balance.
    if order.partially_fillable {
        return Some(1.into());
    }
    order.sell_amount.checked_add(order.fee_amount)
}

/// Retrieves the quote for an order that is being created and verify that its
/// fee is sufficient.
///
/// The fee is checked only if `fee_amount` is specified.
pub async fn get_quote_and_check_fee(
    quoter: &dyn OrderQuoting,
    quote_search_parameters: &QuoteSearchParameters,
    quote_id: Option<i64>,
    fee_amount: Option<U256>,
    market_orders_deprecation_date: Option<chrono::DateTime<chrono::Utc>>,
) -> Result<Quote, ValidationError> {
    let quote = get_or_create_quote(quoter, quote_search_parameters, quote_id).await?;

    match market_orders_deprecation_date {
        Some(date) if Utc::now() > date && fee_amount.is_some_and(|fee| !fee.is_zero()) => {
            return Err(ValidationError::NonZeroFee);
        }
        None if fee_amount.is_some_and(|fee| fee < quote.fee_amount) => {
            return Err(ValidationError::InsufficientFee);
        }
        _ => (),
    }

    Ok(quote)
}

/// Retrieves the quote for an order that is being created
///
/// This works by first trying to find an existing quote, and then falling back
/// to calculating a brand new one if none can be found and a quote ID was not
/// specified.
async fn get_or_create_quote(
    quoter: &dyn OrderQuoting,
    quote_search_parameters: &QuoteSearchParameters,
    quote_id: Option<i64>,
) -> Result<Quote, ValidationError> {
    let quote = match quoter
        .find_quote(quote_id, quote_search_parameters.clone())
        .await
    {
        Ok(quote) => {
            tracing::debug!(quote_id =? quote.id, "found quote for order creation");
            quote
        }
        // We couldn't find a quote, and no ID was specified. Try computing a
        // fresh quote to use instead.
        Err(FindQuoteError::NotFound(_)) if quote_id.is_none() => {
            let parameters = QuoteParameters {
                sell_token: quote_search_parameters.sell_token,
                buy_token: quote_search_parameters.buy_token,
                side: match quote_search_parameters.kind {
                    OrderKind::Buy => OrderQuoteSide::Buy {
                        buy_amount_after_fee: quote_search_parameters
                            .buy_amount
                            .try_into()
                            .map_err(|_| ValidationError::ZeroAmount)?,
                    },
                    OrderKind::Sell => OrderQuoteSide::Sell {
                        sell_amount: SellAmount::AfterFee {
                            value: quote_search_parameters
                                .sell_amount
                                .try_into()
                                .map_err(|_| ValidationError::ZeroAmount)?,
                        },
                    },
                },
                verification: quote_search_parameters.verification.clone(),
                signing_scheme: quote_search_parameters.signing_scheme,
                additional_gas: quote_search_parameters.additional_gas,
            };

            let quote = quoter.calculate_quote(parameters).await?;
            let quote = quoter
                .store_quote(quote)
                .await
                .map_err(ValidationError::Other)?;

            tracing::debug!(quote_id =? quote.id, "computed fresh quote for order creation");
            quote
        }
        Err(err) => return Err(err.into()),
    };

    Ok(quote)
}

/// Amounts used for market price checker.
#[derive(Debug)]
pub struct Amounts {
    pub sell: U256,
    pub buy: U256,
    pub fee: U256,
}

/// Checks whether or not an order's limit price is outside the market price
/// specified by the quote.
///
/// Note that this check only looks at the order's limit price and the market
/// price and is independent of amounts or trade direction.
pub fn is_order_outside_market_price(order: &Amounts, quote: &Amounts) -> bool {
    (order.sell + order.fee).full_mul(quote.buy) < (quote.sell + quote.fee).full_mul(order.buy)
}

pub fn convert_signing_scheme_into_quote_signing_scheme(
    scheme: SigningScheme,
    order_placement_via_api: bool,
    verification_gas_limit: u64,
) -> Result<QuoteSigningScheme, ValidationError> {
    match (order_placement_via_api, scheme) {
        (true, SigningScheme::Eip712) => Ok(QuoteSigningScheme::Eip712),
        (true, SigningScheme::EthSign) => Ok(QuoteSigningScheme::EthSign),
        (false, SigningScheme::Eip712) => Err(ValidationError::IncompatibleSigningScheme),
        (false, SigningScheme::EthSign) => Err(ValidationError::IncompatibleSigningScheme),
        (order_placement_via_api, SigningScheme::PreSign) => Ok(QuoteSigningScheme::PreSign {
            onchain_order: !order_placement_via_api,
        }),
        (order_placement_via_api, SigningScheme::Eip1271) => Ok(QuoteSigningScheme::Eip1271 {
            onchain_order: !order_placement_via_api,
            verification_gas_limit,
        }),
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            account_balances::MockBalanceFetching,
            bad_token::{MockBadTokenDetecting, TokenQuality},
            code_fetching::MockCodeFetching,
            order_quoting::MockOrderQuoting,
            signature_validator::MockSignatureValidating,
        },
        anyhow::anyhow,
        chrono::Utc,
        contracts::dummy_contract,
        ethcontract::web3::signing::SecretKeyRef,
        futures::FutureExt,
        maplit::hashset,
        mockall::predicate::{always, eq},
        model::{
            quote::default_verification_gas_limit,
            signature::{EcdsaSignature, EcdsaSigningScheme},
        },
        number::nonzero::U256 as NonZeroU256,
        serde_json::json,
        std::str::FromStr,
    };

    #[test]
    fn minimum_balance_() {
        let order = OrderData {
            sell_amount: U256::MAX,
            fee_amount: U256::from(1),
            ..Default::default()
        };
        assert_eq!(minimum_balance(&order), None);
        let order = OrderData {
            sell_amount: U256::from(1),
            fee_amount: U256::from(1),
            ..Default::default()
        };
        assert_eq!(minimum_balance(&order), Some(U256::from(2)));
    }

    #[test]
    fn detects_orders_with_same_buy_and_sell_token() {
        let native_token = dummy_contract!(WETH9, [0xef; 20]);
        assert!(has_same_buy_and_sell_token(
            &PreOrderData {
                sell_token: H160([0x01; 20]),
                buy_token: H160([0x01; 20]),
                ..Default::default()
            },
            &native_token,
        ));
        assert!(has_same_buy_and_sell_token(
            &PreOrderData {
                sell_token: native_token.address(),
                buy_token: BUY_ETH_ADDRESS,
                ..Default::default()
            },
            &native_token,
        ));

        assert!(!has_same_buy_and_sell_token(
            &PreOrderData {
                sell_token: H160([0x01; 20]),
                buy_token: H160([0x02; 20]),
                ..Default::default()
            },
            &native_token,
        ));
        // Sell token set to 0xeee...eee has no special meaning, so it isn't
        // considered buying and selling the same token.
        assert!(!has_same_buy_and_sell_token(
            &PreOrderData {
                sell_token: BUY_ETH_ADDRESS,
                buy_token: native_token.address(),
                ..Default::default()
            },
            &native_token,
        ));
    }

    #[tokio::test]
    async fn pre_validate_err() {
        let native_token = dummy_contract!(WETH9, [0xef; 20]);
        let validity_configuration = OrderValidPeriodConfiguration {
            min: Duration::from_secs(1),
            max_market: Duration::from_secs(100),
            max_limit: Duration::from_secs(200),
        };
        let banned_users = hashset![H160::from_low_u64_be(1)];
        let legit_valid_to =
            time::now_in_epoch_seconds() + validity_configuration.min.as_secs() as u32 + 2;
        let mut limit_order_counter = MockLimitOrderCounting::new();
        limit_order_counter.expect_count().returning(|_| Ok(0u64));
        let validator = OrderValidator::new(
            native_token,
            banned_users,
            validity_configuration,
            false,
            Arc::new(MockBadTokenDetecting::new()),
            dummy_contract!(HooksTrampoline, [0xcf; 20]),
            Arc::new(MockOrderQuoting::new()),
            Arc::new(MockBalanceFetching::new()),
            Arc::new(MockSignatureValidating::new()),
            Arc::new(limit_order_counter),
            0,
            Arc::new(MockCodeFetching::new()),
            Default::default(),
            None,
        );
        let result = validator
            .partial_validate(PreOrderData {
                partially_fillable: true,
                ..Default::default()
            })
            .await;
        assert!(
            matches!(result, Err(PartialValidationError::UnsupportedOrderType)),
            "{result:?}"
        );
        assert!(matches!(
            validator
                .partial_validate(PreOrderData {
                    owner: H160::from_low_u64_be(1),
                    ..Default::default()
                })
                .await,
            Err(PartialValidationError::Forbidden)
        ));
        assert!(matches!(
            validator
                .partial_validate(PreOrderData {
                    receiver: H160::from_low_u64_be(1),
                    ..Default::default()
                })
                .await,
            Err(PartialValidationError::Forbidden)
        ));
        assert!(matches!(
            validator
                .partial_validate(PreOrderData {
                    buy_token_balance: BuyTokenDestination::Internal,
                    ..Default::default()
                })
                .await,
            Err(PartialValidationError::UnsupportedBuyTokenDestination(
                BuyTokenDestination::Internal
            ))
        ));
        assert!(matches!(
            validator
                .partial_validate(PreOrderData {
                    sell_token_balance: SellTokenSource::Internal,
                    ..Default::default()
                })
                .await,
            Err(PartialValidationError::UnsupportedSellTokenSource(
                SellTokenSource::Internal
            ))
        ));
        assert!(matches!(
            validator
                .partial_validate(PreOrderData {
                    valid_to: 0,
                    ..Default::default()
                })
                .await,
            Err(PartialValidationError::ValidTo(
                OrderValidToError::Insufficient,
            ))
        ));
        assert!(matches!(
            validator
                .partial_validate(PreOrderData {
                    valid_to: legit_valid_to
                        + validity_configuration.max_market.as_secs() as u32
                        + 1,
                    ..Default::default()
                })
                .await,
            Err(PartialValidationError::ValidTo(
                OrderValidToError::Excessive,
            ))
        ));
        assert!(matches!(
            validator
                .partial_validate(PreOrderData {
                    valid_to: legit_valid_to
                        + validity_configuration.max_limit.as_secs() as u32
                        + 1,
                    class: OrderClass::Limit,
                    ..Default::default()
                })
                .await,
            Err(PartialValidationError::ValidTo(
                OrderValidToError::Excessive,
            ))
        ));
        assert!(matches!(
            validator
                .partial_validate(PreOrderData {
                    valid_to: legit_valid_to,
                    buy_token: H160::from_low_u64_be(2),
                    sell_token: H160::from_low_u64_be(2),
                    ..Default::default()
                })
                .await,
            Err(PartialValidationError::SameBuyAndSellToken)
        ));
        assert!(matches!(
            validator
                .partial_validate(PreOrderData {
                    valid_to: legit_valid_to,
                    sell_token: BUY_ETH_ADDRESS,
                    ..Default::default()
                })
                .await,
            Err(PartialValidationError::InvalidNativeSellToken)
        ));
    }

    #[tokio::test]
    async fn pre_validate_ok() {
        let validity_configuration = OrderValidPeriodConfiguration {
            min: Duration::from_secs(1),
            max_market: Duration::from_secs(100),
            max_limit: Duration::from_secs(200),
        };

        let mut bad_token_detector = MockBadTokenDetecting::new();
        bad_token_detector
            .expect_detect()
            .with(eq(H160::from_low_u64_be(1)))
            .returning(|_| Ok(TokenQuality::Good));
        bad_token_detector
            .expect_detect()
            .with(eq(H160::from_low_u64_be(2)))
            .returning(|_| Ok(TokenQuality::Good));

        let mut limit_order_counter = MockLimitOrderCounting::new();
        limit_order_counter.expect_count().returning(|_| Ok(0u64));
        let validator = OrderValidator::new(
            dummy_contract!(WETH9, [0xef; 20]),
            hashset!(),
            validity_configuration,
            false,
            Arc::new(bad_token_detector),
            dummy_contract!(HooksTrampoline, [0xcf; 20]),
            Arc::new(MockOrderQuoting::new()),
            Arc::new(MockBalanceFetching::new()),
            Arc::new(MockSignatureValidating::new()),
            Arc::new(limit_order_counter),
            0,
            Arc::new(MockCodeFetching::new()),
            Default::default(),
            None,
        );
        let order = || PreOrderData {
            valid_to: time::now_in_epoch_seconds()
                + validity_configuration.min.as_secs() as u32
                + 2,
            sell_token: H160::from_low_u64_be(1),
            buy_token: H160::from_low_u64_be(2),
            ..Default::default()
        };

        assert!(validator.partial_validate(order()).await.is_ok());
        assert!(validator
            .partial_validate(PreOrderData {
                valid_to: u32::MAX,
                signing_scheme: SigningScheme::PreSign,
                ..order()
            })
            .await
            .is_ok());
        assert!(validator
            .partial_validate(PreOrderData {
                class: OrderClass::Limit,
                owner: H160::from_low_u64_be(0x42),
                valid_to: time::now_in_epoch_seconds()
                    + validity_configuration.max_market.as_secs() as u32
                    + 2,
                ..order()
            })
            .await
            .is_ok());
        assert!(validator
            .partial_validate(PreOrderData {
                partially_fillable: true,
                class: OrderClass::Liquidity,
                owner: H160::from_low_u64_be(0x42),
                valid_to: u32::MAX,
                ..order()
            })
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn post_validate_ok() {
        let mut order_quoter = MockOrderQuoting::new();
        let mut bad_token_detector = MockBadTokenDetecting::new();
        let mut balance_fetcher = MockBalanceFetching::new();
        order_quoter
            .expect_find_quote()
            .returning(|_, _| Ok(Default::default()));
        bad_token_detector
            .expect_detect()
            .returning(|_| Ok(TokenQuality::Good));
        balance_fetcher
            .expect_can_transfer()
            .returning(|_, _| Ok(()));

        let mut signature_validating = MockSignatureValidating::new();
        signature_validating
            .expect_validate_signature_and_get_additional_gas()
            .never();
        let signature_validating = Arc::new(signature_validating);

        let max_limit_orders_per_user = 1;

        let hooks = dummy_contract!(HooksTrampoline, [0xcf; 20]);

        let mut limit_order_counter = MockLimitOrderCounting::new();
        limit_order_counter.expect_count().returning(|_| Ok(0u64));
        let validator = OrderValidator::new(
            dummy_contract!(WETH9, [0xef; 20]),
            hashset!(),
            OrderValidPeriodConfiguration {
                min: Duration::from_secs(1),
                max_market: Duration::from_secs(100),
                max_limit: Duration::from_secs(200),
            },
            false,
            Arc::new(bad_token_detector),
            hooks.clone(),
            Arc::new(order_quoter),
            Arc::new(balance_fetcher),
            signature_validating,
            Arc::new(limit_order_counter),
            max_limit_orders_per_user,
            Arc::new(MockCodeFetching::new()),
            Default::default(),
            None,
        );

        let creation = OrderCreation {
            valid_to: time::now_in_epoch_seconds() + 2,
            sell_token: H160::from_low_u64_be(1),
            buy_token: H160::from_low_u64_be(2),
            buy_amount: U256::from(1),
            sell_amount: U256::from(1),
            fee_amount: U256::from(1),
            signature: Signature::Eip712(EcdsaSignature::non_zero()),
            app_data: OrderCreationAppData::Full {
                full: "{}".to_string(),
            },
            ..Default::default()
        };
        validator
            .validate_and_construct_order(
                creation.clone(),
                &Default::default(),
                Default::default(),
                None,
            )
            .await
            .unwrap();

        let domain_separator = DomainSeparator::default();
        let creation = OrderCreation {
            from: Some(H160([1; 20])),
            signature: Signature::Eip1271(vec![1, 2, 3]),
            app_data: OrderCreationAppData::Full {
                full: json!({
                    "metadata": {
                        "hooks": {
                            "pre": [
                                {
                                    "target": "0x1111111111111111111111111111111111111111",
                                    "callData": "0x112233",
                                    "gasLimit": "42",
                                }
                            ],
                            "post": [
                                {
                                    "target": "0x2222222222222222222222222222222222222222",
                                    "callData": "0x112233",
                                    "gasLimit": "42",
                                }
                            ],
                        },
                    },
                })
                .to_string(),
            },
            ..creation
        };
        let order_hash = hashed_eip712_message(&domain_separator, &creation.data().hash_struct());

        let pre_interactions = vec![InteractionData {
            target: hooks.address(),
            value: U256::zero(),
            call_data: hooks
                .execute(vec![(
                    addr!("1111111111111111111111111111111111111111"),
                    Bytes(vec![0x11, 0x22, 0x33]),
                    42.into(),
                )])
                .tx
                .data
                .unwrap()
                .0,
        }];

        let mut signature_validator = MockSignatureValidating::new();
        signature_validator
            .expect_validate_signature_and_get_additional_gas()
            .with(eq(SignatureCheck {
                signer: creation.from.unwrap(),
                hash: order_hash,
                signature: vec![1, 2, 3],
                interactions: pre_interactions.clone(),
            }))
            .returning(|_| Ok(0u64));

        let validator = OrderValidator {
            signature_validator: Arc::new(signature_validator),
            ..validator
        };

        assert!(validator
            .validate_and_construct_order(
                creation.clone(),
                &domain_separator,
                Default::default(),
                None
            )
            .await
            .is_ok());

        let mut signature_validator = MockSignatureValidating::new();
        signature_validator
            .expect_validate_signature_and_get_additional_gas()
            .with(eq(SignatureCheck {
                signer: creation.from.unwrap(),
                hash: order_hash,
                signature: vec![1, 2, 3],
                interactions: pre_interactions.clone(),
            }))
            .returning(|_| Err(SignatureValidationError::Invalid));

        let validator = OrderValidator {
            signature_validator: Arc::new(signature_validator),
            eip1271_skip_creation_validation: true,
            ..validator
        };

        assert!(validator
            .validate_and_construct_order(
                creation.clone(),
                &domain_separator,
                Default::default(),
                None
            )
            .await
            .is_ok());

        let creation_ = OrderCreation {
            fee_amount: U256::zero(),
            ..creation.clone()
        };
        let (order, quote) = validator
            .validate_and_construct_order(creation_, &domain_separator, Default::default(), None)
            .await
            .unwrap();
        assert!(quote.is_some());
        assert!(order.metadata.class.is_limit());

        let creation_ = OrderCreation {
            fee_amount: U256::zero(),
            partially_fillable: true,
            app_data: OrderCreationAppData::Full {
                full: "{}".to_string(),
            },
            ..creation
        };
        let (order, quote) = validator
            .validate_and_construct_order(creation_, &domain_separator, Default::default(), None)
            .await
            .unwrap();
        assert!(quote.is_some());
        assert!(order.metadata.class.is_limit());
    }

    #[tokio::test]
    async fn post_validate_too_many_limit_orders() {
        let mut order_quoter = MockOrderQuoting::new();
        let mut bad_token_detector = MockBadTokenDetecting::new();
        let mut balance_fetcher = MockBalanceFetching::new();
        order_quoter
            .expect_find_quote()
            .returning(|_, _| Ok(Default::default()));
        bad_token_detector
            .expect_detect()
            .returning(|_| Ok(TokenQuality::Good));
        balance_fetcher
            .expect_can_transfer()
            .returning(|_, _| Ok(()));

        let mut signature_validating = MockSignatureValidating::new();
        signature_validating
            .expect_validate_signature_and_get_additional_gas()
            .never();
        let signature_validating = Arc::new(signature_validating);

        const MAX_LIMIT_ORDERS_PER_USER: u64 = 2;

        let mut limit_order_counter = MockLimitOrderCounting::new();
        limit_order_counter
            .expect_count()
            .returning(|_| Ok(MAX_LIMIT_ORDERS_PER_USER));

        let validator = OrderValidator::new(
            dummy_contract!(WETH9, [0xef; 20]),
            hashset!(),
            OrderValidPeriodConfiguration::any(),
            false,
            Arc::new(bad_token_detector),
            dummy_contract!(HooksTrampoline, [0xcf; 20]),
            Arc::new(order_quoter),
            Arc::new(balance_fetcher),
            signature_validating,
            Arc::new(limit_order_counter),
            MAX_LIMIT_ORDERS_PER_USER,
            Arc::new(MockCodeFetching::new()),
            Default::default(),
            None,
        );

        let creation = OrderCreation {
            valid_to: model::time::now_in_epoch_seconds() + 2,
            sell_token: H160::from_low_u64_be(1),
            buy_token: H160::from_low_u64_be(2),
            buy_amount: U256::from(1),
            sell_amount: U256::from(1),
            signature: Signature::Eip712(EcdsaSignature::non_zero()),
            app_data: OrderCreationAppData::Full {
                full: "{}".to_string(),
            },
            ..Default::default()
        };
        let res = validator
            .validate_and_construct_order(
                creation.clone(),
                &Default::default(),
                Default::default(),
                None,
            )
            .await;
        assert!(
            matches!(res, Err(ValidationError::TooManyLimitOrders)),
            "{res:?}"
        );
    }

    #[tokio::test]
    async fn post_validate_err_zero_amount() {
        let mut order_quoter = MockOrderQuoting::new();
        let mut bad_token_detector = MockBadTokenDetecting::new();
        let mut balance_fetcher = MockBalanceFetching::new();
        order_quoter
            .expect_find_quote()
            .returning(|_, _| Ok(Default::default()));
        bad_token_detector
            .expect_detect()
            .returning(|_| Ok(TokenQuality::Good));
        balance_fetcher
            .expect_can_transfer()
            .returning(|_, _| Ok(()));
        let mut limit_order_counter = MockLimitOrderCounting::new();
        limit_order_counter.expect_count().returning(|_| Ok(0u64));
        let validator = OrderValidator::new(
            dummy_contract!(WETH9, [0xef; 20]),
            hashset!(),
            OrderValidPeriodConfiguration::any(),
            false,
            Arc::new(bad_token_detector),
            dummy_contract!(HooksTrampoline, [0xcf; 20]),
            Arc::new(order_quoter),
            Arc::new(balance_fetcher),
            Arc::new(MockSignatureValidating::new()),
            Arc::new(limit_order_counter),
            0,
            Arc::new(MockCodeFetching::new()),
            Default::default(),
            None,
        );
        let order = OrderCreation {
            valid_to: time::now_in_epoch_seconds() + 2,
            sell_token: H160::from_low_u64_be(1),
            buy_token: H160::from_low_u64_be(2),
            buy_amount: U256::from(0),
            sell_amount: U256::from(0),
            fee_amount: U256::from(1),
            signature: Signature::Eip712(EcdsaSignature::non_zero()),
            app_data: OrderCreationAppData::Full {
                full: "{}".to_string(),
            },
            ..Default::default()
        };
        let result = validator
            .validate_and_construct_order(order, &Default::default(), Default::default(), None)
            .await;
        assert!(matches!(result, Err(ValidationError::ZeroAmount)));
    }

    #[tokio::test]
    async fn post_out_of_market_orders_when_limit_orders_disabled() {
        let expected_buy_amount = U256::from(100);

        let mut order_quoter = MockOrderQuoting::new();
        let mut bad_token_detector = MockBadTokenDetecting::new();
        let mut balance_fetcher = MockBalanceFetching::new();
        order_quoter.expect_find_quote().returning(move |_, _| {
            Ok(Quote {
                buy_amount: expected_buy_amount,
                sell_amount: U256::from(1),
                fee_amount: U256::from(1),
                ..Default::default()
            })
        });
        bad_token_detector
            .expect_detect()
            .returning(|_| Ok(TokenQuality::Good));
        balance_fetcher
            .expect_can_transfer()
            .returning(|_, _| Ok(()));
        let mut limit_order_counter = MockLimitOrderCounting::new();
        limit_order_counter.expect_count().returning(|_| Ok(0u64));
        let validator = OrderValidator::new(
            dummy_contract!(WETH9, [0xef; 20]),
            hashset!(),
            OrderValidPeriodConfiguration::any(),
            false,
            Arc::new(bad_token_detector),
            dummy_contract!(HooksTrampoline, [0xcf; 20]),
            Arc::new(order_quoter),
            Arc::new(balance_fetcher),
            Arc::new(MockSignatureValidating::new()),
            Arc::new(limit_order_counter),
            0,
            Arc::new(MockCodeFetching::new()),
            Default::default(),
            None,
        );
        let order = OrderCreation {
            valid_to: time::now_in_epoch_seconds() + 2,
            sell_token: H160::from_low_u64_be(1),
            buy_token: H160::from_low_u64_be(2),
            buy_amount: expected_buy_amount + 1, // buy more than expected
            sell_amount: U256::from(1),
            fee_amount: U256::from(1),
            kind: OrderKind::Sell,
            signature: Signature::Eip712(EcdsaSignature::non_zero()),
            app_data: OrderCreationAppData::Full {
                full: "{}".to_string(),
            },
            ..Default::default()
        };
        let (order, quote) = validator
            .validate_and_construct_order(order, &Default::default(), Default::default(), None)
            .await
            .unwrap();

        // Out-of-price orders are intentionally marked as liquidity
        // orders!
        assert_eq!(order.metadata.class, OrderClass::Liquidity);
        assert!(quote.is_some());
    }

    #[tokio::test]
    async fn post_validate_err_wrong_owner() {
        let mut order_quoter = MockOrderQuoting::new();
        let mut bad_token_detector = MockBadTokenDetecting::new();
        let mut balance_fetcher = MockBalanceFetching::new();
        order_quoter
            .expect_find_quote()
            .returning(|_, _| Ok(Default::default()));
        bad_token_detector
            .expect_detect()
            .returning(|_| Ok(TokenQuality::Good));
        balance_fetcher
            .expect_can_transfer()
            .returning(|_, _| Ok(()));
        let mut limit_order_counter = MockLimitOrderCounting::new();
        limit_order_counter.expect_count().returning(|_| Ok(0u64));
        let validator = OrderValidator::new(
            dummy_contract!(WETH9, [0xef; 20]),
            hashset!(),
            OrderValidPeriodConfiguration::any(),
            false,
            Arc::new(bad_token_detector),
            dummy_contract!(HooksTrampoline, [0xcf; 20]),
            Arc::new(order_quoter),
            Arc::new(balance_fetcher),
            Arc::new(MockSignatureValidating::new()),
            Arc::new(limit_order_counter),
            0,
            Arc::new(MockCodeFetching::new()),
            Default::default(),
            None,
        );
        let order = OrderCreation {
            valid_to: time::now_in_epoch_seconds() + 2,
            sell_token: H160::from_low_u64_be(1),
            buy_token: H160::from_low_u64_be(2),
            buy_amount: U256::from(1),
            sell_amount: U256::from(1),
            fee_amount: U256::from(1),
            from: Some(Default::default()),
            signature: Signature::Eip712(EcdsaSignature::non_zero()),
            app_data: OrderCreationAppData::Full {
                full: "{}".to_string(),
            },
            ..Default::default()
        };
        let result = validator
            .validate_and_construct_order(order, &Default::default(), Default::default(), None)
            .await;
        assert!(matches!(result, Err(ValidationError::WrongOwner(_))));
    }

    #[tokio::test]
    async fn post_validate_err_getting_quote() {
        let mut order_quoter = MockOrderQuoting::new();
        let mut bad_token_detector = MockBadTokenDetecting::new();
        let mut balance_fetcher = MockBalanceFetching::new();
        order_quoter
            .expect_find_quote()
            .returning(|_, _| Err(FindQuoteError::Other(anyhow!("err"))));
        bad_token_detector
            .expect_detect()
            .returning(|_| Ok(TokenQuality::Good));
        balance_fetcher
            .expect_can_transfer()
            .returning(|_, _| Ok(()));
        let mut limit_order_counter = MockLimitOrderCounting::new();
        limit_order_counter.expect_count().returning(|_| Ok(0u64));
        let validator = OrderValidator::new(
            dummy_contract!(WETH9, [0xef; 20]),
            hashset!(),
            OrderValidPeriodConfiguration::any(),
            false,
            Arc::new(bad_token_detector),
            dummy_contract!(HooksTrampoline, [0xcf; 20]),
            Arc::new(order_quoter),
            Arc::new(balance_fetcher),
            Arc::new(MockSignatureValidating::new()),
            Arc::new(limit_order_counter),
            0,
            Arc::new(MockCodeFetching::new()),
            Default::default(),
            None,
        );
        let order = OrderCreation {
            valid_to: time::now_in_epoch_seconds() + 2,
            sell_token: H160::from_low_u64_be(1),
            buy_token: H160::from_low_u64_be(2),
            buy_amount: U256::from(1),
            sell_amount: U256::from(1),
            fee_amount: U256::from(1),
            signature: Signature::Eip712(EcdsaSignature::non_zero()),
            app_data: OrderCreationAppData::Full {
                full: "{}".to_string(),
            },
            ..Default::default()
        };
        let result = validator
            .validate_and_construct_order(order, &Default::default(), Default::default(), None)
            .await;
        dbg!(&result);
        assert!(matches!(result, Err(ValidationError::Other(_))));
    }

    #[tokio::test]
    async fn post_validate_err_unsupported_token() {
        let mut order_quoter = MockOrderQuoting::new();
        let mut bad_token_detector = MockBadTokenDetecting::new();
        let mut balance_fetcher = MockBalanceFetching::new();
        order_quoter
            .expect_find_quote()
            .returning(|_, _| Ok(Default::default()));
        bad_token_detector.expect_detect().returning(|_| {
            Ok(TokenQuality::Bad {
                reason: Default::default(),
            })
        });
        balance_fetcher
            .expect_can_transfer()
            .returning(|_, _| Ok(()));
        let mut limit_order_counter = MockLimitOrderCounting::new();
        limit_order_counter.expect_count().returning(|_| Ok(0u64));
        let validator = OrderValidator::new(
            dummy_contract!(WETH9, [0xef; 20]),
            hashset!(),
            OrderValidPeriodConfiguration::any(),
            false,
            Arc::new(bad_token_detector),
            dummy_contract!(HooksTrampoline, [0xcf; 20]),
            Arc::new(order_quoter),
            Arc::new(balance_fetcher),
            Arc::new(MockSignatureValidating::new()),
            Arc::new(limit_order_counter),
            0,
            Arc::new(MockCodeFetching::new()),
            Default::default(),
            None,
        );
        let order = OrderCreation {
            valid_to: time::now_in_epoch_seconds() + 2,
            sell_token: H160::from_low_u64_be(1),
            buy_token: H160::from_low_u64_be(2),
            buy_amount: U256::from(1),
            sell_amount: U256::from(1),
            fee_amount: U256::from(1),
            signature: Signature::Eip712(EcdsaSignature::non_zero()),
            app_data: OrderCreationAppData::Full {
                full: "{}".to_string(),
            },
            ..Default::default()
        };
        let result = validator
            .validate_and_construct_order(order, &Default::default(), Default::default(), None)
            .await;
        dbg!(&result);
        assert!(matches!(
            result,
            Err(ValidationError::Partial(
                PartialValidationError::UnsupportedToken { .. }
            ))
        ));
    }

    #[tokio::test]
    async fn post_validate_err_sell_amount_overflow() {
        let mut order_quoter = MockOrderQuoting::new();
        let mut bad_token_detector = MockBadTokenDetecting::new();
        let mut balance_fetcher = MockBalanceFetching::new();
        order_quoter
            .expect_find_quote()
            .returning(|_, _| Ok(Default::default()));
        order_quoter.expect_store_quote().returning(Ok);
        bad_token_detector
            .expect_detect()
            .returning(|_| Ok(TokenQuality::Good));
        balance_fetcher
            .expect_can_transfer()
            .returning(|_, _| Ok(()));
        let mut limit_order_counter = MockLimitOrderCounting::new();
        limit_order_counter.expect_count().returning(|_| Ok(0u64));
        let validator = OrderValidator::new(
            dummy_contract!(WETH9, [0xef; 20]),
            hashset!(),
            OrderValidPeriodConfiguration::any(),
            false,
            Arc::new(bad_token_detector),
            dummy_contract!(HooksTrampoline, [0xcf; 20]),
            Arc::new(order_quoter),
            Arc::new(balance_fetcher),
            Arc::new(MockSignatureValidating::new()),
            Arc::new(limit_order_counter),
            0,
            Arc::new(MockCodeFetching::new()),
            Default::default(),
            None,
        );
        let order = OrderCreation {
            valid_to: time::now_in_epoch_seconds() + 2,
            sell_token: H160::from_low_u64_be(1),
            buy_token: H160::from_low_u64_be(2),
            buy_amount: U256::from(1),
            sell_amount: U256::MAX,
            fee_amount: U256::from(1),
            signature: Signature::Eip712(EcdsaSignature::non_zero()),
            app_data: OrderCreationAppData::Full {
                full: "{}".to_string(),
            },
            ..Default::default()
        };
        let result = validator
            .validate_and_construct_order(order, &Default::default(), Default::default(), None)
            .await;
        dbg!(&result);
        assert!(matches!(result, Err(ValidationError::SellAmountOverflow)));
    }

    #[tokio::test]
    async fn post_validate_err_insufficient_balance() {
        let mut order_quoter = MockOrderQuoting::new();
        let mut bad_token_detector = MockBadTokenDetecting::new();
        let mut balance_fetcher = MockBalanceFetching::new();
        order_quoter
            .expect_find_quote()
            .returning(|_, _| Ok(Default::default()));
        bad_token_detector
            .expect_detect()
            .returning(|_| Ok(TokenQuality::Good));
        balance_fetcher
            .expect_can_transfer()
            .returning(|_, _| Err(TransferSimulationError::InsufficientBalance));
        let mut limit_order_counter = MockLimitOrderCounting::new();
        limit_order_counter.expect_count().returning(|_| Ok(0u64));
        let validator = OrderValidator::new(
            dummy_contract!(WETH9, [0xef; 20]),
            hashset!(),
            OrderValidPeriodConfiguration::any(),
            false,
            Arc::new(bad_token_detector),
            dummy_contract!(HooksTrampoline, [0xcf; 20]),
            Arc::new(order_quoter),
            Arc::new(balance_fetcher),
            Arc::new(MockSignatureValidating::new()),
            Arc::new(limit_order_counter),
            0,
            Arc::new(MockCodeFetching::new()),
            Default::default(),
            None,
        );
        let order = OrderCreation {
            valid_to: time::now_in_epoch_seconds() + 2,
            sell_token: H160::from_low_u64_be(1),
            buy_token: H160::from_low_u64_be(2),
            buy_amount: U256::from(1),
            sell_amount: U256::from(1),
            fee_amount: U256::from(1),
            signature: Signature::Eip712(EcdsaSignature::non_zero()),
            app_data: OrderCreationAppData::Full {
                full: "{}".to_string(),
            },
            ..Default::default()
        };
        let result = validator
            .validate_and_construct_order(order, &Default::default(), Default::default(), None)
            .await;
        dbg!(&result);
        assert!(matches!(result, Err(ValidationError::InsufficientBalance)));
    }

    #[tokio::test]
    async fn post_validate_err_invalid_eip1271_signature() {
        let mut order_quoter = MockOrderQuoting::new();
        let mut bad_token_detector = MockBadTokenDetecting::new();
        let mut balance_fetcher = MockBalanceFetching::new();
        let mut signature_validator = MockSignatureValidating::new();
        order_quoter
            .expect_find_quote()
            .returning(|_, _| Ok(Default::default()));
        bad_token_detector
            .expect_detect()
            .returning(|_| Ok(TokenQuality::Good));
        balance_fetcher
            .expect_can_transfer()
            .returning(|_, _| Ok(()));
        signature_validator
            .expect_validate_signature_and_get_additional_gas()
            .returning(|_| Err(SignatureValidationError::Invalid));
        let mut limit_order_counter = MockLimitOrderCounting::new();
        limit_order_counter.expect_count().returning(|_| Ok(0u64));
        let validator = OrderValidator::new(
            dummy_contract!(WETH9, [0xef; 20]),
            hashset!(),
            OrderValidPeriodConfiguration::any(),
            false,
            Arc::new(bad_token_detector),
            dummy_contract!(HooksTrampoline, [0xcf; 20]),
            Arc::new(order_quoter),
            Arc::new(balance_fetcher),
            Arc::new(signature_validator),
            Arc::new(limit_order_counter),
            0,
            Arc::new(MockCodeFetching::new()),
            Default::default(),
            None,
        );

        let creation = OrderCreation {
            valid_to: time::now_in_epoch_seconds() + 2,
            sell_token: H160::from_low_u64_be(1),
            buy_token: H160::from_low_u64_be(2),
            buy_amount: U256::from(1),
            sell_amount: U256::from(1),
            fee_amount: U256::from(1),
            from: Some(H160([1; 20])),
            signature: Signature::Eip1271(vec![1, 2, 3]),
            app_data: OrderCreationAppData::Full {
                full: "{}".to_string(),
            },
            ..Default::default()
        };
        let domain = DomainSeparator::default();

        assert!(matches!(
            validator
                .validate_and_construct_order(creation.clone(), &domain, Default::default(), None)
                .await
                .unwrap_err(),
            ValidationError::InvalidEip1271Signature(hash)
                if hash.0 == signature::hashed_eip712_message(&domain, &creation.data().hash_struct()),
        ));
    }

    #[test]
    fn allows_insufficient_allowance_and_balance_for_presign_orders() {
        fn assert_allows_failed_transfer(
            create_error: impl Fn() -> TransferSimulationError + Send + 'static,
            is_expected_error: impl Fn(ValidationError) -> bool,
        ) {
            let mut order_quoter = MockOrderQuoting::new();
            let mut bad_token_detector = MockBadTokenDetecting::new();
            let mut balance_fetcher = MockBalanceFetching::new();
            order_quoter
                .expect_find_quote()
                .returning(|_, _| Ok(Default::default()));
            bad_token_detector
                .expect_detect()
                .returning(|_| Ok(TokenQuality::Good));
            balance_fetcher
                .expect_can_transfer()
                .returning(move |_, _| Err(create_error()));
            let mut limit_order_counter = MockLimitOrderCounting::new();
            limit_order_counter.expect_count().returning(|_| Ok(0u64));
            let validator = OrderValidator::new(
                dummy_contract!(WETH9, [0xef; 20]),
                hashset!(),
                OrderValidPeriodConfiguration::any(),
                false,
                Arc::new(bad_token_detector),
                dummy_contract!(HooksTrampoline, [0xcf; 20]),
                Arc::new(order_quoter),
                Arc::new(balance_fetcher),
                Arc::new(MockSignatureValidating::new()),
                Arc::new(limit_order_counter),
                0,
                Arc::new(MockCodeFetching::new()),
                Default::default(),
                None,
            );

            let order = OrderCreation {
                valid_to: u32::MAX,
                sell_token: H160::from_low_u64_be(1),
                sell_amount: 1.into(),
                buy_token: H160::from_low_u64_be(2),
                buy_amount: 1.into(),
                fee_amount: 1.into(),
                app_data: OrderCreationAppData::Full {
                    full: "{}".to_string(),
                },
                ..Default::default()
            };

            for signing_scheme in [EcdsaSigningScheme::Eip712, EcdsaSigningScheme::EthSign] {
                let err = validator
                    .validate_and_construct_order(
                        order.clone().sign(
                            signing_scheme,
                            &Default::default(),
                            SecretKeyRef::new(&secp256k1::SecretKey::from_str("0000000000000000000000000000000000000000000000000000000000000001").unwrap()),
                        ),
                        &Default::default(),
                        Default::default(),
                        None,
                    )
                    .now_or_never()
                    .unwrap()
                    .unwrap_err();
                assert!(is_expected_error(err));
            }

            let order = OrderCreation {
                signature: Signature::PreSign,
                from: Some(Default::default()),
                ..order
            };
            validator
                .validate_and_construct_order(order, &Default::default(), Default::default(), None)
                .now_or_never()
                .unwrap()
                .unwrap();
        }

        assert_allows_failed_transfer(
            || TransferSimulationError::InsufficientAllowance,
            |e| matches!(e, ValidationError::InsufficientAllowance),
        );
        assert_allows_failed_transfer(
            || TransferSimulationError::InsufficientBalance,
            |e| matches!(e, ValidationError::InsufficientBalance),
        );
    }

    #[tokio::test]
    async fn get_quote_find_by_id() {
        let mut order_quoter = MockOrderQuoting::new();
        let quote_search_parameters = QuoteSearchParameters {
            sell_token: H160([1; 20]),
            buy_token: H160([2; 20]),
            sell_amount: 3.into(),
            buy_amount: 4.into(),
            fee_amount: 6.into(),
            kind: OrderKind::Buy,
            signing_scheme: QuoteSigningScheme::Eip1271 {
                onchain_order: true,
                verification_gas_limit: default_verification_gas_limit(),
            },
            additional_gas: 0,
            verification: Some(Verification {
                from: H160([0xf0; 20]),
                ..Default::default()
            }),
        };
        let quote_data = Quote {
            fee_amount: 6.into(),
            ..Default::default()
        };
        let fee_amount = quote_data.fee_amount;
        let quote_id = Some(42);
        order_quoter
            .expect_find_quote()
            .with(eq(quote_id), eq(quote_search_parameters.clone()))
            .returning(move |_, _| Ok(quote_data.clone()));

        let quote = get_quote_and_check_fee(
            &order_quoter,
            &quote_search_parameters,
            quote_id,
            Some(fee_amount),
        )
        .await
        .unwrap();

        assert_eq!(
            quote,
            Quote {
                fee_amount: 6.into(),
                ..Default::default()
            }
        );
    }

    #[tokio::test]
    async fn get_quote_calculates_fresh_quote_when_not_found() {
        let verification = Some(Verification {
            from: H160([0xf0; 20]),
            ..Default::default()
        });

        let mut order_quoter = MockOrderQuoting::new();
        order_quoter
            .expect_find_quote()
            .with(eq(None), always())
            .returning(|_, _| Err(FindQuoteError::NotFound(None)));
        let quote_search_parameters = QuoteSearchParameters {
            sell_token: H160([1; 20]),
            buy_token: H160([2; 20]),
            sell_amount: 3.into(),
            kind: OrderKind::Sell,
            verification: verification.clone(),
            ..Default::default()
        };
        let quote_data = Quote {
            fee_amount: 6.into(),
            ..Default::default()
        };
        let fee_amount = quote_data.fee_amount;
        order_quoter
            .expect_calculate_quote()
            .with(eq(QuoteParameters {
                sell_token: quote_search_parameters.sell_token,
                buy_token: quote_search_parameters.buy_token,
                side: OrderQuoteSide::Sell {
                    sell_amount: SellAmount::AfterFee {
                        value: NonZeroU256::try_from(quote_search_parameters.sell_amount).unwrap(),
                    },
                },
                verification,
                signing_scheme: QuoteSigningScheme::Eip712,
                additional_gas: 0,
            }))
            .returning({
                let quote_data = quote_data.clone();
                move |_| Ok(quote_data.clone())
            });
        order_quoter
            .expect_store_quote()
            .with(eq(quote_data.clone()))
            .returning(|quote| {
                Ok(Quote {
                    id: Some(42),
                    ..quote
                })
            });

        let quote = get_quote_and_check_fee(
            &order_quoter,
            &quote_search_parameters,
            None,
            Some(fee_amount),
        )
        .await
        .unwrap();

        assert_eq!(
            quote,
            Quote {
                id: Some(42),
                fee_amount: 6.into(),
                ..Default::default()
            }
        );
    }

    #[tokio::test]
    async fn get_quote_errors_when_not_found_by_id() {
        let quote_search_parameters = QuoteSearchParameters {
            ..Default::default()
        };

        let mut order_quoter = MockOrderQuoting::new();
        order_quoter
            .expect_find_quote()
            .returning(|_, _| Err(FindQuoteError::NotFound(Some(0))));

        let err = get_quote_and_check_fee(
            &order_quoter,
            &quote_search_parameters,
            Some(0),
            Some(U256::zero()),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, ValidationError::QuoteNotFound));
    }

    #[tokio::test]
    async fn get_quote_errors_on_insufficient_fees() {
        let mut order_quoter = MockOrderQuoting::new();
        order_quoter.expect_find_quote().returning(|_, _| {
            Ok(Quote {
                fee_amount: 2.into(),
                ..Default::default()
            })
        });

        let err = get_quote_and_check_fee(
            &order_quoter,
            &Default::default(),
            Default::default(),
            Some(U256::one()),
        )
        .await
        .unwrap_err();

        assert!(matches!(err, ValidationError::InsufficientFee));
    }

    #[tokio::test]
    async fn get_quote_bubbles_errors() {
        macro_rules! assert_find_error_matches {
            ($find_err:expr, $validation_err:pat) => {{
                let mut order_quoter = MockOrderQuoting::new();
                order_quoter
                    .expect_find_quote()
                    .returning(|_, _| Err($find_err));
                let err = get_quote_and_check_fee(
                    &order_quoter,
                    &QuoteSearchParameters {
                        sell_amount: 1.into(),
                        kind: OrderKind::Sell,
                        ..Default::default()
                    },
                    Default::default(),
                    Default::default(),
                )
                .await
                .unwrap_err();

                assert!(matches!(err, $validation_err));
            }};
        }

        assert_find_error_matches!(
            FindQuoteError::Expired(Utc::now()),
            ValidationError::InvalidQuote
        );
        assert_find_error_matches!(
            FindQuoteError::ParameterMismatch(Default::default()),
            ValidationError::InvalidQuote
        );

        macro_rules! assert_calc_error_matches {
            ($calc_err:expr, $validation_err:pat) => {{
                let mut order_quoter = MockOrderQuoting::new();
                order_quoter
                    .expect_find_quote()
                    .returning(|_, _| Err(FindQuoteError::NotFound(None)));
                order_quoter
                    .expect_calculate_quote()
                    .returning(|_| Err($calc_err));

                let err = get_quote_and_check_fee(
                    &order_quoter,
                    &QuoteSearchParameters {
                        sell_amount: 1.into(),
                        kind: OrderKind::Sell,
                        ..Default::default()
                    },
                    Default::default(),
                    Some(U256::zero()),
                )
                .await
                .unwrap_err();

                assert!(matches!(err, $validation_err));
            }};
        }

        assert_calc_error_matches!(
            CalculateQuoteError::SellAmountDoesNotCoverFee {
                fee_amount: Default::default()
            },
            ValidationError::Other(_)
        );
        assert_calc_error_matches!(
            CalculateQuoteError::Price(PriceEstimationError::UnsupportedToken {
                token: Default::default(),
                reason: Default::default()
            }),
            ValidationError::Partial(PartialValidationError::UnsupportedToken { .. })
        );
        assert_calc_error_matches!(
            CalculateQuoteError::Price(PriceEstimationError::NoLiquidity),
            ValidationError::PriceForQuote(_)
        );
        assert_calc_error_matches!(
            CalculateQuoteError::Price(PriceEstimationError::UnsupportedOrderType("test".into())),
            ValidationError::PriceForQuote(_)
        );
        assert_calc_error_matches!(
            CalculateQuoteError::Price(PriceEstimationError::RateLimited),
            ValidationError::PriceForQuote(_)
        );
    }

    #[test]
    fn detects_market_orders() {
        let quote = Quote {
            sell_amount: 90.into(),
            buy_amount: 100.into(),
            fee_amount: 10.into(),
            ..Default::default()
        };

        // at market price
        assert!(!is_order_outside_market_price(
            &Amounts {
                sell: 100.into(),
                buy: 100.into(),
                fee: 0.into(),
            },
            &Amounts {
                sell: quote.sell_amount,
                buy: quote.buy_amount,
                fee: quote.fee_amount,
            },
        ));
        // willing to buy less than market price
        assert!(!is_order_outside_market_price(
            &Amounts {
                sell: 100.into(),
                buy: 90.into(),
                fee: 0.into(),
            },
            &Amounts {
                sell: quote.sell_amount,
                buy: quote.buy_amount,
                fee: quote.fee_amount,
            },
        ));
        // wanting to buy more than market price
        assert!(is_order_outside_market_price(
            &Amounts {
                sell: 100.into(),
                buy: 1000.into(),
                fee: 0.into(),
            },
            &Amounts {
                sell: quote.sell_amount,
                buy: quote.buy_amount,
                fee: quote.fee_amount,
            },
        ));
    }
}
