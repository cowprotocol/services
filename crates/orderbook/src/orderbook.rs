use crate::database::orders::{InsertionError, OrderStoring};
use anyhow::{Context, Result};
use chrono::Utc;
use ethcontract::H256;
use futures::StreamExt;
use model::{
    auction::AuctionWithId,
    order::{Order, OrderCancellation, OrderCreation, OrderStatus, OrderUid},
    DomainSeparator,
};
use primitive_types::H160;
use shared::{
    metrics::LivenessChecking,
    order_validation::{OrderValidating, ValidationError},
    price_estimation::native::NativePriceEstimating,
};
use std::sync::Arc;
use thiserror::Error;

#[derive(prometheus_metric_storage::MetricStorage, Clone, Debug)]
#[metric(subsystem = "orderbook")]
struct Metrics {
    /// Counter for measuring order statistics.
    #[metric(labels("kind", "operation"))]
    orders: prometheus::IntCounterVec,
}

enum OrderOperation {
    Created,
    Cancelled,
}

impl Metrics {
    fn on_order_operation(order: &Order, operation: OrderOperation) {
        let metrics = Self::instance(global_metrics::get_metric_storage_registry())
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
    database: crate::database::Postgres,
    order_validator: Arc<dyn OrderValidating>,
    native_price_estimator: Arc<dyn NativePriceEstimating>,
}

impl Orderbook {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        domain_separator: DomainSeparator,
        settlement_contract: H160,
        database: crate::database::Postgres,
        order_validator: Arc<dyn OrderValidating>,
        native_price_estimator: Arc<dyn NativePriceEstimating>,
    ) -> Self {
        Self {
            domain_separator,
            settlement_contract,
            database,
            order_validator,
            native_price_estimator,
        }
    }

    pub async fn add_order(&self, payload: OrderCreation) -> Result<OrderUid, AddOrderError> {
        let (order, quote) = self
            .order_validator
            .validate_and_construct_order(payload, &self.domain_separator, self.settlement_contract)
            .await?;

        self.database.insert_order(&order, quote).await?;
        Metrics::on_order_operation(&order, OrderOperation::Created);

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
            .map_err(|_| OrderCancellationError::InvalidSignature)?;
        if signer != order.metadata.owner {
            return Err(OrderCancellationError::WrongOwner);
        };

        // order is already known to exist in DB at this point, and signer is
        // known to be correct!
        self.database
            .cancel_order(&order.metadata.uid, Utc::now())
            .await?;
        Metrics::on_order_operation(&order, OrderOperation::Cancelled);

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
            .scheme()
            .try_to_ecdsa_scheme()
            .ok_or(ReplaceOrderError::InvalidReplacement)?;

        let old_order = self.find_order_for_cancellation(&old_order).await?;
        let (new_order, new_quote) = self
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
            .replace_order(&old_order.metadata.uid, &new_order, new_quote)
            .await?;
        Metrics::on_order_operation(&old_order, OrderOperation::Cancelled);
        Metrics::on_order_operation(&new_order, OrderOperation::Created);

        Ok(new_order.metadata.uid)
    }

    pub async fn get_order(&self, uid: &OrderUid) -> Result<Option<Order>> {
        self.database.single_order(uid).await
    }

    pub async fn get_orders_for_tx(&self, hash: &H256) -> Result<Vec<Order>> {
        self.database.orders_for_tx(hash).await
    }

    pub async fn get_auction(&self) -> Result<Option<AuctionWithId>> {
        let auction = match self.database.most_recent_auction().await? {
            Some(auction) => auction,
            None => {
                tracing::warn!("there is no current auction");
                return Ok(None);
            }
        };
        Ok(Some(auction))
    }

    pub async fn get_user_orders(
        &self,
        owner: &H160,
        offset: u64,
        limit: u64,
    ) -> Result<Vec<Order>> {
        self.database
            .user_orders(owner, offset, Some(limit))
            .await
            .context("get_user_orders error")
    }

    pub async fn get_native_prices(&self, tokens: &[H160]) -> Result<Vec<f64>> {
        self.native_price_estimator
            .estimate_native_prices(tokens)
            .map(|(_, price)| price.context("missing estimation"))
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect()
    }
}

#[async_trait::async_trait]
impl LivenessChecking for Orderbook {
    async fn is_alive(&self) -> bool {
        self.get_auction().await.is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::orders::MockOrderStoring;
    use ethcontract::H160;
    use mockall::predicate::eq;
    use model::{
        app_id::AppId,
        order::{OrderData, OrderMetadata},
        signature::Signature,
    };
    use shared::order_validation::MockOrderValidating;

    #[tokio::test]
    #[ignore]
    async fn postgres_replace_order_verifies_signer_and_app_data() {
        let old_order = Order {
            metadata: OrderMetadata {
                uid: OrderUid([1; 56]),
                owner: H160([1; 20]),
                ..Default::default()
            },
            data: OrderData {
                valid_to: u32::MAX,
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
                            owner: creation.from.unwrap(),
                            uid: new_order_uid,
                            ..Default::default()
                        },
                        data: creation.data,
                        signature: creation.signature,
                    },
                    Default::default(),
                ))
            });

        let database = crate::database::Postgres::new("postgresql://").unwrap();
        database::clear_DANGER(&database.pool).await.unwrap();
        database.insert_order(&old_order, None).await.unwrap();
        let orderbook = Orderbook {
            database,
            order_validator: Arc::new(order_validator),
            domain_separator: Default::default(),
            settlement_contract: H160([0xba; 20]),
        };

        // App data does not encode cancellation.
        assert!(matches!(
            orderbook
                .replace_order(
                    old_order.metadata.uid,
                    OrderCreation {
                        from: Some(old_order.metadata.owner),
                        signature: Signature::Eip712(Default::default()),
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
                        from: Some(H160([2; 20])),
                        signature: Signature::Eip712(Default::default()),
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
                        from: Some(old_order.metadata.owner),
                        signature: Signature::PreSign,
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
                        from: Some(old_order.metadata.owner),
                        signature: Signature::Eip712(Default::default()),
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
