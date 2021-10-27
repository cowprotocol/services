use crate::{
    api::order_validation::{OrderValidating, OrderValidator, ValidationError},
    database::orders::{InsertionError, OrderFilter, OrderStoring},
    solvable_orders::{SolvableOrders, SolvableOrdersCache},
};
use anyhow::{ensure, Context, Result};
use chrono::Utc;
use ethcontract::H256;
use model::{
    order::{Order, OrderCancellation, OrderCreationPayload, OrderStatus, OrderUid},
    signature::SigningScheme,
    DomainSeparator,
};
use primitive_types::H160;
use shared::{bad_token::BadTokenDetecting, metrics::LivenessChecking};
use std::{collections::HashSet, sync::Arc, time::Duration};

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

        let order = match self
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

#[cfg(test)]
mod tests {
    use super::*;
    use ethcontract::H160;
    use futures::FutureExt;
    use model::order::OrderBuilder;
    use shared::bad_token::list_based::ListBasedDetector;

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
