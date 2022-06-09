use crate::{
    api::order_validation::{OrderValidating, ValidationError},
    database::orders::{InsertionError, OrderFilter, OrderStoring},
    solvable_orders::{SolvableOrders, SolvableOrdersCache},
};
use anyhow::{ensure, Context, Result};
use chrono::Utc;
use ethcontract::H256;
use model::{
    auction::Auction,
    order::{Order, OrderCancellation, OrderCreation, OrderStatus, OrderUid},
    DomainSeparator,
};
use primitive_types::H160;
use shared::{bad_token::BadTokenDetecting, metrics, metrics::LivenessChecking};
use std::{collections::HashSet, sync::Arc, time::Duration};
use thiserror::Error;

#[derive(prometheus_metric_storage::MetricStorage, Clone, Debug)]
#[metric(subsystem = "orderbook")]
struct Metrics {
    /// Counter for measuring order statistics.
    #[metric(labels("kind", "operation"))]
    orders: prometheus::CounterVec,
}

enum OrderOperation {
    Created,
    Cancelled,
}

impl Metrics {
    fn on_order_operation(order: &Order, operation: OrderOperation) {
        let metrics = Self::instance(metrics::get_metric_storage_registry())
            .expect("unexpected error getting metrics instance");

        let kind = match order.metadata.is_liquidity_order {
            true => "liquidity",
            false => "user",
        };
        let op = match operation {
            OrderOperation::Created => "created",
            OrderOperation::Cancelled => "cancelled",
        };
        metrics.orders.with_label_values(&[kind, op]).inc();
    }
}

#[derive(Debug, Error)]
pub enum AddOrderError {
    #[error("duplicated order")]
    DuplicatedOrder,
    #[error("{0:?}")]
    OrderValidation(ValidationError),
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

#[derive(Debug, Error)]
pub enum ReplaceOrderError {
    #[error("unable to cancel existing order: {0}")]
    Cancellation(#[from] OrderCancellationError),
    #[error("unable to add new order: {0}")]
    Add(#[from] AddOrderError),
    #[error("the new order is not a valid replacement for the old one")]
    InvalidReplacement,
}

impl From<ValidationError> for ReplaceOrderError {
    fn from(err: ValidationError) -> Self {
        Self::Add(err.into())
    }
}

impl From<InsertionError> for ReplaceOrderError {
    fn from(err: InsertionError) -> Self {
        Self::Add(err.into())
    }
}

pub struct Orderbook {
    domain_separator: DomainSeparator,
    settlement_contract: H160,
    database: Arc<dyn OrderStoring>,
    bad_token_detector: Arc<dyn BadTokenDetecting>,
    solvable_orders: Arc<SolvableOrdersCache>,
    solvable_orders_max_update_age: Duration,
    order_validator: Arc<dyn OrderValidating>,
}

impl Orderbook {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        domain_separator: DomainSeparator,
        settlement_contract: H160,
        database: Arc<dyn OrderStoring>,
        bad_token_detector: Arc<dyn BadTokenDetecting>,
        solvable_orders: Arc<SolvableOrdersCache>,
        solvable_orders_max_update_age: Duration,
        order_validator: Arc<dyn OrderValidating>,
    ) -> Self {
        Self {
            domain_separator,
            settlement_contract,
            database,
            bad_token_detector,
            solvable_orders,
            solvable_orders_max_update_age,
            order_validator,
        }
    }

    pub async fn add_order(&self, payload: OrderCreation) -> Result<OrderUid, AddOrderError> {
        let (order, fee) = self
            .order_validator
            .validate_and_construct_order(payload, &self.domain_separator, self.settlement_contract)
            .await?;

        self.database.insert_order(&order, fee).await?;
        Metrics::on_order_operation(&order, OrderOperation::Created);

        self.solvable_orders.request_update();

        Ok(order.metadata.uid)
    }

    /// Finds an order for cancellation.
    ///
    /// Returns an error if the order cannot be found or cannot be cancelled.
    async fn find_order_for_cancellation(
        &self,
        order_uid: &OrderUid,
    ) -> Result<Order, OrderCancellationError> {
        let order = self
            .database
            .single_order(order_uid)
            .await?
            .ok_or(OrderCancellationError::OrderNotFound)?;

        match order.metadata.status {
            OrderStatus::PresignaturePending => return Err(OrderCancellationError::OnChainOrder),
            OrderStatus::Open if !order.signature.scheme().is_ecdsa_scheme() => {
                return Err(OrderCancellationError::OnChainOrder);
            }
            OrderStatus::Fulfilled => return Err(OrderCancellationError::OrderFullyExecuted),
            OrderStatus::Cancelled => return Err(OrderCancellationError::AlreadyCancelled),
            OrderStatus::Expired => return Err(OrderCancellationError::OrderExpired),
            _ => {}
        }

        Ok(order)
    }

    pub async fn cancel_order(
        &self,
        cancellation: OrderCancellation,
    ) -> Result<(), OrderCancellationError> {
        let order = self
            .find_order_for_cancellation(&cancellation.order_uid)
            .await?;

        // Verify the cancellation signer is the same as the order signer.
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
        Metrics::on_order_operation(&order, OrderOperation::Created);

        self.solvable_orders.request_update();

        Ok(())
    }

    pub async fn replace_order(
        &self,
        old_order: OrderUid,
        new_order: OrderCreation,
    ) -> Result<OrderUid, ReplaceOrderError> {
        // Replacement order signatures need to be validated meaning we cannot
        // accept `PreSign` orders, otherwise anyone can cancel a user order by
        // submitting a `PreSign` order on someone's behalf.
        new_order
            .signature
            .to_protocol_signature()
            .scheme()
            .try_to_ecdsa_scheme()
            .ok_or(ReplaceOrderError::InvalidReplacement)?;

        let old_order = self.find_order_for_cancellation(&old_order).await?;
        let (new_order, new_fee) = self
            .order_validator
            .validate_and_construct_order(
                new_order,
                &self.domain_separator,
                self.settlement_contract,
            )
            .await?;

        // Verify that the new order is a valid replacement order by checking
        // that the `app_data` encodes an order cancellation and that both the
        // old and new orders have the same signer.
        let cancellation = OrderCancellation {
            order_uid: old_order.metadata.uid,
            ..Default::default()
        };
        if new_order.data.app_data != cancellation.hash_struct()
            || new_order.metadata.owner != old_order.metadata.owner
        {
            return Err(ReplaceOrderError::InvalidReplacement);
        }

        self.database
            .replace_order(&old_order.metadata.uid, &new_order, new_fee)
            .await?;
        Metrics::on_order_operation(&old_order, OrderOperation::Cancelled);
        Metrics::on_order_operation(&new_order, OrderOperation::Created);

        self.solvable_orders.request_update();

        Ok(new_order.metadata.uid)
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

pub async fn filter_unsupported_tokens(
    mut orders: Vec<Order>,
    bad_token: &dyn BadTokenDetecting,
) -> Result<Vec<Order>> {
    // Can't use normal `retain` or `filter` because the bad token detection is async. So either
    // this manual iteration or conversion to stream.
    let mut index = 0;
    'outer: while index < orders.len() {
        for token in orders[index].data.token_pair().unwrap() {
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
    use crate::{
        account_balances::MockBalanceFetching, api::order_validation::MockOrderValidating,
        database::orders::MockOrderStoring, metrics::NoopMetrics,
    };
    use ethcontract::H160;
    use futures::FutureExt;
    use mockall::predicate::eq;
    use model::{
        app_id::AppId,
        order::{OrderBuilder, OrderData, OrderMetadata},
        signature::CreationSignature,
    };
    use shared::{
        bad_token::{list_based::ListBasedDetector, MockBadTokenDetecting},
        current_block,
        price_estimation::native::MockNativePriceEstimating,
    };

    fn mock_orderbook() -> Orderbook {
        Orderbook {
            domain_separator: Default::default(),
            settlement_contract: H160([0xba; 20]),
            database: Arc::new(MockOrderStoring::new()),
            bad_token_detector: Arc::new(MockBadTokenDetecting::new()),
            solvable_orders: SolvableOrdersCache::new(
                Duration::default(),
                Arc::new(MockOrderStoring::new()),
                Default::default(),
                Arc::new(MockBalanceFetching::new()),
                Arc::new(MockBadTokenDetecting::new()),
                current_block::mock_single_block(Default::default()),
                Arc::new(MockNativePriceEstimating::new()),
                Arc::new(NoopMetrics),
            ),
            solvable_orders_max_update_age: Default::default(),
            order_validator: Arc::new(MockOrderValidating::new()),
        }
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

    #[tokio::test]
    async fn replace_order_verifies_signer_and_app_data() {
        let old_order = Order {
            metadata: OrderMetadata {
                uid: OrderUid([1; 56]),
                owner: H160([1; 20]),
                ..Default::default()
            },
            ..Default::default()
        };
        let new_order_uid = OrderUid([2; 56]);
        let cancellation = OrderCancellation {
            order_uid: old_order.metadata.uid,
            ..Default::default()
        };

        let mut database = MockOrderStoring::new();
        database
            .expect_single_order()
            .with(eq(old_order.metadata.uid))
            .returning({
                let old_order = old_order.clone();
                move |_| Ok(Some(old_order.clone()))
            });
        database.expect_replace_order().returning(|_, _, _| Ok(()));

        let mut order_validator = MockOrderValidating::new();
        order_validator
            .expect_validate_and_construct_order()
            .returning(move |creation, _, _| {
                Ok((
                    Order {
                        metadata: OrderMetadata {
                            owner: creation.signature.expected_owner().unwrap(),
                            uid: new_order_uid,
                            ..Default::default()
                        },
                        data: creation.data,
                        signature: creation.signature.to_protocol_signature(),
                    },
                    Default::default(),
                ))
            });

        let orderbook = Orderbook {
            database: Arc::new(database),
            order_validator: Arc::new(order_validator),
            ..mock_orderbook()
        };

        // App data does not encode cancellation.
        assert!(matches!(
            orderbook
                .replace_order(
                    old_order.metadata.uid,
                    OrderCreation {
                        signature: CreationSignature::Eip712 {
                            from: Some(old_order.metadata.owner),
                            signature: Default::default(),
                        },
                        ..Default::default()
                    },
                )
                .await,
            Err(ReplaceOrderError::InvalidReplacement)
        ));

        // Different owner
        assert!(matches!(
            orderbook
                .replace_order(
                    old_order.metadata.uid,
                    OrderCreation {
                        signature: CreationSignature::Eip712 {
                            from: Some(H160([2; 20])),
                            signature: Default::default(),
                        },
                        data: OrderData {
                            app_data: AppId(cancellation.hash_struct()),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                )
                .await,
            Err(ReplaceOrderError::InvalidReplacement)
        ));

        // Non-signed order.
        assert!(matches!(
            orderbook
                .replace_order(
                    old_order.metadata.uid,
                    OrderCreation {
                        signature: CreationSignature::PreSign {
                            from: old_order.metadata.owner,
                        },
                        data: OrderData {
                            app_data: AppId(cancellation.hash_struct()),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                )
                .await,
            Err(ReplaceOrderError::InvalidReplacement)
        ));

        // Stars align...
        assert_eq!(
            orderbook
                .replace_order(
                    old_order.metadata.uid,
                    OrderCreation {
                        signature: CreationSignature::Eip712 {
                            from: Some(old_order.metadata.owner),
                            signature: Default::default(),
                        },
                        data: OrderData {
                            app_data: AppId(cancellation.hash_struct()),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                )
                .await
                .unwrap(),
            new_order_uid,
        );
    }
}
