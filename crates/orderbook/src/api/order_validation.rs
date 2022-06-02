use crate::{
    account_balances::{BalanceFetching, TransferSimulationError},
    api::IntoWarpReply,
    fee::{FeeData, FeeParameters, GetUnsubsidizedMinFeeError, MinFeeCalculating},
};
use contracts::WETH9;
use ethcontract::{H160, U256};
use model::{
    order::{
        BuyTokenDestination, Order, OrderCreation, OrderKind, SellTokenSource, BUY_ETH_ADDRESS,
    },
    signature::SigningScheme,
    DomainSeparator,
};
use shared::{
    bad_token::BadTokenDetecting, price_estimation::PriceEstimationError, web3_traits::CodeFetching,
};
use std::{collections::HashSet, sync::Arc, time::Duration};
use warp::{http::StatusCode, reply::with_status};

#[cfg_attr(test, mockall::automock)]
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
    async fn partial_validate(&self, order: PreOrderData) -> Result<(), PartialValidationError>;

    /// This is the full order validation performed at the time of order placement
    /// (i.e. once all the required fields on an Order are provided). Specifically, verifying that
    ///     - buy & sell amounts are non-zero,
    ///     - order's owner matches the from field (if specified),
    ///     - fee is sufficient,
    ///     - buy & sell tokens passed "bad token" detection,
    ///     - user has sufficient (transferable) funds to execute the order.
    ///
    /// Furthermore, full order validation also calls partial_validate to ensure that
    /// other aspects of the order are not malformed.
    async fn validate_and_construct_order(
        &self,
        order_creation: OrderCreation,
        sender: Option<H160>,
        domain_separator: &DomainSeparator,
        settlement_contract: H160,
    ) -> Result<(Order, FeeParameters), ValidationError>;
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
    Other(anyhow::Error),
}

impl IntoWarpReply for PartialValidationError {
    fn into_warp_reply(self) -> super::ApiReply {
        match self {
            Self::UnsupportedBuyTokenDestination(dest) => with_status(
                super::error("UnsupportedBuyTokenDestination", format!("Type {:?}", dest)),
                StatusCode::BAD_REQUEST,
            ),
            Self::UnsupportedSellTokenSource(src) => with_status(
                super::error("UnsupportedSellTokenSource", format!("Type {:?}", src)),
                StatusCode::BAD_REQUEST,
            ),
            Self::UnsupportedOrderType => with_status(
                super::error(
                    "UnsupportedOrderType",
                    "This order type is currently not supported",
                ),
                StatusCode::BAD_REQUEST,
            ),
            Self::Forbidden => with_status(
                super::error("Forbidden", "Forbidden, your account is deny-listed"),
                StatusCode::FORBIDDEN,
            ),
            Self::InsufficientValidTo => with_status(
                super::error(
                    "InsufficientValidTo",
                    "validTo is not far enough in the future",
                ),
                StatusCode::BAD_REQUEST,
            ),
            Self::ExcessiveValidTo => with_status(
                super::error("ExcessiveValidTo", "validTo is too far into the future"),
                StatusCode::BAD_REQUEST,
            ),
            Self::TransferEthToContract => with_status(
                super::error(
                    "TransferEthToContract",
                    "Sending Ether to smart contract wallets is currently not supported",
                ),
                StatusCode::BAD_REQUEST,
            ),
            Self::InvalidNativeSellToken => with_status(
                super::error(
                    "InvalidNativeSellToken",
                    "The chain's native token (Ether/xDai) cannot be used as the sell token",
                ),
                StatusCode::BAD_REQUEST,
            ),
            Self::SameBuyAndSellToken => with_status(
                super::error(
                    "SameBuyAndSellToken",
                    "Buy token is the same as the sell token.",
                ),
                StatusCode::BAD_REQUEST,
            ),
            Self::UnsupportedSignature => with_status(
                super::error("UnsupportedSignature", "signing scheme is not supported"),
                StatusCode::BAD_REQUEST,
            ),
            Self::Other(err) => with_status(
                super::internal_error(err.context("partial_validation")),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
        }
    }
}

#[derive(Debug)]
pub enum ValidationError {
    Partial(PartialValidationError),
    InsufficientFee,
    InsufficientBalance,
    InsufficientAllowance,
    InvalidSignature,
    // If fee and sell amount overflow u256
    SellAmountOverflow,
    TransferSimulationFailed,
    UnsupportedToken(H160),
    WrongOwner(H160),
    ZeroAmount,
    Other(anyhow::Error),
}

impl IntoWarpReply for ValidationError {
    fn into_warp_reply(self) -> super::ApiReply {
        match self {
            ValidationError::Partial(pre) => pre.into_warp_reply(),
            Self::UnsupportedToken(token) => with_status(
                super::error("UnsupportedToken", format!("Token address {}", token)),
                StatusCode::BAD_REQUEST,
            ),
            Self::WrongOwner(owner) => with_status(
                super::error(
                    "WrongOwner",
                    format!(
                        "Address recovered from signature {} does not match from address",
                        owner
                    ),
                ),
                StatusCode::BAD_REQUEST,
            ),
            Self::InsufficientBalance => with_status(
                super::error(
                    "InsufficientBalance",
                    "order owner must have funds worth at least x in his account",
                ),
                StatusCode::BAD_REQUEST,
            ),
            Self::InsufficientAllowance => with_status(
                super::error(
                    "InsufficientAllowance",
                    "order owner must give allowance to VaultRelayer",
                ),
                StatusCode::BAD_REQUEST,
            ),
            Self::InvalidSignature => with_status(
                super::error("InvalidSignature", "invalid signature"),
                StatusCode::BAD_REQUEST,
            ),
            Self::InsufficientFee => with_status(
                super::error("InsufficientFee", "Order does not include sufficient fee"),
                StatusCode::BAD_REQUEST,
            ),
            Self::SellAmountOverflow => with_status(
                super::error(
                    "SellAmountOverflow",
                    "Sell amount + fee amount must fit in U256",
                ),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
            Self::TransferSimulationFailed => with_status(
                super::error(
                    "TransferSimulationFailed",
                    "sell token cannot be transferred",
                ),
                StatusCode::BAD_REQUEST,
            ),
            Self::ZeroAmount => with_status(
                super::error("ZeroAmount", "Buy or sell amount is zero."),
                StatusCode::BAD_REQUEST,
            ),
            Self::Other(err) => with_status(
                super::internal_error(err.context("order_validation")),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
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
    enable_presign_orders: bool,
    /// For Full-Validation: performed time of order placement
    fee_validator: Arc<dyn MinFeeCalculating>,
    bad_token_detector: Arc<dyn BadTokenDetecting>,
    balance_fetcher: Arc<dyn BalanceFetching>,
}

#[derive(Debug, PartialEq, Default)]
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

fn actual_receiver(owner: H160, order: &OrderCreation) -> H160 {
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
        order: &OrderCreation,
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
            signing_scheme: order.signature.scheme(),
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
        enable_presign_orders: bool,
        fee_validator: Arc<dyn MinFeeCalculating>,
        bad_token_detector: Arc<dyn BadTokenDetecting>,
        balance_fetcher: Arc<dyn BalanceFetching>,
    ) -> Self {
        Self {
            code_fetcher,
            native_token,
            banned_users,
            liquidity_order_owners,
            min_order_validity_period,
            max_order_validity_period,
            enable_presign_orders,
            fee_validator,
            bad_token_detector,
            balance_fetcher,
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
        if !matches!(
            (order.signing_scheme, self.enable_presign_orders),
            (SigningScheme::Eip712 | SigningScheme::EthSign, _) | (SigningScheme::PreSign, true)
        ) {
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
        Ok(())
    }

    async fn validate_and_construct_order(
        &self,
        order_creation: OrderCreation,
        sender: Option<H160>,
        domain_separator: &DomainSeparator,
        settlement_contract: H160,
    ) -> Result<(Order, FeeParameters), ValidationError> {
        let owner = order_creation
            .signature
            .validate(domain_separator, &order_creation.hash_struct())
            .ok_or(ValidationError::InvalidSignature)?;

        if order_creation.buy_amount.is_zero() || order_creation.sell_amount.is_zero() {
            return Err(ValidationError::ZeroAmount);
        }
        if matches!(sender, Some(from) if from != owner) {
            return Err(ValidationError::WrongOwner(owner));
        }
        for &token in &[order_creation.sell_token, order_creation.buy_token] {
            if !self
                .bad_token_detector
                .detect(token)
                .await
                .map_err(ValidationError::Other)?
                .is_good()
            {
                return Err(ValidationError::UnsupportedToken(token));
            }
        }

        let is_liquidity_order = self.liquidity_order_owners.contains(&owner);
        self.partial_validate(PreOrderData::from_order_creation(
            owner,
            &order_creation,
            is_liquidity_order,
        ))
        .await
        .map_err(ValidationError::Partial)?;

        let unsubsidized_fee = self
            .fee_validator
            .get_unsubsidized_min_fee(
                FeeData {
                    sell_token: order_creation.sell_token,
                    buy_token: order_creation.buy_token,
                    amount: match order_creation.kind {
                        OrderKind::Buy => order_creation.buy_amount,
                        OrderKind::Sell => order_creation.sell_amount,
                    },
                    kind: order_creation.kind,
                },
                order_creation.app_data,
                order_creation.fee_amount,
                owner,
            )
            .await
            .map_err(|err| match err {
                GetUnsubsidizedMinFeeError::Other(err) => ValidationError::Other(err),
                GetUnsubsidizedMinFeeError::PriceEstimationError(PriceEstimationError::Other(
                    err,
                )) => ValidationError::Other(err),
                GetUnsubsidizedMinFeeError::InsufficientFee => ValidationError::InsufficientFee,
                // Some of the possible errors here have been already checked in this function or
                // should have been checked when the order was pre-validated. There is no good way
                // and not much need to bubble them up and we don't want to error log for them so
                // treat them as insufficient fee.
                GetUnsubsidizedMinFeeError::PriceEstimationError(_) => {
                    ValidationError::InsufficientFee
                }
            })?;

        let min_balance = match minimum_balance(&order_creation) {
            Some(amount) => amount,
            None => return Err(ValidationError::SellAmountOverflow),
        };

        // Fast path to check if transfer is possible with a single node query.
        // If not, run extra queries for additional information.
        match self
            .balance_fetcher
            .can_transfer(
                order_creation.sell_token,
                owner,
                min_balance,
                order_creation.sell_token_balance,
            )
            .await
        {
            Ok(_) => (),
            Err(
                TransferSimulationError::InsufficientAllowance
                | TransferSimulationError::InsufficientBalance,
            ) if order_creation.signature.scheme() == SigningScheme::PreSign => {
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

        let order = Order::from_order_creation(
            &order_creation,
            domain_separator,
            settlement_contract,
            unsubsidized_fee.amount_in_sell_token(),
            owner,
            is_liquidity_order,
        );
        Ok((order, unsubsidized_fee))
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
fn minimum_balance(order: &OrderCreation) -> Option<U256> {
    // TODO: Note that we are pessimistic here for partially fillable orders,
    // since they don't need the full balance in order for the order to be
    // tradable. However, since they are currently only used for PMMs for
    // matching against user orders, it makes sense for the full sell token
    // amount balance to be required.
    order.sell_amount.checked_add(order.fee_amount)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        account_balances::MockBalanceFetching,
        fee::{GetUnsubsidizedMinFeeError, MockMinFeeCalculating},
    };
    use anyhow::anyhow;
    use ethcontract::web3::signing::SecretKeyRef;
    use maplit::hashset;
    use model::{order::OrderBuilder, signature::EcdsaSigningScheme};
    use secp256k1::ONE_KEY;
    use shared::{
        bad_token::{MockBadTokenDetecting, TokenQuality},
        dummy_contract,
        web3_traits::MockCodeFetching,
    };

    #[test]
    fn minimum_balance_() {
        let order = OrderCreation {
            sell_amount: U256::MAX,
            fee_amount: U256::from(1),
            ..Default::default()
        };
        assert_eq!(minimum_balance(&order), None);
        let order = OrderCreation {
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
            false,
            Arc::new(MockMinFeeCalculating::new()),
            Arc::new(MockBadTokenDetecting::new()),
            Arc::new(MockBalanceFetching::new()),
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
            false,
            Arc::new(MockMinFeeCalculating::new()),
            Arc::new(MockBadTokenDetecting::new()),
            Arc::new(MockBalanceFetching::new()),
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
        let validator = OrderValidator::new(
            Box::new(MockCodeFetching::new()),
            dummy_contract!(WETH9, [0xef; 20]),
            hashset!(),
            hashset!(liquidity_order_owner),
            min_order_validity_period,
            max_order_validity_period,
            true,
            Arc::new(MockMinFeeCalculating::new()),
            Arc::new(MockBadTokenDetecting::new()),
            Arc::new(MockBalanceFetching::new()),
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
        let mut fee_calculator = MockMinFeeCalculating::new();
        let mut bad_token_detector = MockBadTokenDetecting::new();
        let mut balance_fetcher = MockBalanceFetching::new();
        fee_calculator
            .expect_get_unsubsidized_min_fee()
            .returning(|_, _, _, _| Ok(Default::default()));
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
            true,
            Arc::new(fee_calculator),
            Arc::new(bad_token_detector),
            Arc::new(balance_fetcher),
        );
        let order = OrderCreation {
            valid_to: model::time::now_in_epoch_seconds() + 2,
            sell_token: H160::from_low_u64_be(1),
            buy_token: H160::from_low_u64_be(2),
            buy_amount: U256::from(1),
            sell_amount: U256::from(1),
            ..Default::default()
        };
        let (order, _) = validator
            .validate_and_construct_order(order, None, &Default::default(), Default::default())
            .await
            .unwrap();
        assert_eq!(order.metadata.full_fee_amount, order.creation.fee_amount);
    }

    #[tokio::test]
    async fn post_validate_err_zero_amount() {
        let mut fee_calculator = MockMinFeeCalculating::new();
        let mut bad_token_detector = MockBadTokenDetecting::new();
        let mut balance_fetcher = MockBalanceFetching::new();
        fee_calculator
            .expect_get_unsubsidized_min_fee()
            .returning(|_, _, _, _| Ok(Default::default()));
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
            true,
            Arc::new(fee_calculator),
            Arc::new(bad_token_detector),
            Arc::new(balance_fetcher),
        );
        let order = OrderCreation {
            valid_to: model::time::now_in_epoch_seconds() + 2,
            sell_token: H160::from_low_u64_be(1),
            buy_token: H160::from_low_u64_be(2),
            buy_amount: U256::from(0),
            sell_amount: U256::from(0),
            ..Default::default()
        };
        let result = validator
            .validate_and_construct_order(order, None, &Default::default(), Default::default())
            .await;
        dbg!(&result);
        assert!(matches!(result, Err(ValidationError::ZeroAmount)));
    }

    #[tokio::test]
    async fn post_validate_err_wrong_owner() {
        let mut fee_calculator = MockMinFeeCalculating::new();
        let mut bad_token_detector = MockBadTokenDetecting::new();
        let mut balance_fetcher = MockBalanceFetching::new();
        fee_calculator
            .expect_get_unsubsidized_min_fee()
            .returning(|_, _, _, _| Ok(Default::default()));
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
            true,
            Arc::new(fee_calculator),
            Arc::new(bad_token_detector),
            Arc::new(balance_fetcher),
        );
        let order = OrderCreation {
            valid_to: model::time::now_in_epoch_seconds() + 2,
            sell_token: H160::from_low_u64_be(1),
            buy_token: H160::from_low_u64_be(2),
            buy_amount: U256::from(1),
            sell_amount: U256::from(1),
            ..Default::default()
        };
        let result = validator
            .validate_and_construct_order(
                order,
                Some(Default::default()),
                &Default::default(),
                Default::default(),
            )
            .await;
        assert!(matches!(result, Err(ValidationError::WrongOwner(_))));
    }

    #[tokio::test]
    async fn post_validate_err_insufficient_fee() {
        let mut fee_calculator = MockMinFeeCalculating::new();
        let mut bad_token_detector = MockBadTokenDetecting::new();
        let mut balance_fetcher = MockBalanceFetching::new();
        fee_calculator
            .expect_get_unsubsidized_min_fee()
            .returning(|_, _, _, _| Err(GetUnsubsidizedMinFeeError::InsufficientFee));
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
            true,
            Arc::new(fee_calculator),
            Arc::new(bad_token_detector),
            Arc::new(balance_fetcher),
        );
        let order = OrderCreation {
            valid_to: model::time::now_in_epoch_seconds() + 2,
            sell_token: H160::from_low_u64_be(1),
            buy_token: H160::from_low_u64_be(2),
            buy_amount: U256::from(1),
            sell_amount: U256::from(1),
            ..Default::default()
        };
        let result = validator
            .validate_and_construct_order(order, None, &Default::default(), Default::default())
            .await;
        dbg!(&result);
        assert!(matches!(result, Err(ValidationError::InsufficientFee)));
    }

    #[tokio::test]
    async fn post_validate_err_unsupported_token() {
        let mut fee_calculator = MockMinFeeCalculating::new();
        let mut bad_token_detector = MockBadTokenDetecting::new();
        let mut balance_fetcher = MockBalanceFetching::new();
        fee_calculator
            .expect_get_unsubsidized_min_fee()
            .returning(|_, _, _, _| Ok(Default::default()));
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
            true,
            Arc::new(fee_calculator),
            Arc::new(bad_token_detector),
            Arc::new(balance_fetcher),
        );
        let order = OrderCreation {
            valid_to: model::time::now_in_epoch_seconds() + 2,
            sell_token: H160::from_low_u64_be(1),
            buy_token: H160::from_low_u64_be(2),
            buy_amount: U256::from(1),
            sell_amount: U256::from(1),
            ..Default::default()
        };
        let result = validator
            .validate_and_construct_order(order, None, &Default::default(), Default::default())
            .await;
        dbg!(&result);
        assert!(matches!(result, Err(ValidationError::UnsupportedToken(_))));
    }

    #[tokio::test]
    async fn post_validate_err_sell_amount_overflow() {
        let mut fee_calculator = MockMinFeeCalculating::new();
        let mut bad_token_detector = MockBadTokenDetecting::new();
        let mut balance_fetcher = MockBalanceFetching::new();
        fee_calculator
            .expect_get_unsubsidized_min_fee()
            .returning(|_, _, _, _| Ok(Default::default()));
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
            true,
            Arc::new(fee_calculator),
            Arc::new(bad_token_detector),
            Arc::new(balance_fetcher),
        );
        let order = OrderCreation {
            valid_to: model::time::now_in_epoch_seconds() + 2,
            sell_token: H160::from_low_u64_be(1),
            buy_token: H160::from_low_u64_be(2),
            buy_amount: U256::from(1),
            sell_amount: U256::MAX,
            fee_amount: U256::from(1),
            ..Default::default()
        };
        let result = validator
            .validate_and_construct_order(order, None, &Default::default(), Default::default())
            .await;
        dbg!(&result);
        assert!(matches!(result, Err(ValidationError::SellAmountOverflow)));
    }

    #[tokio::test]
    async fn post_validate_err_insufficient_balance() {
        let mut fee_calculator = MockMinFeeCalculating::new();
        let mut bad_token_detector = MockBadTokenDetecting::new();
        let mut balance_fetcher = MockBalanceFetching::new();
        fee_calculator
            .expect_get_unsubsidized_min_fee()
            .returning(|_, _, _, _| Ok(Default::default()));
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
            true,
            Arc::new(fee_calculator),
            Arc::new(bad_token_detector),
            Arc::new(balance_fetcher),
        );
        let order = OrderCreation {
            valid_to: model::time::now_in_epoch_seconds() + 2,
            sell_token: H160::from_low_u64_be(1),
            buy_token: H160::from_low_u64_be(2),
            buy_amount: U256::from(1),
            sell_amount: U256::from(1),
            ..Default::default()
        };
        let result = validator
            .validate_and_construct_order(order, None, &Default::default(), Default::default())
            .await;
        dbg!(&result);
        assert!(matches!(result, Err(ValidationError::InsufficientBalance)));
    }

    #[tokio::test]
    async fn allows_insufficient_allowance_and_balance_for_presign_orders() {
        macro_rules! assert_allows_failed_transfer {
            ($err:ident) => {
                let mut fee_calculator = MockMinFeeCalculating::new();
                let mut bad_token_detector = MockBadTokenDetecting::new();
                let mut balance_fetcher = MockBalanceFetching::new();
                fee_calculator
                    .expect_get_unsubsidized_min_fee()
                    .returning(|_, _, _, _| Ok(Default::default()));
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
                    true,
                    Arc::new(fee_calculator),
                    Arc::new(bad_token_detector),
                    Arc::new(balance_fetcher),
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
                                    .creation,
                                None,
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
                            order.with_presign(Default::default()).build().creation,
                            None,
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
}
