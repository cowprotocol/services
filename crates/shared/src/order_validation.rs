use crate::{
    account_balances::{BalanceFetching, TransferSimulationError},
    bad_token::BadTokenDetecting,
    order_quoting::{
        CalculateQuoteError, FindQuoteError, OrderQuoting, Quote, QuoteParameters,
        QuoteSearchParameters,
    },
    price_estimation::PriceEstimationError,
    signature_validator::{SignatureCheck, SignatureValidating, SignatureValidationError},
    web3_traits::CodeFetching,
};
use anyhow::anyhow;
use contracts::WETH9;
use database::quotes::QuoteKind;
use ethcontract::{H160, U256};
use model::{
    order::{
        BuyTokenDestination, Order, OrderCreation, OrderData, OrderKind, SellTokenSource,
        BUY_ETH_ADDRESS,
    },
    quote::{OrderQuoteSide, QuoteSigningScheme, SellAmount},
    signature::{hashed_eip712_message, Signature, SigningScheme, VerificationError},
    DomainSeparator,
};
use std::{collections::HashSet, sync::Arc, time::Duration};

#[mockall::automock]
#[async_trait::async_trait]
pub trait OrderValidating: Send + Sync {
    /// Partial (aka Pre-) Validation is aimed at catching malformed order data during the
    /// fee & quote phase (i.e. before the order is signed).
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

    /// This is the full order validation performed at the time of order placement
    /// (i.e. once all the required fields on an Order are provided). Specifically, verifying that
    ///     - buy & sell amounts are non-zero,
    ///     - order's signature recovers correctly
    ///     - fee is sufficient,
    ///     - user has sufficient (transferable) funds to execute the order.
    ///
    /// Furthermore, full order validation also calls partial_validate to ensure that
    /// other aspects of the order are not malformed.
    async fn validate_and_construct_order(
        &self,
        order: OrderCreation,
        domain_separator: &DomainSeparator,
        settlement_contract: H160,
    ) -> Result<(Order, Option<Quote>), ValidationError>;
}

#[derive(Debug)]
pub enum PartialValidationError {
    Forbidden,
    InsufficientValidTo,
    ExcessiveValidTo,
    TransferEthToContract,
    InvalidNativeSellToken,
    SameBuyAndSellToken,
    UnsupportedBuyTokenDestination(BuyTokenDestination),
    UnsupportedSellTokenSource(SellTokenSource),
    UnsupportedOrderType,
    UnsupportedSignature,
    UnsupportedToken(H160),
    Other(anyhow::Error),
}

#[derive(Debug)]
pub enum ValidationError {
    Partial(PartialValidationError),
    /// The quote ID specifed with the order could not be found.
    QuoteNotFound,
    /// The quote specified by ID is invalid. Either it doesn't match the order
    /// or it has already expired.
    InvalidQuote,
    /// Unable to compute quote because of a price estimation error.
    PriceForQuote(PriceEstimationError),
    InsufficientFee,
    InsufficientBalance,
    InsufficientAllowance,
    InvalidSignature,
    /// If fee and sell amount overflow u256
    SellAmountOverflow,
    TransferSimulationFailed,
    /// The specified on-chain signature requires the from address of the
    /// order signer.
    MissingFrom,
    WrongOwner(H160),
    ZeroAmount,
    IncompatibleSigningScheme,
    Other(anyhow::Error),
}

impl From<VerificationError> for ValidationError {
    fn from(err: VerificationError) -> Self {
        match err {
            VerificationError::UnableToRecoverSigner(_) => Self::InvalidSignature,
            VerificationError::UnexpectedSigner(signer) => Self::WrongOwner(signer),
            VerificationError::MissingFrom => Self::MissingFrom,
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
            CalculateQuoteError::Price(PriceEstimationError::UnsupportedToken(token)) => {
                ValidationError::Partial(PartialValidationError::UnsupportedToken(token))
            }
            CalculateQuoteError::Price(PriceEstimationError::ZeroAmount) => {
                ValidationError::ZeroAmount
            }
            CalculateQuoteError::Other(err)
            | CalculateQuoteError::Price(PriceEstimationError::Other(err)) => {
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

impl From<SignatureValidationError> for ValidationError {
    fn from(err: SignatureValidationError) -> Self {
        match err {
            SignatureValidationError::Invalid => Self::InvalidSignature,
            SignatureValidationError::Other(err) => Self::Other(err.into()),
        }
    }
}

pub struct OrderValidator {
    /// For Pre/Partial-Validation: performed during fee & quote phase
    /// when only part of the order data is available
    code_fetcher: Box<dyn CodeFetching>,
    native_token: WETH9,
    banned_users: HashSet<H160>,
    liquidity_order_owners: HashSet<H160>,
    min_order_validity_period: Duration,
    max_order_validity_period: Duration,
    signature_configuration: SignatureConfiguration,
    bad_token_detector: Arc<dyn BadTokenDetecting>,
    /// For Full-Validation: performed time of order placement
    quoter: Arc<dyn OrderQuoting>,
    balance_fetcher: Arc<dyn BalanceFetching>,
    signature_validator: Arc<dyn SignatureValidating>,
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
    pub is_liquidity_order: bool,
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
        is_liquidity_order: bool,
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
            is_liquidity_order,
        }
    }
}

impl OrderValidator {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        code_fetcher: Box<dyn CodeFetching>,
        native_token: WETH9,
        banned_users: HashSet<H160>,
        liquidity_order_owners: HashSet<H160>,
        min_order_validity_period: Duration,
        max_order_validity_period: Duration,
        signature_configuration: SignatureConfiguration,
        bad_token_detector: Arc<dyn BadTokenDetecting>,
        quoter: Arc<dyn OrderQuoting>,
        balance_fetcher: Arc<dyn BalanceFetching>,
        signature_validator: Arc<dyn SignatureValidating>,
    ) -> Self {
        Self {
            code_fetcher,
            native_token,
            banned_users,
            liquidity_order_owners,
            min_order_validity_period,
            max_order_validity_period,
            signature_configuration,
            bad_token_detector,
            quoter,
            balance_fetcher,
            signature_validator,
        }
    }
}

#[async_trait::async_trait]
impl OrderValidating for OrderValidator {
    async fn partial_validate(&self, order: PreOrderData) -> Result<(), PartialValidationError> {
        if self.banned_users.contains(&order.owner) {
            return Err(PartialValidationError::Forbidden);
        }

        if order.partially_fillable && !order.is_liquidity_order {
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

        // Eventually we will support all Signature types and can remove this.
        if !self
            .signature_configuration
            .is_signing_scheme_supported(order.signing_scheme)
        {
            return Err(PartialValidationError::UnsupportedSignature);
        }

        let now = model::time::now_in_epoch_seconds();
        if order.valid_to < now + self.min_order_validity_period.as_secs() as u32 {
            return Err(PartialValidationError::InsufficientValidTo);
        }
        if order.valid_to > now.saturating_add(self.max_order_validity_period.as_secs() as u32)
            && !order.is_liquidity_order
            && order.signing_scheme != SigningScheme::PreSign
        {
            return Err(PartialValidationError::ExcessiveValidTo);
        }

        if has_same_buy_and_sell_token(&order, &self.native_token) {
            return Err(PartialValidationError::SameBuyAndSellToken);
        }
        if order.sell_token == BUY_ETH_ADDRESS {
            return Err(PartialValidationError::InvalidNativeSellToken);
        }
        if order.buy_token == BUY_ETH_ADDRESS {
            let code_size = self
                .code_fetcher
                .code_size(order.receiver)
                .await
                .map_err(PartialValidationError::Other)?;
            if code_size != 0 {
                return Err(PartialValidationError::TransferEthToContract);
            }
        }

        for &token in &[order.sell_token, order.buy_token] {
            if !self
                .bad_token_detector
                .detect(token)
                .await
                .map_err(PartialValidationError::Other)?
                .is_good()
            {
                return Err(PartialValidationError::UnsupportedToken(token));
            }
        }

        Ok(())
    }

    async fn validate_and_construct_order(
        &self,
        order: OrderCreation,
        domain_separator: &DomainSeparator,
        settlement_contract: H160,
    ) -> Result<(Order, Option<Quote>), ValidationError> {
        let owner = order.verify_owner(domain_separator)?;
        let signing_scheme = order.signature.scheme();

        if let Signature::Eip1271(signature) = &order.signature {
            self.signature_validator
                .validate_signature(SignatureCheck {
                    signer: owner,
                    hash: hashed_eip712_message(domain_separator, &order.data.hash_struct()),
                    signature: signature.to_owned(),
                })
                .await?;
        }

        if order.data.buy_amount.is_zero() || order.data.sell_amount.is_zero() {
            return Err(ValidationError::ZeroAmount);
        }

        let liquidity_owner = self.liquidity_order_owners.contains(&owner);
        self.partial_validate(PreOrderData::from_order_creation(
            owner,
            &order.data,
            signing_scheme,
            liquidity_owner,
        ))
        .await
        .map_err(ValidationError::Partial)?;

        let quote = if !liquidity_owner {
            Some(get_quote_and_check_fee(&*self.quoter, &order, true, owner).await?)
        } else {
            // We don't try to get quotes for orders created by liqudity order
            // owners for two reasons:
            // 1. They don't pay fees, meaning we don't need to know what the
            //    min fee amount is.
            // 2. We don't really care about the equivalent quote since they
            //    aren't expected to follow regular order creation flow.
            None
        };

        let full_fee_amount = quote
            .as_ref()
            .map(|quote| quote.data.fee_parameters.unsubsidized())
            .unwrap_or_default();

        let min_balance = match minimum_balance(&order.data) {
            Some(amount) => amount,
            None => return Err(ValidationError::SellAmountOverflow),
        };

        // Fast path to check if transfer is possible with a single node query.
        // If not, run extra queries for additional information.
        match self
            .balance_fetcher
            .can_transfer(
                order.data.sell_token,
                owner,
                min_balance,
                order.data.sell_token_balance,
            )
            .await
        {
            Ok(_) => (),
            Err(
                TransferSimulationError::InsufficientAllowance
                | TransferSimulationError::InsufficientBalance,
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

        // Orders that are placed and priced outside the market (i.e. buying
        // more than the market can pay or selling less than the market wants)
        // get flagged as liquidity orders. The reasoning is that these orders
        // are not intended to be filled immediately and so need to be treated
        // slightly differently by the protocol.
        let is_liquidity_order = match &quote {
            Some(quote) if is_order_outside_market_price(&order.data, quote) => {
                let order_uid = order.data.uid(domain_separator, &owner);
                tracing::debug!(%order_uid, ?owner, "order being flagged as outside market price");
                true
            }
            Some(_) => false,
            None => true,
        };

        let order = Order::from_order_creation(
            &order,
            domain_separator,
            settlement_contract,
            full_fee_amount,
            is_liquidity_order,
        )?;
        Ok((order, quote))
    }
}

/// Signature configuration that is accepted by the orderbook.
#[derive(Debug, Eq, PartialEq, Default)]
pub struct SignatureConfiguration {
    pub eip1271: bool,
    pub presign: bool,
}

impl SignatureConfiguration {
    /// Returns a configuration where only off-chain signing schemes are
    /// supported.
    pub fn off_chain() -> Self {
        Self {
            eip1271: false,
            presign: false,
        }
    }

    /// Returns a configuration where all signing schemes are enabled.
    pub fn all() -> Self {
        Self {
            eip1271: true,
            presign: true,
        }
    }

    /// returns whether the supplied signature scheme is supported.
    pub fn is_signing_scheme_supported(&self, signing_scheme: SigningScheme) -> bool {
        match signing_scheme {
            SigningScheme::Eip712 | SigningScheme::EthSign => true,
            SigningScheme::Eip1271 => self.eip1271,
            SigningScheme::PreSign => self.presign,
        }
    }
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
    // TODO: Note that we are pessimistic here for partially fillable orders,
    // since they don't need the full balance in order for the order to be
    // tradable. However, since they are currently only used for PMMs for
    // matching against user orders, it makes sense for the full sell token
    // amount balance to be required.
    order.sell_amount.checked_add(order.fee_amount)
}

/// Retrieves the quote for an order that is being created and verify that its
/// fee is sufficient.
///
/// This works by first trying to find an existing quote, and then falling back
/// to calculating a brand new one if none can be found and a quote ID was not
/// specified.
async fn get_quote_and_check_fee(
    quoter: &dyn OrderQuoting,
    order: &OrderCreation,
    order_placement_via_api: bool,
    owner: H160,
) -> Result<Quote, ValidationError> {
    let quote_kind =
        convert_signing_scheme_into_quote_kind(order.signature.scheme(), order_placement_via_api)?;
    let parameters = QuoteSearchParameters {
        sell_token: order.data.sell_token,
        buy_token: order.data.buy_token,
        sell_amount: order.data.sell_amount,
        buy_amount: order.data.buy_amount,
        fee_amount: order.data.fee_amount,
        kind: order.data.kind,
        from: owner,
        app_data: order.data.app_data,
        quote_kind,
    };

    let quote = match quoter.find_quote(order.quote_id, parameters).await {
        Ok(quote) => {
            tracing::debug!(quote_id =? order.quote_id, "found quote for order creation");
            quote
        }
        // We couldn't find a quote, and no ID was specified. Try computing a
        // fresh quote to use instead.
        Err(FindQuoteError::NotFound(_)) if order.quote_id.is_none() => {
            let signing_scheme = convert_signing_scheme_into_quote_signing_scheme(
                order.signature.scheme(),
                order_placement_via_api,
            )?;
            let parameters = QuoteParameters {
                sell_token: order.data.sell_token,
                buy_token: order.data.buy_token,
                side: match order.data.kind {
                    OrderKind::Buy => OrderQuoteSide::Buy {
                        buy_amount_after_fee: order.data.buy_amount,
                    },
                    OrderKind::Sell => OrderQuoteSide::Sell {
                        sell_amount: SellAmount::AfterFee {
                            value: order.data.sell_amount,
                        },
                    },
                },
                from: owner,
                app_data: order.data.app_data,
                signing_scheme,
            };
            let quote = quoter.calculate_quote(parameters).await?;

            tracing::debug!("computed fresh quote for order creation");
            quote
        }
        Err(err) => return Err(err.into()),
    };

    if order.data.fee_amount < quote.fee_amount {
        return Err(ValidationError::InsufficientFee);
    }

    Ok(quote)
}

/// Checks whether or not an order's limit price is outside the market price
/// specified by the quote.
///
/// Note that this check only looks at the order's limit price and the market
/// price and is independent of amounts or trade direction.
fn is_order_outside_market_price(order: &OrderData, quote: &Quote) -> bool {
    order.sell_amount.full_mul(quote.buy_amount) < quote.sell_amount.full_mul(order.buy_amount)
}

fn convert_signing_scheme_into_quote_signing_scheme(
    scheme: SigningScheme,
    order_placement_via_api: bool,
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
        }),
    }
}

fn convert_signing_scheme_into_quote_kind(
    scheme: SigningScheme,
    order_placement_via_api: bool,
) -> Result<QuoteKind, ValidationError> {
    match order_placement_via_api {
        true => Ok(QuoteKind::Standard),
        false => match scheme {
            SigningScheme::Eip1271 => Ok(QuoteKind::Eip1271OnchainOrder),
            SigningScheme::PreSign => Ok(QuoteKind::PreSignOnchainOrder),
            _ => Err(ValidationError::IncompatibleSigningScheme),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        account_balances::MockBalanceFetching,
        bad_token::{MockBadTokenDetecting, TokenQuality},
        dummy_contract,
        order_quoting::MockOrderQuoting,
        rate_limiter::RateLimiterError,
        signature_validator::MockSignatureValidating,
        web3_traits::MockCodeFetching,
    };
    use anyhow::anyhow;
    use chrono::Utc;
    use ethcontract::web3::signing::SecretKeyRef;
    use maplit::hashset;
    use mockall::predicate::{always, eq};
    use model::{
        app_id::AppId,
        order::OrderBuilder,
        signature::{EcdsaSignature, EcdsaSigningScheme},
    };
    use secp256k1::ONE_KEY;

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
        let mut code_fetcher = Box::new(MockCodeFetching::new());
        let native_token = dummy_contract!(WETH9, [0xef; 20]);
        let min_order_validity_period = Duration::from_secs(1);
        let max_order_validity_period = Duration::from_secs(100);
        let banned_users = hashset![H160::from_low_u64_be(1)];
        let legit_valid_to =
            model::time::now_in_epoch_seconds() + min_order_validity_period.as_secs() as u32 + 2;
        code_fetcher
            .expect_code_size()
            .times(1)
            .return_once(|_| Ok(1));
        let validator = OrderValidator::new(
            code_fetcher,
            native_token,
            banned_users,
            hashset!(),
            min_order_validity_period,
            max_order_validity_period,
            SignatureConfiguration::off_chain(),
            Arc::new(MockBadTokenDetecting::new()),
            Arc::new(MockOrderQuoting::new()),
            Arc::new(MockBalanceFetching::new()),
            Arc::new(MockSignatureValidating::new()),
        );
        assert!(matches!(
            validator
                .partial_validate(PreOrderData {
                    partially_fillable: true,
                    ..Default::default()
                })
                .await,
            Err(PartialValidationError::UnsupportedOrderType)
        ));
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
            Err(PartialValidationError::InsufficientValidTo)
        ));
        assert!(matches!(
            validator
                .partial_validate(PreOrderData {
                    valid_to: legit_valid_to + max_order_validity_period.as_secs() as u32 + 1,
                    ..Default::default()
                })
                .await,
            Err(PartialValidationError::ExcessiveValidTo)
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
                    buy_token: BUY_ETH_ADDRESS,
                    ..Default::default()
                })
                .await,
            Err(PartialValidationError::TransferEthToContract)
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
        assert!(matches!(
            validator
                .partial_validate(PreOrderData {
                    valid_to: legit_valid_to,
                    signing_scheme: SigningScheme::PreSign,
                    ..Default::default()
                })
                .await,
            Err(PartialValidationError::UnsupportedSignature)
        ));

        let mut code_fetcher = Box::new(MockCodeFetching::new());
        let _err = anyhow!("Failed to fetch Code Size!");
        code_fetcher
            .expect_code_size()
            .times(1)
            .return_once(|_| Err(_err));
        let validator = OrderValidator::new(
            code_fetcher,
            dummy_contract!(WETH9, [0xef; 20]),
            hashset!(),
            hashset!(),
            Duration::from_secs(1),
            Duration::from_secs(100),
            SignatureConfiguration::off_chain(),
            Arc::new(MockBadTokenDetecting::new()),
            Arc::new(MockOrderQuoting::new()),
            Arc::new(MockBalanceFetching::new()),
            Arc::new(MockSignatureValidating::new()),
        );

        assert!(matches!(
            validator
                .partial_validate(PreOrderData {
                    valid_to: legit_valid_to,
                    buy_token: BUY_ETH_ADDRESS,
                    ..Default::default()
                })
                .await,
            Err(PartialValidationError::Other(_err))
        ));
    }

    #[tokio::test]
    async fn pre_validate_ok() {
        let liquidity_order_owner = H160::from_low_u64_be(0x42);
        let min_order_validity_period = Duration::from_secs(1);
        let max_order_validity_period = Duration::from_secs(100);

        let mut bad_token_detector = MockBadTokenDetecting::new();
        bad_token_detector
            .expect_detect()
            .with(eq(H160::from_low_u64_be(1)))
            .returning(|_| Ok(TokenQuality::Good));
        bad_token_detector
            .expect_detect()
            .with(eq(H160::from_low_u64_be(2)))
            .returning(|_| Ok(TokenQuality::Good));

        let validator = OrderValidator::new(
            Box::new(MockCodeFetching::new()),
            dummy_contract!(WETH9, [0xef; 20]),
            hashset!(),
            hashset!(liquidity_order_owner),
            min_order_validity_period,
            max_order_validity_period,
            SignatureConfiguration::all(),
            Arc::new(bad_token_detector),
            Arc::new(MockOrderQuoting::new()),
            Arc::new(MockBalanceFetching::new()),
            Arc::new(MockSignatureValidating::new()),
        );
        let order = || PreOrderData {
            valid_to: model::time::now_in_epoch_seconds()
                + min_order_validity_period.as_secs() as u32
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
                partially_fillable: true,
                is_liquidity_order: true,
                owner: liquidity_order_owner,
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
            .returning(|_, _, _, _| Ok(()));

        let validator = OrderValidator::new(
            Box::new(MockCodeFetching::new()),
            dummy_contract!(WETH9, [0xef; 20]),
            hashset!(),
            hashset!(),
            Duration::from_secs(1),
            Duration::from_secs(100),
            SignatureConfiguration::all(),
            Arc::new(bad_token_detector),
            Arc::new(order_quoter),
            Arc::new(balance_fetcher),
            Arc::new(MockSignatureValidating::new()),
        );

        let creation = OrderCreation {
            data: OrderData {
                valid_to: model::time::now_in_epoch_seconds() + 2,
                sell_token: H160::from_low_u64_be(1),
                buy_token: H160::from_low_u64_be(2),
                buy_amount: U256::from(1),
                sell_amount: U256::from(1),
                ..Default::default()
            },
            ..Default::default()
        };
        let (order, _) = validator
            .validate_and_construct_order(creation.clone(), &Default::default(), Default::default())
            .await
            .unwrap();
        assert_eq!(order.metadata.full_fee_amount, order.data.fee_amount);

        let domain_separator = DomainSeparator::default();
        let creation = OrderCreation {
            from: Some(H160([1; 20])),
            signature: Signature::Eip1271(vec![1, 2, 3]),
            ..creation
        };
        let order_hash = hashed_eip712_message(&domain_separator, &creation.data.hash_struct());

        let mut signature_validator = MockSignatureValidating::new();
        signature_validator
            .expect_validate_signature()
            .with(eq(SignatureCheck {
                signer: creation.from.unwrap(),
                hash: order_hash,
                signature: vec![1, 2, 3],
            }))
            .returning(|_| Ok(()));

        let validator = OrderValidator {
            signature_validator: Arc::new(signature_validator),
            ..validator
        };

        assert!(validator
            .validate_and_construct_order(creation, &domain_separator, Default::default())
            .await
            .is_ok());
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
            .returning(|_, _, _, _| Ok(()));
        let validator = OrderValidator::new(
            Box::new(MockCodeFetching::new()),
            dummy_contract!(WETH9, [0xef; 20]),
            hashset!(),
            hashset!(),
            Duration::from_secs(1),
            Duration::from_secs(100),
            SignatureConfiguration::all(),
            Arc::new(bad_token_detector),
            Arc::new(order_quoter),
            Arc::new(balance_fetcher),
            Arc::new(MockSignatureValidating::new()),
        );
        let order = OrderCreation {
            data: OrderData {
                valid_to: model::time::now_in_epoch_seconds() + 2,
                sell_token: H160::from_low_u64_be(1),
                buy_token: H160::from_low_u64_be(2),
                buy_amount: U256::from(0),
                sell_amount: U256::from(0),
                ..Default::default()
            },
            ..Default::default()
        };
        let result = validator
            .validate_and_construct_order(order, &Default::default(), Default::default())
            .await;
        dbg!(&result);
        assert!(matches!(result, Err(ValidationError::ZeroAmount)));
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
            .returning(|_, _, _, _| Ok(()));
        let validator = OrderValidator::new(
            Box::new(MockCodeFetching::new()),
            dummy_contract!(WETH9, [0xef; 20]),
            hashset!(),
            hashset!(),
            Duration::from_secs(1),
            Duration::from_secs(100),
            SignatureConfiguration::all(),
            Arc::new(bad_token_detector),
            Arc::new(order_quoter),
            Arc::new(balance_fetcher),
            Arc::new(MockSignatureValidating::new()),
        );
        let order = OrderCreation {
            data: OrderData {
                valid_to: model::time::now_in_epoch_seconds() + 2,
                sell_token: H160::from_low_u64_be(1),
                buy_token: H160::from_low_u64_be(2),
                buy_amount: U256::from(1),
                sell_amount: U256::from(1),
                ..Default::default()
            },
            from: Some(Default::default()),
            ..Default::default()
        };
        let result = validator
            .validate_and_construct_order(order, &Default::default(), Default::default())
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
            .returning(|_, _, _, _| Ok(()));
        let validator = OrderValidator::new(
            Box::new(MockCodeFetching::new()),
            dummy_contract!(WETH9, [0xef; 20]),
            hashset!(),
            hashset!(),
            Duration::from_secs(1),
            Duration::from_secs(100),
            SignatureConfiguration::all(),
            Arc::new(bad_token_detector),
            Arc::new(order_quoter),
            Arc::new(balance_fetcher),
            Arc::new(MockSignatureValidating::new()),
        );
        let order = OrderCreation {
            data: OrderData {
                valid_to: model::time::now_in_epoch_seconds() + 2,
                sell_token: H160::from_low_u64_be(1),
                buy_token: H160::from_low_u64_be(2),
                buy_amount: U256::from(1),
                sell_amount: U256::from(1),
                ..Default::default()
            },
            ..Default::default()
        };
        let result = validator
            .validate_and_construct_order(order, &Default::default(), Default::default())
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
            .returning(|_, _, _, _| Ok(()));
        let validator = OrderValidator::new(
            Box::new(MockCodeFetching::new()),
            dummy_contract!(WETH9, [0xef; 20]),
            hashset!(),
            hashset!(),
            Duration::from_secs(1),
            Duration::from_secs(100),
            SignatureConfiguration::all(),
            Arc::new(bad_token_detector),
            Arc::new(order_quoter),
            Arc::new(balance_fetcher),
            Arc::new(MockSignatureValidating::new()),
        );
        let order = OrderCreation {
            data: OrderData {
                valid_to: model::time::now_in_epoch_seconds() + 2,
                sell_token: H160::from_low_u64_be(1),
                buy_token: H160::from_low_u64_be(2),
                buy_amount: U256::from(1),
                sell_amount: U256::from(1),
                ..Default::default()
            },
            ..Default::default()
        };
        let result = validator
            .validate_and_construct_order(order, &Default::default(), Default::default())
            .await;
        dbg!(&result);
        assert!(matches!(
            result,
            Err(ValidationError::Partial(
                PartialValidationError::UnsupportedToken(_)
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
        bad_token_detector
            .expect_detect()
            .returning(|_| Ok(TokenQuality::Good));
        balance_fetcher
            .expect_can_transfer()
            .returning(|_, _, _, _| Ok(()));
        let validator = OrderValidator::new(
            Box::new(MockCodeFetching::new()),
            dummy_contract!(WETH9, [0xef; 20]),
            hashset!(),
            hashset!(),
            Duration::from_secs(1),
            Duration::from_secs(100),
            SignatureConfiguration::all(),
            Arc::new(bad_token_detector),
            Arc::new(order_quoter),
            Arc::new(balance_fetcher),
            Arc::new(MockSignatureValidating::new()),
        );
        let order = OrderCreation {
            data: OrderData {
                valid_to: model::time::now_in_epoch_seconds() + 2,
                sell_token: H160::from_low_u64_be(1),
                buy_token: H160::from_low_u64_be(2),
                buy_amount: U256::from(1),
                sell_amount: U256::MAX,
                fee_amount: U256::from(1),
                ..Default::default()
            },
            ..Default::default()
        };
        let result = validator
            .validate_and_construct_order(order, &Default::default(), Default::default())
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
            .returning(|_, _, _, _| Err(TransferSimulationError::InsufficientBalance));
        let validator = OrderValidator::new(
            Box::new(MockCodeFetching::new()),
            dummy_contract!(WETH9, [0xef; 20]),
            hashset!(),
            hashset!(),
            Duration::from_secs(1),
            Duration::from_secs(100),
            SignatureConfiguration::all(),
            Arc::new(bad_token_detector),
            Arc::new(order_quoter),
            Arc::new(balance_fetcher),
            Arc::new(MockSignatureValidating::new()),
        );
        let order = OrderCreation {
            data: OrderData {
                valid_to: model::time::now_in_epoch_seconds() + 2,
                sell_token: H160::from_low_u64_be(1),
                buy_token: H160::from_low_u64_be(2),
                buy_amount: U256::from(1),
                sell_amount: U256::from(1),
                ..Default::default()
            },
            ..Default::default()
        };
        let result = validator
            .validate_and_construct_order(order, &Default::default(), Default::default())
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
            .returning(|_, _, _, _| Ok(()));
        signature_validator
            .expect_validate_signature()
            .returning(|_| Err(SignatureValidationError::Invalid));

        let validator = OrderValidator::new(
            Box::new(MockCodeFetching::new()),
            dummy_contract!(WETH9, [0xef; 20]),
            hashset!(),
            hashset!(),
            Duration::from_secs(1),
            Duration::from_secs(100),
            SignatureConfiguration::all(),
            Arc::new(bad_token_detector),
            Arc::new(order_quoter),
            Arc::new(balance_fetcher),
            Arc::new(signature_validator),
        );

        let creation = OrderCreation {
            data: OrderData {
                valid_to: model::time::now_in_epoch_seconds() + 2,
                sell_token: H160::from_low_u64_be(1),
                buy_token: H160::from_low_u64_be(2),
                buy_amount: U256::from(1),
                sell_amount: U256::from(1),
                ..Default::default()
            },
            from: Some(H160([1; 20])),
            signature: Signature::Eip1271(vec![1, 2, 3]),
            ..Default::default()
        };

        assert!(matches!(
            validator
                .validate_and_construct_order(creation, &Default::default(), Default::default())
                .await
                .unwrap_err(),
            ValidationError::InvalidSignature,
        ));
    }

    #[tokio::test]
    async fn allows_insufficient_allowance_and_balance_for_presign_orders() {
        macro_rules! assert_allows_failed_transfer {
            ($err:ident) => {
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
                    .returning(|_, _, _, _| Err(TransferSimulationError::$err));
                let validator = OrderValidator::new(
                    Box::new(MockCodeFetching::new()),
                    dummy_contract!(WETH9, [0xef; 20]),
                    hashset!(),
                    hashset!(),
                    Duration::from_secs(1),
                    Duration::MAX,
                    SignatureConfiguration::all(),
                    Arc::new(bad_token_detector),
                    Arc::new(order_quoter),
                    Arc::new(balance_fetcher),
                    Arc::new(MockSignatureValidating::new()),
                );

                let order = OrderBuilder::default()
                    .with_valid_to(u32::MAX)
                    .with_sell_token(H160::from_low_u64_be(1))
                    .with_sell_amount(1.into())
                    .with_buy_token(H160::from_low_u64_be(2))
                    .with_buy_amount(1.into());

                for signing_scheme in [EcdsaSigningScheme::Eip712, EcdsaSigningScheme::EthSign] {
                    assert!(matches!(
                        validator
                            .validate_and_construct_order(
                                order
                                    .clone()
                                    .sign_with(
                                        signing_scheme,
                                        &Default::default(),
                                        SecretKeyRef::new(&ONE_KEY)
                                    )
                                    .build()
                                    .into(),
                                &Default::default(),
                                Default::default()
                            )
                            .await,
                        Err(ValidationError::$err)
                    ));
                }

                assert!(matches!(
                    validator
                        .validate_and_construct_order(
                            order.with_presign(Default::default()).build().into(),
                            &Default::default(),
                            Default::default()
                        )
                        .await,
                    Ok(_)
                ));
            };
        }

        assert_allows_failed_transfer!(InsufficientAllowance);
        assert_allows_failed_transfer!(InsufficientBalance);
    }

    #[tokio::test]
    async fn get_quote_find_by_id() {
        let order = OrderCreation {
            data: OrderData {
                sell_token: H160([1; 20]),
                buy_token: H160([2; 20]),
                sell_amount: 3.into(),
                buy_amount: 4.into(),
                app_data: AppId([5; 32]),
                fee_amount: 6.into(),
                kind: OrderKind::Buy,
                ..Default::default()
            },
            quote_id: Some(42),
            ..Default::default()
        };
        let from = H160([0xf0; 20]);

        let mut order_quoter = MockOrderQuoting::new();
        order_quoter
            .expect_find_quote()
            .with(
                eq(Some(42)),
                eq(QuoteSearchParameters {
                    sell_token: H160([1; 20]),
                    buy_token: H160([2; 20]),
                    sell_amount: 3.into(),
                    buy_amount: 4.into(),
                    fee_amount: 6.into(),
                    kind: OrderKind::Buy,
                    from: H160([0xf0; 20]),
                    app_data: AppId([5; 32]),
                    quote_kind: QuoteKind::Standard,
                }),
            )
            .returning(|_, _| {
                Ok(Quote {
                    fee_amount: 6.into(),
                    ..Default::default()
                })
            });

        let quote = get_quote_and_check_fee(&order_quoter, &order, true, from)
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
        let order = OrderCreation {
            data: OrderData {
                sell_token: H160([1; 20]),
                buy_token: H160([2; 20]),
                sell_amount: 3.into(),
                buy_amount: 4.into(),
                app_data: AppId([5; 32]),
                fee_amount: 6.into(),
                kind: OrderKind::Sell,
                ..Default::default()
            },
            quote_id: None,
            ..Default::default()
        };
        let from = H160([0xf0; 20]);

        let mut order_quoter = MockOrderQuoting::new();
        order_quoter
            .expect_find_quote()
            .with(eq(None), always())
            .returning(|_, _| Err(FindQuoteError::NotFound(None)));
        order_quoter
            .expect_calculate_quote()
            .with(eq(QuoteParameters {
                sell_token: H160([1; 20]),
                buy_token: H160([2; 20]),
                side: OrderQuoteSide::Sell {
                    sell_amount: SellAmount::AfterFee { value: 3.into() },
                },
                from,
                app_data: AppId([5; 32]),
                signing_scheme: QuoteSigningScheme::Eip712,
            }))
            .returning(|_| {
                Ok(Quote {
                    fee_amount: 6.into(),
                    ..Default::default()
                })
            });

        let quote = get_quote_and_check_fee(&order_quoter, &order, true, from)
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
    async fn get_quote_errors_when_not_found_by_id() {
        let order = OrderCreation {
            quote_id: Some(0),
            ..Default::default()
        };

        let mut order_quoter = MockOrderQuoting::new();
        order_quoter
            .expect_find_quote()
            .returning(|_, _| Err(FindQuoteError::NotFound(Some(0))));

        let err = get_quote_and_check_fee(&order_quoter, &order, true, Default::default())
            .await
            .unwrap_err();

        assert!(matches!(err, ValidationError::QuoteNotFound));
    }

    #[tokio::test]
    async fn get_quote_errors_in_case_of_api_call_and_quote_requested_for_onchain_orders() {
        let order = OrderCreation {
            data: OrderData {
                fee_amount: 1.into(),
                ..Default::default()
            },
            signature: Signature::Eip712(EcdsaSignature::default()),
            ..Default::default()
        };

        let order_quoter = MockOrderQuoting::new();

        let err = get_quote_and_check_fee(&order_quoter, &order, false, Default::default())
            .await
            .unwrap_err();

        assert!(matches!(err, ValidationError::IncompatibleSigningScheme));
    }

    #[tokio::test]
    async fn get_quote_errors_on_insufficient_fees() {
        let order = OrderCreation {
            data: OrderData {
                fee_amount: 1.into(),
                ..Default::default()
            },
            ..Default::default()
        };

        let mut order_quoter = MockOrderQuoting::new();
        order_quoter.expect_find_quote().returning(|_, _| {
            Ok(Quote {
                fee_amount: 2.into(),
                ..Default::default()
            })
        });

        let err = get_quote_and_check_fee(&order_quoter, &order, true, Default::default())
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
                    &Default::default(),
                    true,
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
                    &Default::default(),
                    true,
                    Default::default(),
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
            CalculateQuoteError::Price(PriceEstimationError::UnsupportedToken(Default::default())),
            ValidationError::Partial(PartialValidationError::UnsupportedToken(_))
        );
        assert_calc_error_matches!(
            CalculateQuoteError::Price(PriceEstimationError::ZeroAmount),
            ValidationError::ZeroAmount
        );
        assert_calc_error_matches!(
            CalculateQuoteError::Price(PriceEstimationError::NoLiquidity),
            ValidationError::PriceForQuote(_)
        );
        assert_calc_error_matches!(
            CalculateQuoteError::Price(PriceEstimationError::UnsupportedOrderType),
            ValidationError::PriceForQuote(_)
        );
        assert_calc_error_matches!(
            CalculateQuoteError::Price(PriceEstimationError::RateLimited(
                RateLimiterError::RateLimited
            )),
            ValidationError::PriceForQuote(_)
        );
    }

    #[test]
    fn detects_market_orders() {
        let quote = Quote {
            sell_amount: 100.into(),
            buy_amount: 100.into(),
            ..Default::default()
        };

        // *** SELL ORDERS ***
        // at market price
        assert!(!is_order_outside_market_price(
            &OrderData {
                sell_amount: 100.into(),
                buy_amount: 100.into(),
                kind: OrderKind::Sell,
                ..Default::default()
            },
            &quote,
        ));
        // willing to buy less than market price
        assert!(!is_order_outside_market_price(
            &OrderData {
                sell_amount: 100.into(),
                buy_amount: 99.into(), // 1% slippage
                kind: OrderKind::Sell,
                ..Default::default()
            },
            &quote,
        ));
        // wanting to buy more than market price
        assert!(is_order_outside_market_price(
            &OrderData {
                sell_amount: 100.into(),
                buy_amount: 1000.into(),
                kind: OrderKind::Sell,
                ..Default::default()
            },
            &quote
        ));

        // *** BUY ORDERS ***
        // at market price
        assert!(!is_order_outside_market_price(
            &OrderData {
                sell_amount: 100.into(),
                buy_amount: 100.into(),
                kind: OrderKind::Buy,
                ..Default::default()
            },
            &quote,
        ));
        // willing to sell more than market price
        assert!(!is_order_outside_market_price(
            &OrderData {
                sell_amount: 101.into(), // 1% slippage
                buy_amount: 100.into(),
                kind: OrderKind::Buy,
                ..Default::default()
            },
            &quote,
        ));
        // wanting to sell less than market price
        assert!(is_order_outside_market_price(
            &OrderData {
                sell_amount: 1.into(),
                buy_amount: 100.into(),
                kind: OrderKind::Buy,
                ..Default::default()
            },
            &quote
        ));
    }
}
