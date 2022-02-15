use crate::{
    api::order_validation::{OrderValidating, OrderValidator, ValidationError},
    database::orders::{InsertionError, OrderFilter, OrderStoring},
    event_updater::EventUpdating,
    solvable_orders::{SolvableOrders, SolvableOrdersCache},
};
use anyhow::{ensure, Context, Result};
use chrono::Utc;
use ethcontract::{H256, U256};
use model::{
    auction::Auction,
    order::{Order, OrderCancellation, OrderCreationPayload, OrderStatus, OrderUid},
    signature::SigningScheme,
    DomainSeparator,
};
use primitive_types::H160;
use shared::{
    bad_token::BadTokenDetecting, metrics::LivenessChecking,
    price_estimation::native::NativePriceEstimating,
};
use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
    time::Duration,
};

#[derive(Debug)]
pub enum AddOrderResult {
    Added(OrderUid),
    DuplicatedOrder,
    OrderValidation(ValidationError),
    UnsupportedSignature,
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
    bad_token_detector: Arc<dyn BadTokenDetecting>,
    native_price_estimator: Arc<dyn NativePriceEstimating>,
    enable_presign_orders: bool,
    solvable_orders: Arc<SolvableOrdersCache>,
    solvable_orders_max_update_age: Duration,
    order_validator: Arc<OrderValidator>,
    event_updater: Arc<dyn EventUpdating>,
}

impl Orderbook {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        domain_separator: DomainSeparator,
        settlement_contract: H160,
        database: Arc<dyn OrderStoring>,
        bad_token_detector: Arc<dyn BadTokenDetecting>,
        native_price_estimator: Arc<dyn NativePriceEstimating>,
        enable_presign_orders: bool,
        solvable_orders: Arc<SolvableOrdersCache>,
        solvable_orders_max_update_age: Duration,
        order_validator: Arc<OrderValidator>,
        event_updater: Arc<dyn EventUpdating>,
    ) -> Self {
        Self {
            domain_separator,
            settlement_contract,
            database,
            bad_token_detector,
            native_price_estimator,
            enable_presign_orders,
            solvable_orders,
            solvable_orders_max_update_age,
            order_validator,
            event_updater,
        }
    }

    pub async fn add_order(&self, payload: OrderCreationPayload) -> Result<AddOrderResult> {
        let order_creation = payload.order_creation;
        // Eventually we will support all Signature types and can remove this.
        if !matches!(
            (
                order_creation.signature.scheme(),
                self.enable_presign_orders
            ),
            (SigningScheme::Eip712 | SigningScheme::EthSign, _) | (SigningScheme::PreSign, true)
        ) {
            return Ok(AddOrderResult::UnsupportedSignature);
        }

        let (order, fee) = match self
            .order_validator
            .validate_and_construct_order(
                order_creation,
                payload.from,
                &self.domain_separator,
                self.settlement_contract,
            )
            .await
        {
            Ok(order) => order,
            Err(validation_err) => return Ok(AddOrderResult::OrderValidation(validation_err)),
        };

        match self.database.insert_order(&order, fee).await {
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
                .orders
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

    pub async fn get_orders_for_tx(&self, hash: &H256) -> Result<Vec<Order>> {
        let mut orders = self.database.orders_for_tx(hash).await?;
        set_available_balances(orders.as_mut_slice(), &self.solvable_orders);
        Ok(orders)
    }

    pub async fn get_solvable_orders(&self) -> Result<SolvableOrders> {
        let solvable_orders = self.solvable_orders.cached_solvable_orders();
        ensure!(
            solvable_orders.update_time.elapsed() <= self.solvable_orders_max_update_age,
            "solvable orders are out of date"
        );
        Ok(solvable_orders)
    }

    pub async fn get_auction(&self) -> Result<Auction> {
        let last_handled_block = self.event_updater.last_handled_block().await;
        let solvable_orders = self.get_solvable_orders().await?;

        let block = last_handled_block.unwrap_or(solvable_orders.latest_settlement_block);
        let (orders, prices) =
            get_orders_with_native_prices(solvable_orders.orders, &*self.native_price_estimator)
                .await;

        Ok(Auction {
            block,
            orders,
            prices,
        })
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
            .await
            .context("get_user_orders error")?;
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

async fn get_orders_with_native_prices(
    mut orders: Vec<Order>,
    native_price_estimator: &dyn NativePriceEstimating,
) -> (Vec<Order>, BTreeMap<H160, U256>) {
    let traded_tokens = orders
        .iter()
        .flat_map(|order| {
            [
                order.order_creation.sell_token,
                order.order_creation.buy_token,
            ]
        })
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let prices = native_price_estimator
        .estimate_native_prices(&traded_tokens)
        .await
        .into_iter()
        .zip(traded_tokens)
        .filter_map(|(price, token)| match price {
            Ok(price) => Some((token, to_normalized_price(price)?)),
            Err(err) => {
                tracing::warn!(?token, ?err, "error estimating native token price");
                None
            }
        })
        .collect::<BTreeMap<_, _>>();

    orders.retain(|order| {
        prices.contains_key(&order.order_creation.sell_token)
            && prices.contains_key(&order.order_creation.buy_token)
    });

    (orders, prices)
}

fn to_normalized_price(price: f64) -> Option<U256> {
    let uint_max = 2.0_f64.powi(256);

    // NOTE: The `NativePriceEstimating` component returns prices denominated
    // in the token that it is estimating and not in ETH. This means that we
    // need to invert the price in order for it to be correct.
    let price_in_eth = 1e18 / price;
    if price_in_eth.is_normal() && price_in_eth >= 1. && price_in_eth < uint_max {
        Some(U256::from_f64_lossy(price_in_eth))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethcontract::H160;
    use futures::FutureExt;
    use maplit::{btreemap, hashset};
    use model::order::OrderBuilder;
    use shared::{
        bad_token::list_based::ListBasedDetector,
        price_estimation::{native::MockNativePriceEstimating, PriceEstimationError},
    };

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
    fn computes_u256_prices_normalized_to_1e18() {
        assert_eq!(
            to_normalized_price(0.5).unwrap(), // means 0.5 token buys 1 ETH
            U256::from(2_000_000_000_000_000_000_u128)  // Means that the price of token is 2 ETH
        );
    }

    #[test]
    fn normalize_prices_fail_when_outside_valid_input_range() {
        assert!(to_normalized_price(0.).is_none());
        assert!(to_normalized_price(-1.).is_none());
        assert!(to_normalized_price(f64::INFINITY).is_none());

        let uint_max = 2.0_f64.powi(256);

        let min_price = 1e18 / uint_max;
        assert!(to_normalized_price(min_price).is_none());
        assert!(to_normalized_price(min_price * (1. + f64::EPSILON)).is_some());

        let max_price = 1e18;
        assert!(to_normalized_price(max_price).is_some());
        assert!(to_normalized_price(max_price * (1. + f64::EPSILON)).is_none());
    }

    #[tokio::test]
    async fn filters_tokens_without_native_prices() {
        let token1 = H160([1; 20]);
        let token2 = H160([2; 20]);
        let token3 = H160([3; 20]);
        let token4 = H160([4; 20]);

        let orders = vec![
            OrderBuilder::default()
                .with_sell_token(token1)
                .with_buy_token(token2)
                .build(),
            OrderBuilder::default()
                .with_sell_token(token2)
                .with_buy_token(token3)
                .build(),
            OrderBuilder::default()
                .with_sell_token(token1)
                .with_buy_token(token3)
                .build(),
            OrderBuilder::default()
                .with_sell_token(token2)
                .with_buy_token(token4)
                .build(),
        ];
        let prices = btreemap! {
            token1 => 0.5,
            token3 => 4.0,
            token4 => 0.0, // invalid price!
        };

        let mut native_price_estimator = MockNativePriceEstimating::new();
        native_price_estimator
            .expect_estimate_native_prices()
            // deal with undeterministic ordering of `HashSet`.
            .withf(move |tokens| {
                tokens.iter().cloned().collect::<HashSet<_>>()
                    == hashset!(token1, token2, token3, token4)
            })
            .returning({
                let prices = prices.clone();
                move |tokens| {
                    tokens
                        .iter()
                        .map(|token| {
                            prices
                                .get(token)
                                .copied()
                                .ok_or(PriceEstimationError::NoLiquidity)
                        })
                        .collect()
                }
            });

        let (filtered_orders, prices) =
            get_orders_with_native_prices(orders.clone(), &native_price_estimator).await;

        assert_eq!(filtered_orders, [orders[2].clone()]);
        assert_eq!(
            prices,
            btreemap! {
                token1 => U256::from(2_000_000_000_000_000_000_u128),
                token3 => U256::from(250_000_000_000_000_000_u128),
            }
        );
    }
}
