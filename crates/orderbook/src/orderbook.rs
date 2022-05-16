use crate::{
    api::order_validation::{OrderValidating, OrderValidator, ValidationError},
    database::orders::{InsertionError, OrderFilter, OrderStoring},
    fee::FeeParameters,
    solvable_orders::{SolvableOrders, SolvableOrdersCache},
};
use anyhow::{ensure, Context, Result};
use chrono::Utc;
use ethcontract::H256;
use gas_estimation::EstimatedGasPrice;
use model::{
    auction::Auction,
    order::{Order, OrderCancellation, OrderCreationPayload, OrderStatus, OrderUid},
    signature::SigningScheme,
    DomainSeparator,
};
use primitive_types::H160;
use shared::{bad_token::BadTokenDetecting, metrics, metrics::LivenessChecking};
use std::{collections::HashSet, sync::Arc, time::Duration};
use thiserror::Error;

#[derive(prometheus_metric_storage::MetricStorage, Clone, Debug)]
#[metric(subsystem = "orderbook")]
struct Metrics {
    /// Number of user (non-liquidity) orders created.
    user_orders_created: prometheus::Counter,
}

#[derive(Debug, Error)]
pub enum AddOrderError {
    #[error("duplicated order")]
    DuplicatedOrder,
    #[error("{0:?}")]
    OrderValidation(ValidationError),
    #[error("unsupported signature kind")]
    UnsupportedSignature,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl From<InsertionError> for AddOrderError {
    fn from(err: InsertionError) -> Self {
        match err {
            InsertionError::DuplicatedRecord => AddOrderError::DuplicatedOrder,
            InsertionError::DbError(err) => AddOrderError::Database(err),
        }
    }
}

// This requires a manual implementation because the `#[from]` attribute from
// `thiserror` implies `#[source]` which requires `ValidationError: Error`,
// which it currently does not!
impl From<ValidationError> for AddOrderError {
    fn from(err: ValidationError) -> Self {
        Self::OrderValidation(err)
    }
}

#[derive(Debug, Error)]
pub enum OrderCancellationError {
    #[error("invalid signature")]
    InvalidSignature,
    #[error("signer does not match order owner")]
    WrongOwner,
    #[error("order not found")]
    OrderNotFound,
    #[error("order already cancelled")]
    AlreadyCancelled,
    #[error("order fully executed")]
    OrderFullyExecuted,
    #[error("order expired")]
    OrderExpired,
    #[error("on-chain orders cannot be cancelled with off-chain signature")]
    OnChainOrder,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub struct Orderbook {
    domain_separator: DomainSeparator,
    settlement_contract: H160,
    database: Arc<dyn OrderStoring>,
    bad_token_detector: Arc<dyn BadTokenDetecting>,
    enable_presign_orders: bool,
    solvable_orders: Arc<SolvableOrdersCache>,
    solvable_orders_max_update_age: Duration,
    order_validator: Arc<OrderValidator>,
}

impl Orderbook {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        domain_separator: DomainSeparator,
        settlement_contract: H160,
        database: Arc<dyn OrderStoring>,
        bad_token_detector: Arc<dyn BadTokenDetecting>,
        enable_presign_orders: bool,
        solvable_orders: Arc<SolvableOrdersCache>,
        solvable_orders_max_update_age: Duration,
        order_validator: Arc<OrderValidator>,
    ) -> Self {
        Self {
            domain_separator,
            settlement_contract,
            database,
            bad_token_detector,
            enable_presign_orders,
            solvable_orders,
            solvable_orders_max_update_age,
            order_validator,
        }
    }

    pub async fn add_order(
        &self,
        payload: OrderCreationPayload,
    ) -> Result<OrderUid, AddOrderError> {
        let order_creation = payload.order_creation;
        // Eventually we will support all Signature types and can remove this.
        if !matches!(
            (
                order_creation.signature.scheme(),
                self.enable_presign_orders
            ),
            (SigningScheme::Eip712 | SigningScheme::EthSign, _) | (SigningScheme::PreSign, true)
        ) {
            return Err(AddOrderError::UnsupportedSignature);
        }

        let (order, fee) = self
            .order_validator
            .validate_and_construct_order(
                order_creation,
                payload.from,
                &self.domain_separator,
                self.settlement_contract,
            )
            .await?;

        self.database.insert_order(&order, fee).await?;

        if !order.metadata.is_liquidity_order {
            Metrics::instance(metrics::get_metric_storage_registry())
                .expect("unexpected error getting metrics instance")
                .user_orders_created
                .inc();
        }

        self.solvable_orders.request_update();

        Ok(order.metadata.uid)
    }

    pub async fn cancel_order(
        &self,
        cancellation: OrderCancellation,
    ) -> Result<(), OrderCancellationError> {
        // TODO - Would like to use get_order_by_uid, but not implemented on self
        let orders = self
            .get_orders(&OrderFilter {
                uid: Some(cancellation.order_uid),
                ..Default::default()
            })
            .await?;
        // Could be that order doesn't exist and is not fetched.
        let order = orders
            .first()
            .ok_or(OrderCancellationError::OrderNotFound)?;

        match order.metadata.status {
            OrderStatus::PresignaturePending => return Err(OrderCancellationError::OnChainOrder),
            OrderStatus::Open if !order.creation.signature.scheme().is_ecdsa_scheme() => {
                return Err(OrderCancellationError::OnChainOrder);
            }
            OrderStatus::Fulfilled => return Err(OrderCancellationError::OrderFullyExecuted),
            OrderStatus::Cancelled => return Err(OrderCancellationError::AlreadyCancelled),
            OrderStatus::Expired => return Err(OrderCancellationError::OrderExpired),
            _ => {}
        }

        let signer = cancellation
            .validate(&self.domain_separator)
            .ok_or(OrderCancellationError::InvalidSignature)?;
        if signer != order.metadata.owner {
            return Err(OrderCancellationError::WrongOwner);
        };

        // order is already known to exist in DB at this point, and signer is
        // known to be correct!
        self.database
            .cancel_order(&order.metadata.uid, Utc::now())
            .await?;
        Ok(())
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

    pub fn get_solvable_orders(&self) -> Result<SolvableOrders> {
        let solvable_orders = self.solvable_orders.cached_solvable_orders();
        ensure!(
            solvable_orders.update_time.elapsed() <= self.solvable_orders_max_update_age,
            "solvable orders are out of date"
        );
        Ok(solvable_orders)
    }

    pub fn get_auction(&self) -> Result<Auction> {
        let (auction, update_time) = self.solvable_orders.cached_auction();
        ensure!(
            update_time.elapsed() <= self.solvable_orders_max_update_age,
            "auction is out of date"
        );
        Ok(auction)
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
        self.get_solvable_orders().is_ok()
    }
}

pub async fn filter_low_fee_payments(
    current_gas_price: EstimatedGasPrice,
    order_storing: Arc<dyn OrderStoring>,
    orders: Vec<Order>,
) -> Result<Vec<Order>> {
    let orders_with_fee_parameters = orders.iter().map(|order| async {
        let fees = order_storing
            .fee_of_order(&order.metadata.uid)
            .await
            .unwrap();
        (order.clone(), fees)
    });
    let orders_with_fee_parameters: Vec<(Order, FeeParameters)> =
        futures::future::join_all(orders_with_fee_parameters).await;
    let acceptable_gas_price_increase = 1.5f64;
    // Todo: The following filtering could have also been already done on a database level, in order to increase the performance
    let orders = orders_with_fee_parameters
        .iter()
        .filter(|(order, fee_estimate)| {
            let gas_price_condition = if let Some(eip1559_gas_price) = current_gas_price.eip1559 {
                fee_estimate.gas_price * acceptable_gas_price_increase
                    > eip1559_gas_price.base_fee_per_gas
            } else {
                fee_estimate.gas_price * acceptable_gas_price_increase > current_gas_price.legacy
            };
            gas_price_condition
                || (order.metadata.creation_date + chrono::Duration::minutes(30)
                    > chrono::offset::Utc::now())
        })
        .map(|(order, _)| order.clone())
        .collect();
    Ok(orders)
}

pub async fn filter_unsupported_tokens(
    mut orders: Vec<Order>,
    bad_token: &dyn BadTokenDetecting,
) -> Result<Vec<Order>> {
    // Can't use normal `retain` or `filter` because the bad token detection is async. So either
    // this manual iteration or conversion to stream.
    let mut index = 0;
    'outer: while index < orders.len() {
        for token in orders[index].creation.token_pair().unwrap() {
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
        order.metadata.available_balance =
            cache.cached_balance(&crate::account_balances::Query::from_order(order));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::orders::MockOrderStoring;
    use ethcontract::H160;
    use futures::FutureExt;
    use gas_estimation::GasPrice1559;
    use mockall::predicate;
    use model::order::OrderMetadata;
    use model::order::{OrderBuilder, OrderCreation, OrderKind};
    use shared::bad_token::list_based::ListBasedDetector;

    #[test]
    fn test_filter_low_fee_payments() {
        let current_gas_price = EstimatedGasPrice {
            legacy: 50.0f64,
            eip1559: Some(GasPrice1559 {
                base_fee_per_gas: 50.0f64,
                max_fee_per_gas: 51.0f64,
                max_priority_fee_per_gas: 1.0f64,
            }),
        };
        let sufficient_fee_order = Order {
            metadata: OrderMetadata {
                creation_date: chrono::offset::Utc::now() - chrono::Duration::minutes(1),
                ..Default::default()
            },
            creation: OrderCreation {
                kind: OrderKind::Sell,
                sell_amount: 1i32.into(),
                buy_amount: 1.into(),
                ..Default::default()
            },
        };

        let mut order_storing = MockOrderStoring::new();
        let uid = sufficient_fee_order.metadata.uid;
        order_storing
            .expect_fee_of_order()
            .with(predicate::eq(uid))
            .times(1)
            .returning(|_| {
                Ok(FeeParameters {
                    gas_amount: 50.0f64,
                    gas_price: 50.0f64,
                    sell_token_price: 50.0f64,
                })
            });

        let in_sufficient_fee_order_and_old = Order {
            metadata: OrderMetadata {
                creation_date: chrono::offset::Utc::now() - chrono::Duration::minutes(31),
                ..Default::default()
            },
            creation: OrderCreation {
                kind: OrderKind::Sell,
                sell_amount: 2.into(),
                buy_amount: 1.into(),
                ..Default::default()
            },
        };
        let uid = in_sufficient_fee_order_and_old.metadata.uid;
        order_storing
            .expect_fee_of_order()
            .with(predicate::eq(uid))
            .times(1)
            .returning(|_| {
                Ok(FeeParameters {
                    gas_amount: 50.0f64,
                    gas_price: 10.0f64,
                    sell_token_price: 50.0f64,
                })
            });

        let in_sufficient_fee_order_but_recent = Order {
            metadata: OrderMetadata {
                creation_date: chrono::offset::Utc::now() - chrono::Duration::minutes(1),
                ..Default::default()
            },
            creation: OrderCreation {
                kind: OrderKind::Sell,
                sell_amount: 3.into(),
                buy_amount: 1.into(),
                ..Default::default()
            },
        };
        let uid = in_sufficient_fee_order_but_recent.metadata.uid;
        order_storing
            .expect_fee_of_order()
            .with(predicate::eq(uid))
            .times(1)
            .returning(move |_| {
                Ok(FeeParameters {
                    gas_amount: 50.0f64,
                    gas_price: 10.0f64,
                    sell_token_price: 50.0f64,
                })
            });

        let result = filter_low_fee_payments(
            current_gas_price,
            Arc::new(order_storing),
            vec![
                sufficient_fee_order.clone(),
                in_sufficient_fee_order_but_recent.clone(),
                in_sufficient_fee_order_and_old,
            ],
        )
        .now_or_never()
        .unwrap()
        .unwrap();
        assert_eq!(
            result,
            vec![sufficient_fee_order, in_sufficient_fee_order_but_recent]
        );
    }

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
}
