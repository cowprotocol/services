use crate::{
    database::orders::{InsertionError, OrderStoring},
    order_validation::{OrderValidating, ValidationError},
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
use shared::metrics::LivenessChecking;
use std::{sync::Arc, time::Duration};
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
    database: Arc<dyn OrderStoring>,
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
        solvable_orders: Arc<SolvableOrdersCache>,
        solvable_orders_max_update_age: Duration,
        order_validator: Arc<dyn OrderValidating>,
    ) -> Self {
        Self {
            domain_separator,
            settlement_contract,
            database,
            solvable_orders,
            solvable_orders_max_update_age,
            order_validator,
        }
    }

    pub async fn add_order(&self, payload: OrderCreation) -> Result<OrderUid, AddOrderError> {
        let (order, quote) = self
            .order_validator
            .validate_and_construct_order(payload, &self.domain_separator, self.settlement_contract)
            .await?;

        self.database.insert_order(&order, quote).await?;
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

        self.solvable_orders.request_update();

        Ok(())
    }

    pub async fn replace_order(
        &self,
        old_order: OrderUid,
        new_order: OrderCreation,
    ) -> Result<OrderUid, ReplaceOrderError> {
        let old_order = self.find_order_for_cancellation(&old_order).await?;
        let (new_order, new_quote) = self
            .order_validator
            .validate_and_construct_order(
                new_order,
                &self.domain_separator,
                self.settlement_contract,
            )
            .await?;

        Self::check_valid_replacement(
            &old_order.metadata.uid,
            &old_order.metadata.owner,
            &new_order,
        )?;

        self.database
            .replace_order(&old_order.metadata.uid, &new_order, new_quote)
            .await?;
        Metrics::on_order_operation(&old_order, OrderOperation::Cancelled);
        Metrics::on_order_operation(&new_order, OrderOperation::Created);

        Ok(new_order.metadata.uid)
    }

    fn check_valid_replacement(
        old_order_uid: &OrderUid,
        old_order_owner: &H160,
        new_order: &Order,
    ) -> Result<(), ReplaceOrderError> {
        // Replacement order signatures need to be validated meaning we cannot
        // accept `PreSign` orders, otherwise anyone can cancel a user order by
        // submitting a `PreSign` order on someone's behalf.
        new_order
            .signature
            .scheme()
            .try_to_ecdsa_scheme()
            .ok_or(ReplaceOrderError::InvalidReplacement)?;

        // Verify that the new order is a valid replacement order by checking
        // that the `app_data` encodes an order cancellation and that both the
        // old and new orders have the same signer.
        let cancellation = OrderCancellation {
            order_uid: *old_order_uid,
            ..Default::default()
        };
        if new_order.data.app_data != cancellation.hash_struct()
            || new_order.metadata.owner != *old_order_owner
        {
            return Err(ReplaceOrderError::InvalidReplacement);
        }

        Ok(())
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

fn set_available_balances(orders: &mut [Order], cache: &SolvableOrdersCache) {
    for order in orders.iter_mut() {
        order.metadata.available_balance =
            cache.cached_balance(&shared::account_balances::Query::from_order(order));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethcontract::H160;
    use model::{
        app_id::AppId,
        order::{OrderData, OrderMetadata},
        signature::Signature,
    };

    #[tokio::test]
    async fn replace_order_verifies_signer_and_app_data() {
        let old_order_uid = OrderUid([1; 56]);
        let old_order_owner = H160([1; 20]);
        let cancellation = OrderCancellation {
            order_uid: old_order_uid,
            ..Default::default()
        };

        // App data does not encode cancellation.
        let new_order = Order {
            metadata: OrderMetadata {
                owner: old_order_owner,
                ..Default::default()
            },
            signature: Signature::Eip712(Default::default()),
            ..Default::default()
        };
        assert!(matches!(
            Orderbook::check_valid_replacement(&old_order_uid, &old_order_owner, &new_order),
            Err(ReplaceOrderError::InvalidReplacement)
        ));

        // Different owner
        let new_order = Order {
            metadata: OrderMetadata {
                owner: H160([2; 20]),
                ..Default::default()
            },
            signature: Signature::Eip712(Default::default()),
            data: OrderData {
                app_data: AppId(cancellation.hash_struct()),
                ..Default::default()
            },
        };
        assert!(matches!(
            Orderbook::check_valid_replacement(&old_order_uid, &old_order_owner, &new_order),
            Err(ReplaceOrderError::InvalidReplacement)
        ));

        // Non-signed order.
        let new_order = Order {
            metadata: OrderMetadata {
                owner: old_order_owner,
                ..Default::default()
            },
            signature: Signature::PreSign,
            data: OrderData {
                app_data: AppId(cancellation.hash_struct()),
                ..Default::default()
            },
        };
        assert!(matches!(
            Orderbook::check_valid_replacement(&old_order_uid, &old_order_owner, &new_order),
            Err(ReplaceOrderError::InvalidReplacement)
        ));

        // Stars align...
        let new_order = Order {
            metadata: OrderMetadata {
                owner: old_order_owner,
                ..Default::default()
            },
            signature: Signature::Eip712(Default::default()),
            data: OrderData {
                app_data: AppId(cancellation.hash_struct()),
                ..Default::default()
            },
        };
        assert!(
            Orderbook::check_valid_replacement(&old_order_uid, &old_order_owner, &new_order)
                .is_ok()
        )
    }
}
