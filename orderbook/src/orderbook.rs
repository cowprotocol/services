use crate::{
    account_balances::BalanceFetching,
    database::orders::{InsertionError, OrderFilter, OrderStoring},
    fee::{EthAwareMinFeeCalculator, MinFeeCalculating},
    solvable_orders::SolvableOrdersCache,
};
use anyhow::{ensure, Result};
use chrono::Utc;
use contracts::WETH9;
use model::{
    order::{
        BuyTokenDestination, OrderCancellation, OrderCreation, OrderCreationPayload,
        SellTokenSource,
    },
    signature::SigningScheme,
};
use model::{
    order::{Order, OrderStatus, OrderUid, BUY_ETH_ADDRESS},
    DomainSeparator,
};
use primitive_types::{H160, U256};
use shared::{bad_token::BadTokenDetecting, metrics::LivenessChecking, web3_traits::CodeFetching};
use std::{collections::HashSet, sync::Arc, time::Duration};

#[derive(Debug, Eq, PartialEq)]
pub enum AddOrderResult {
    Added(OrderUid),
    WrongOwner(H160),
    DuplicatedOrder,
    InvalidSignature,
    UnsupportedSignature,
    Forbidden,
    MissingOrderData,
    InsufficientValidTo,
    InsufficientFunds,
    InsufficientFee,
    UnsupportedToken(H160),
    TransferEthToContract,
    SameBuyAndSellToken,
    UnsupportedBuyTokenDestination(BuyTokenDestination),
    UnsupportedSellTokenSource(SellTokenSource),
}

#[derive(Debug)]
pub enum OrderCancellationResult {
    Cancelled,
    InvalidSignature,
    WrongOwner,
    OrderNotFound,
    AlreadyCancelled,
    OrderFullyExecuted,
    OrderExpired,
    OnChainOrder,
}

pub struct Orderbook {
    domain_separator: DomainSeparator,
    settlement_contract: H160,
    database: Arc<dyn OrderStoring>,
    balance_fetcher: Arc<dyn BalanceFetching>,
    fee_validator: Arc<EthAwareMinFeeCalculator>,
    min_order_validity_period: Duration,
    bad_token_detector: Arc<dyn BadTokenDetecting>,
    code_fetcher: Box<dyn CodeFetching>,
    native_token: WETH9,
    banned_users: Vec<H160>,
    enable_presign_orders: bool,
    solvable_orders: Arc<SolvableOrdersCache>,
    solvable_orders_max_update_age: Duration,
}

impl Orderbook {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        domain_separator: DomainSeparator,
        settlement_contract: H160,
        database: Arc<dyn OrderStoring>,
        balance_fetcher: Arc<dyn BalanceFetching>,
        fee_validator: Arc<EthAwareMinFeeCalculator>,
        min_order_validity_period: Duration,
        bad_token_detector: Arc<dyn BadTokenDetecting>,
        code_fetcher: Box<dyn CodeFetching>,
        native_token: WETH9,
        banned_users: Vec<H160>,
        enable_presign_orders: bool,
        solvable_orders: Arc<SolvableOrdersCache>,
        solvable_orders_max_update_age: Duration,
    ) -> Self {
        Self {
            domain_separator,
            settlement_contract,
            database,
            balance_fetcher,
            fee_validator,
            min_order_validity_period,
            bad_token_detector,
            code_fetcher,
            native_token,
            banned_users,
            enable_presign_orders,
            solvable_orders,
            solvable_orders_max_update_age,
        }
    }

    pub async fn add_order(&self, payload: OrderCreationPayload) -> Result<AddOrderResult> {
        let order = payload.order_creation;

        // Temporary - reject new order types until last stage of balancer integration
        if order.buy_token_balance != BuyTokenDestination::Erc20 {
            return Ok(AddOrderResult::UnsupportedBuyTokenDestination(
                order.buy_token_balance,
            ));
        }
        if !matches!(
            order.sell_token_balance,
            SellTokenSource::Erc20 | SellTokenSource::External
        ) {
            return Ok(AddOrderResult::UnsupportedSellTokenSource(
                order.sell_token_balance,
            ));
        }
        if !matches!(
            (order.signature.scheme(), self.enable_presign_orders),
            (SigningScheme::Eip712 | SigningScheme::EthSign, _) | (SigningScheme::PreSign, true)
        ) {
            return Ok(AddOrderResult::UnsupportedSignature);
        }

        if has_same_buy_and_sell_token(&order, &self.native_token) {
            return Ok(AddOrderResult::SameBuyAndSellToken);
        }
        if order.valid_to
            < shared::time::now_in_epoch_seconds() + self.min_order_validity_period.as_secs() as u32
        {
            return Ok(AddOrderResult::InsufficientValidTo);
        }
        if !self
            .fee_validator
            .is_valid_fee(order.sell_token, order.fee_amount)
            .await
        {
            return Ok(AddOrderResult::InsufficientFee);
        }
        let order = match Order::from_order_creation(
            order,
            &self.domain_separator,
            self.settlement_contract,
        ) {
            Some(order) => order,
            None => return Ok(AddOrderResult::InvalidSignature),
        };

        let owner = order.order_meta_data.owner;
        if self.banned_users.contains(&owner) {
            return Ok(AddOrderResult::Forbidden);
        }

        if matches!(payload.from, Some(from) if from != owner) {
            return Ok(AddOrderResult::WrongOwner(owner));
        }

        for &token in &[
            order.order_creation.sell_token,
            order.order_creation.buy_token,
        ] {
            if !self.bad_token_detector.detect(token).await?.is_good() {
                return Ok(AddOrderResult::UnsupportedToken(token));
            }
        }

        let min_balance = match minimum_balance(&order) {
            Some(amount) => amount,
            None => return Ok(AddOrderResult::InsufficientFunds),
        };
        if !self
            .balance_fetcher
            .can_transfer(
                order.order_creation.sell_token,
                owner,
                min_balance,
                order.order_creation.sell_token_balance,
            )
            .await
            .unwrap_or(false)
        {
            return Ok(AddOrderResult::InsufficientFunds);
        }

        if order.order_creation.buy_token == BUY_ETH_ADDRESS {
            let code_size = self.code_fetcher.code_size(order.actual_receiver()).await?;
            if code_size != 0 {
                return Ok(AddOrderResult::TransferEthToContract);
            }
        }

        match self.database.insert_order(&order).await {
            Err(InsertionError::DuplicatedRecord) => return Ok(AddOrderResult::DuplicatedOrder),
            Err(InsertionError::DbError(err)) => return Err(err.into()),
            _ => (),
        }
        self.solvable_orders.request_update();
        Ok(AddOrderResult::Added(order.order_meta_data.uid))
    }

    pub async fn cancel_order(
        &self,
        cancellation: OrderCancellation,
    ) -> Result<OrderCancellationResult> {
        // TODO - Would like to use get_order_by_uid, but not implemented on self
        let orders = self
            .get_orders(&OrderFilter {
                uid: Some(cancellation.order_uid),
                ..Default::default()
            })
            .await?;
        // Could be that order doesn't exist and is not fetched.
        let order = match orders.first() {
            Some(order) => order,
            None => return Ok(OrderCancellationResult::OrderNotFound),
        };

        match order.order_meta_data.status {
            OrderStatus::PresignaturePending => return Ok(OrderCancellationResult::OnChainOrder),
            OrderStatus::Open if !order.order_creation.signature.scheme().is_ecdsa_scheme() => {
                return Ok(OrderCancellationResult::OnChainOrder);
            }
            OrderStatus::Fulfilled => return Ok(OrderCancellationResult::OrderFullyExecuted),
            OrderStatus::Cancelled => return Ok(OrderCancellationResult::AlreadyCancelled),
            OrderStatus::Expired => return Ok(OrderCancellationResult::OrderExpired),
            _ => {}
        }

        match cancellation.validate(&self.domain_separator) {
            Some(signer) if signer == order.order_meta_data.owner => {}
            Some(_) => return Ok(OrderCancellationResult::WrongOwner),
            None => return Ok(OrderCancellationResult::InvalidSignature),
        };

        // order is already known to exist in DB at this point, and signer is
        // known to be correct!
        self.database
            .cancel_order(&order.order_meta_data.uid, Utc::now())
            .await?;
        Ok(OrderCancellationResult::Cancelled)
    }

    pub async fn get_orders(&self, filter: &OrderFilter) -> Result<Vec<Order>> {
        let mut orders = self.database.orders(filter).await?;
        // This filter is deprecated so filtering solvable orders is a bit awkward but we'll support
        // for a little bit.
        if filter.exclude_insufficient_balance {
            use crate::account_balances::Query;
            let solvable_orders = self
                .solvable_orders
                .cached_solvable_orders()
                .0
                .iter()
                .map(Query::from_order)
                .collect::<HashSet<_>>();
            orders.retain(|order| solvable_orders.contains(&Query::from_order(order)));
        }
        set_available_balances(orders.as_mut_slice(), &self.solvable_orders);
        if filter.exclude_unsupported_tokens {
            orders = filter_unsupported_tokens(orders, self.bad_token_detector.as_ref()).await?;
        }
        Ok(orders)
    }

    pub async fn get_order(&self, uid: &OrderUid) -> Result<Option<Order>> {
        let mut order = match self.database.single_order(uid).await? {
            Some(order) => order,
            None => return Ok(None),
        };
        set_available_balances(std::slice::from_mut(&mut order), &self.solvable_orders);
        Ok(Some(order))
    }

    pub async fn get_solvable_orders(&self) -> Result<Vec<Order>> {
        let (orders, timestamp) = self.solvable_orders.cached_solvable_orders();
        ensure!(
            timestamp.elapsed() <= self.solvable_orders_max_update_age,
            "solvable orders are out of date"
        );
        Ok(orders)
    }

    pub async fn get_user_orders(
        &self,
        owner: &H160,
        offset: u64,
        limit: u64,
    ) -> Result<Vec<Order>> {
        let mut orders = self
            .database
            .user_orders(owner, offset, Some(limit))
            .await?;
        set_available_balances(orders.as_mut_slice(), &self.solvable_orders);
        Ok(orders)
    }
}

#[async_trait::async_trait]
impl LivenessChecking for Orderbook {
    async fn is_alive(&self) -> bool {
        self.get_solvable_orders().await.is_ok()
    }
}

pub async fn filter_unsupported_tokens(
    mut orders: Vec<Order>,
    bad_token: &dyn BadTokenDetecting,
) -> Result<Vec<Order>> {
    // Can't use normal `retain` or `filter` because the bad token detection is async. So either
    // this manual iteration or conversion to stream.
    let mut index = 0;
    'outer: while index < orders.len() {
        for token in orders[index].order_creation.token_pair().unwrap() {
            if !bad_token.detect(token).await?.is_good() {
                orders.swap_remove(index);
                continue 'outer;
            }
        }
        index += 1;
    }
    Ok(orders)
}

fn set_available_balances(orders: &mut [Order], cache: &SolvableOrdersCache) {
    for order in orders.iter_mut() {
        order.order_meta_data.available_balance =
            cache.cached_balance(&crate::account_balances::Query::from_order(order));
    }
}

// Mininum balance user must have in sell token for order to be accepted. None if no balance is
// sufficient.
fn minimum_balance(order: &Order) -> Option<U256> {
    if order.order_creation.partially_fillable {
        Some(U256::from(1))
    } else {
        order
            .order_creation
            .sell_amount
            .checked_add(order.order_creation.fee_amount)
    }
}

/// Returns true if the orders have same buy and sell tokens.
///
/// This also checks for orders selling wrapped native token for native token.
fn has_same_buy_and_sell_token(order: &OrderCreation, native_token: &WETH9) -> bool {
    order.sell_token == order.buy_token
        || (order.sell_token == native_token.address() && order.buy_token == BUY_ETH_ADDRESS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethcontract::H160;
    use futures::FutureExt;
    use model::order::{OrderBuilder, OrderCreation};
    use shared::{bad_token::list_based::ListBasedDetector, dummy_contract};

    #[test]
    fn filter_unsupported_tokens_() {
        let token0 = H160::from_low_u64_le(0);
        let token1 = H160::from_low_u64_le(1);
        let token2 = H160::from_low_u64_le(2);
        let bad_token = ListBasedDetector::deny_list(vec![token0]);
        let orders = vec![
            OrderBuilder::default()
                .with_sell_token(token0)
                .with_buy_token(token1)
                .build(),
            OrderBuilder::default()
                .with_sell_token(token1)
                .with_buy_token(token2)
                .build(),
            OrderBuilder::default()
                .with_sell_token(token0)
                .with_buy_token(token2)
                .build(),
        ];
        let result = filter_unsupported_tokens(orders.clone(), &bad_token)
            .now_or_never()
            .unwrap()
            .unwrap();
        assert_eq!(result, &orders[1..2]);
    }

    #[test]
    fn detects_orders_with_same_buy_and_sell_token() {
        let native_token = dummy_contract!(WETH9, [0xef; 20]);
        assert!(has_same_buy_and_sell_token(
            &OrderCreation {
                sell_token: H160([0x01; 20]),
                buy_token: H160([0x01; 20]),
                ..Default::default()
            },
            &native_token,
        ));
        assert!(has_same_buy_and_sell_token(
            &OrderCreation {
                sell_token: native_token.address(),
                buy_token: BUY_ETH_ADDRESS,
                ..Default::default()
            },
            &native_token,
        ));

        assert!(!has_same_buy_and_sell_token(
            &OrderCreation {
                sell_token: H160([0x01; 20]),
                buy_token: H160([0x02; 20]),
                ..Default::default()
            },
            &native_token,
        ));
        // Sell token set to 0xeee...eee has no special meaning, so it isn't
        // considered buying and selling the same token.
        assert!(!has_same_buy_and_sell_token(
            &OrderCreation {
                sell_token: BUY_ETH_ADDRESS,
                buy_token: native_token.address(),
                ..Default::default()
            },
            &native_token,
        ));
    }
}
