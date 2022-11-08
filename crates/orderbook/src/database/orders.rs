use super::Postgres;
use anyhow::{Context as _, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use database::{
    byte_array::ByteArray,
    orders::{FullOrder, OrderKind as DbOrderKind},
};
use ethcontract::H256;
use futures::{stream::TryStreamExt, FutureExt, StreamExt};
use model::{
    app_id::AppId,
    order::{
        EthflowData, Interactions, Order, OrderClass, OrderData, OrderMetadata, OrderStatus,
        OrderUid,
    },
    signature::Signature,
    time::now_in_epoch_seconds,
};
use num::Zero;
use number_conversions::{big_decimal_to_big_uint, big_decimal_to_u256, u256_to_big_decimal};
use primitive_types::H160;
use shared::{
    db_order_conversions::{
        buy_token_destination_from, buy_token_destination_into, extract_pre_interactions,
        order_class_from, order_class_into, order_kind_from, order_kind_into,
        sell_token_source_from, sell_token_source_into, signing_scheme_from, signing_scheme_into,
    },
    order_quoting::Quote,
    order_validation::LimitOrderCounting,
};
use sqlx::{types::BigDecimal, Connection, PgConnection};
use std::convert::TryInto;

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait OrderStoring: Send + Sync {
    async fn insert_order(&self, order: &Order, quote: Option<Quote>)
        -> Result<(), InsertionError>;
    async fn cancel_order(&self, order_uid: &OrderUid, now: DateTime<Utc>) -> Result<()>;
    async fn replace_order(
        &self,
        old_order: &OrderUid,
        new_order: &Order,
        new_quote: Option<Quote>,
    ) -> Result<(), InsertionError>;
    async fn orders_for_tx(&self, tx_hash: &H256) -> Result<Vec<Order>>;
    async fn single_order(&self, uid: &OrderUid) -> Result<Option<Order>>;
    /// All orders of a single user ordered by creation date descending (newest orders first).
    async fn user_orders(
        &self,
        owner: &H160,
        offset: u64,
        limit: Option<u64>,
    ) -> Result<Vec<Order>>;
}

pub struct SolvableOrders {
    pub orders: Vec<Order>,
    pub latest_settlement_block: u64,
}

#[derive(Debug)]
pub enum InsertionError {
    DuplicatedRecord,
    DbError(sqlx::Error),
}

impl From<sqlx::Error> for InsertionError {
    fn from(err: sqlx::Error) -> Self {
        Self::DbError(err)
    }
}

async fn insert_order(order: &Order, ex: &mut PgConnection) -> Result<(), InsertionError> {
    let order = database::orders::Order {
        uid: ByteArray(order.metadata.uid.0),
        owner: ByteArray(order.metadata.owner.0),
        creation_timestamp: order.metadata.creation_date,
        sell_token: ByteArray(order.data.sell_token.0),
        buy_token: ByteArray(order.data.buy_token.0),
        receiver: order.data.receiver.map(|h160| ByteArray(h160.0)),
        sell_amount: u256_to_big_decimal(&order.data.sell_amount),
        buy_amount: u256_to_big_decimal(&order.data.buy_amount),
        valid_to: order.data.valid_to as i64,
        app_data: ByteArray(order.data.app_data.0),
        fee_amount: u256_to_big_decimal(&order.data.fee_amount),
        kind: order_kind_into(order.data.kind),
        class: order_class_into(order.metadata.class),
        partially_fillable: order.data.partially_fillable,
        signature: order.signature.to_bytes(),
        signing_scheme: signing_scheme_into(order.signature.scheme()),
        settlement_contract: ByteArray(order.metadata.settlement_contract.0),
        sell_token_balance: sell_token_source_into(order.data.sell_token_balance),
        buy_token_balance: buy_token_destination_into(order.data.buy_token_balance),
        full_fee_amount: u256_to_big_decimal(&order.metadata.full_fee_amount),
        cancellation_timestamp: None,
        surplus_fee: u256_to_big_decimal(&order.metadata.surplus_fee),
        surplus_fee_timestamp: order.metadata.surplus_fee_timestamp,
    };
    database::orders::insert_order(ex, &order)
        .await
        .map_err(|err| {
            if database::orders::is_duplicate_record_error(&err) {
                InsertionError::DuplicatedRecord
            } else {
                InsertionError::DbError(err)
            }
        })
}

async fn insert_quote(
    uid: &OrderUid,
    quote: &Quote,
    ex: &mut PgConnection,
) -> Result<(), InsertionError> {
    let quote = database::orders::Quote {
        order_uid: ByteArray(uid.0),
        gas_amount: quote.data.fee_parameters.gas_amount,
        gas_price: quote.data.fee_parameters.gas_price,
        sell_token_price: quote.data.fee_parameters.sell_token_price,
        sell_amount: u256_to_big_decimal(&quote.sell_amount),
        buy_amount: u256_to_big_decimal(&quote.buy_amount),
    };
    database::orders::insert_quote(ex, &quote)
        .await
        .map_err(InsertionError::DbError)?;
    Ok(())
}

#[async_trait::async_trait]
impl OrderStoring for Postgres {
    async fn insert_order(
        &self,
        order: &Order,
        quote: Option<Quote>,
    ) -> Result<(), InsertionError> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["insert_order"])
            .start_timer();

        let order = order.clone();
        let mut connection = self.pool.acquire().await?;
        connection
            .transaction(move |transaction| {
                async move {
                    insert_order(&order, transaction).await?;
                    if let Some(quote) = quote {
                        insert_quote(&order.metadata.uid, &quote, transaction).await?;
                    }
                    Ok(())
                }
                .boxed()
            })
            .await
    }

    async fn cancel_order(&self, order_uid: &OrderUid, now: DateTime<Utc>) -> Result<()> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["cancel_order"])
            .start_timer();

        let order_uid = *order_uid;
        let mut ex = self.pool.acquire().await?;
        database::orders::cancel_order(&mut ex, &ByteArray(order_uid.0), now)
            .await
            .context("cancel_order")
    }

    async fn replace_order(
        &self,
        old_order: &model::order::OrderUid,
        new_order: &model::order::Order,
        new_quote: Option<Quote>,
    ) -> anyhow::Result<(), super::orders::InsertionError> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["replace_order"])
            .start_timer();

        let old_order = *old_order;
        let new_order = new_order.clone();
        let mut connection = self.pool.acquire().await?;
        connection
            .transaction(move |ex| {
                async move {
                    database::orders::cancel_order(
                        ex,
                        &ByteArray(old_order.0),
                        new_order.metadata.creation_date,
                    )
                    .await?;
                    insert_order(&new_order, ex).await?;
                    if let Some(quote) = new_quote {
                        insert_quote(&new_order.metadata.uid, &quote, ex).await?;
                    }
                    Ok(())
                }
                .boxed()
            })
            .await
    }

    async fn single_order(&self, uid: &OrderUid) -> Result<Option<Order>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["single_order"])
            .start_timer();

        let mut ex = self.pool.acquire().await?;
        let order = database::orders::single_full_order(&mut ex, &ByteArray(uid.0)).await?;
        order.map(full_order_into_model_order).transpose()
    }

    async fn orders_for_tx(&self, tx_hash: &H256) -> Result<Vec<Order>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["orders_for_tx"])
            .start_timer();

        let mut ex = self.pool.acquire().await?;
        database::orders::full_orders_in_tx(&mut ex, &ByteArray(tx_hash.0))
            .map(|result| match result {
                Ok(order) => full_order_into_model_order(order),
                Err(err) => Err(anyhow::Error::from(err)),
            })
            .try_collect()
            .await
    }

    async fn user_orders(
        &self,
        owner: &H160,
        offset: u64,
        limit: Option<u64>,
    ) -> Result<Vec<Order>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["user_orders"])
            .start_timer();

        let mut ex = self.pool.acquire().await?;
        database::orders::user_orders(
            &mut ex,
            &ByteArray(owner.0),
            offset as i64,
            limit.map(|l| l as i64),
        )
        .map(|result| match result {
            Ok(order) => full_order_into_model_order(order),
            Err(err) => Err(anyhow::Error::from(err)),
        })
        .try_collect()
        .await
    }
}

#[async_trait]
impl LimitOrderCounting for Postgres {
    async fn count(&self, owner: H160) -> Result<u64> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["count_limit_orders"])
            .start_timer();

        let mut ex = self.pool.acquire().await?;
        Ok(database::orders::count_limit_orders(
            &mut ex,
            now_in_epoch_seconds().try_into().unwrap(),
            &ByteArray(owner.0),
        )
        .await?
        .try_into()
        .unwrap())
    }
}

fn calculate_status(order: &FullOrder) -> OrderStatus {
    match order.kind {
        DbOrderKind::Buy => {
            if is_buy_order_filled(&order.buy_amount, &order.sum_buy) {
                return OrderStatus::Fulfilled;
            }
        }
        DbOrderKind::Sell => {
            if is_sell_order_filled(&order.sell_amount, &order.sum_sell, &order.sum_fee) {
                return OrderStatus::Fulfilled;
            }
        }
    }
    if order.invalidated {
        return OrderStatus::Cancelled;
    }
    if order.valid_to < Utc::now().timestamp() {
        return OrderStatus::Expired;
    }
    if order.presignature_pending {
        return OrderStatus::PresignaturePending;
    }
    OrderStatus::Open
}

fn full_order_into_model_order(order: FullOrder) -> Result<Order> {
    let status = calculate_status(&order);
    let pre_interactions = extract_pre_interactions(&order)?;
    let ethflow_data = if let Some((is_refunded, user_valid_to)) = order.ethflow_data {
        Some(EthflowData {
            user_valid_to,
            is_refunded,
        })
    } else {
        None
    };
    let onchain_user = order.onchain_user.map(|onchain_user| H160(onchain_user.0));
    let class = order_class_from(order.class);
    let metadata = OrderMetadata {
        creation_date: order.creation_timestamp,
        owner: H160(order.owner.0),
        uid: OrderUid(order.uid.0),
        available_balance: Default::default(),
        executed_buy_amount: big_decimal_to_big_uint(&order.sum_buy)
            .context("executed buy amount is not an unsigned integer")?,
        executed_sell_amount: big_decimal_to_big_uint(&order.sum_sell)
            .context("executed sell amount is not an unsigned integer")?,
        // Executed fee amounts and sell amounts before fees are capped by
        // order's fee and sell amounts, and thus can always fit in a `U256`
        // - as it is limited by the order format.
        executed_sell_amount_before_fees: big_decimal_to_u256(&(order.sum_sell - &order.sum_fee))
            .context(
            "executed sell amount before fees does not fit in a u256",
        )?,
        executed_fee_amount: big_decimal_to_u256(&order.sum_fee)
            .context("executed fee amount is not a valid u256")?,
        invalidated: order.invalidated,
        status,
        class,
        settlement_contract: H160(order.settlement_contract.0),
        full_fee_amount: big_decimal_to_u256(&order.full_fee_amount)
            .context("full_fee_amount is not U256")?,
        ethflow_data,
        onchain_user,
        is_liquidity_order: class == OrderClass::Liquidity,
        surplus_fee: big_decimal_to_u256(&order.surplus_fee.unwrap_or_default())
            .context("surplus_fee is not U256")?,
        surplus_fee_timestamp: order.surplus_fee_timestamp.unwrap_or_default(),
    };
    let data = OrderData {
        sell_token: H160(order.sell_token.0),
        buy_token: H160(order.buy_token.0),
        receiver: order.receiver.map(|address| H160(address.0)),
        sell_amount: big_decimal_to_u256(&order.sell_amount).context("sell_amount is not U256")?,
        buy_amount: big_decimal_to_u256(&order.buy_amount).context("buy_amount is not U256")?,
        valid_to: order.valid_to.try_into().context("valid_to is not u32")?,
        app_data: AppId(order.app_data.0),
        fee_amount: big_decimal_to_u256(&order.fee_amount).context("fee_amount is not U256")?,
        kind: order_kind_from(order.kind),
        partially_fillable: order.partially_fillable,
        sell_token_balance: sell_token_source_from(order.sell_token_balance),
        buy_token_balance: buy_token_destination_from(order.buy_token_balance),
    };
    let signing_scheme = signing_scheme_from(order.signing_scheme);
    let signature = Signature::from_bytes(signing_scheme, &order.signature)?;
    Ok(Order {
        metadata,
        data,
        signature,
        interactions: Interactions {
            pre: pre_interactions,
        },
    })
}

fn is_sell_order_filled(
    amount: &BigDecimal,
    executed_amount: &BigDecimal,
    executed_fee: &BigDecimal,
) -> bool {
    if executed_amount.is_zero() {
        return false;
    }
    let total_amount = executed_amount - executed_fee;
    total_amount == *amount
}

fn is_buy_order_filled(amount: &BigDecimal, executed_amount: &BigDecimal) -> bool {
    !executed_amount.is_zero() && *amount == *executed_amount
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use database::{
        byte_array::ByteArray,
        orders::{
            BuyTokenDestination as DbBuyTokenDestination, FullOrder, OrderClass as DbOrderClass,
            OrderKind as DbOrderKind, SellTokenSource as DbSellTokenSource,
            SigningScheme as DbSigningScheme,
        },
    };
    use model::{
        order::{Order, OrderData, OrderMetadata, OrderStatus, OrderUid},
        signature::{Signature, SigningScheme},
    };
    use std::sync::atomic::{AtomicI64, Ordering};

    #[test]
    fn order_status() {
        let valid_to_timestamp = Utc::now() + Duration::days(1);

        let order_row = || FullOrder {
            uid: ByteArray([0; 56]),
            owner: ByteArray([0; 20]),
            creation_timestamp: Utc::now(),
            sell_token: ByteArray([1; 20]),
            buy_token: ByteArray([2; 20]),
            sell_amount: BigDecimal::from(1),
            buy_amount: BigDecimal::from(1),
            valid_to: valid_to_timestamp.timestamp(),
            app_data: ByteArray([0; 32]),
            fee_amount: BigDecimal::default(),
            full_fee_amount: BigDecimal::default(),
            kind: DbOrderKind::Sell,
            class: DbOrderClass::Liquidity,
            partially_fillable: true,
            signature: vec![0; 65],
            receiver: None,
            sum_sell: BigDecimal::default(),
            sum_buy: BigDecimal::default(),
            sum_fee: BigDecimal::default(),
            invalidated: false,
            signing_scheme: DbSigningScheme::Eip712,
            settlement_contract: ByteArray([0; 20]),
            sell_token_balance: DbSellTokenSource::External,
            buy_token_balance: DbBuyTokenDestination::Internal,
            presignature_pending: false,
            pre_interactions: Vec::new(),
            ethflow_data: None,
            onchain_user: None,
            surplus_fee: Default::default(),
            surplus_fee_timestamp: Default::default(),
        };

        // Open - sell (filled - 0%)
        assert_eq!(calculate_status(&order_row()), OrderStatus::Open);

        // Open - sell (almost filled - 99.99%)
        assert_eq!(
            calculate_status(&FullOrder {
                kind: DbOrderKind::Sell,
                sell_amount: BigDecimal::from(10_000),
                sum_sell: BigDecimal::from(9_999),
                ..order_row()
            }),
            OrderStatus::Open
        );

        // Open - with presignature
        assert_eq!(
            calculate_status(&FullOrder {
                signing_scheme: DbSigningScheme::PreSign,
                presignature_pending: false,
                ..order_row()
            }),
            OrderStatus::Open
        );

        // PresignaturePending - without presignature
        assert_eq!(
            calculate_status(&FullOrder {
                signing_scheme: DbSigningScheme::PreSign,
                presignature_pending: true,
                ..order_row()
            }),
            OrderStatus::PresignaturePending
        );

        // Filled - sell (filled - 100%)
        assert_eq!(
            calculate_status(&FullOrder {
                kind: DbOrderKind::Sell,
                sell_amount: BigDecimal::from(2),
                sum_sell: BigDecimal::from(3),
                sum_fee: BigDecimal::from(1),
                ..order_row()
            }),
            OrderStatus::Fulfilled
        );

        // Open - buy (filled - 0%)
        assert_eq!(
            calculate_status(&FullOrder {
                kind: DbOrderKind::Buy,
                buy_amount: BigDecimal::from(1),
                sum_buy: BigDecimal::from(0),
                ..order_row()
            }),
            OrderStatus::Open
        );

        // Open - buy (almost filled - 99.99%)
        assert_eq!(
            calculate_status(&FullOrder {
                kind: DbOrderKind::Buy,
                buy_amount: BigDecimal::from(10_000),
                sum_buy: BigDecimal::from(9_999),
                ..order_row()
            }),
            OrderStatus::Open
        );

        // Filled - buy (filled - 100%)
        assert_eq!(
            calculate_status(&FullOrder {
                kind: DbOrderKind::Buy,
                buy_amount: BigDecimal::from(1),
                sum_buy: BigDecimal::from(1),
                ..order_row()
            }),
            OrderStatus::Fulfilled
        );

        // Cancelled - no fills - sell
        assert_eq!(
            calculate_status(&FullOrder {
                invalidated: true,
                ..order_row()
            }),
            OrderStatus::Cancelled
        );

        // Cancelled - partial fill - sell
        assert_eq!(
            calculate_status(&FullOrder {
                kind: DbOrderKind::Sell,
                sell_amount: BigDecimal::from(2),
                sum_sell: BigDecimal::from(1),
                sum_fee: BigDecimal::default(),
                invalidated: true,
                ..order_row()
            }),
            OrderStatus::Cancelled
        );

        // Cancelled - partial fill - buy
        assert_eq!(
            calculate_status(&FullOrder {
                kind: DbOrderKind::Buy,
                buy_amount: BigDecimal::from(2),
                sum_buy: BigDecimal::from(1),
                invalidated: true,
                ..order_row()
            }),
            OrderStatus::Cancelled
        );

        // Expired - no fills
        let valid_to_yesterday = Utc::now() - Duration::days(1);

        assert_eq!(
            calculate_status(&FullOrder {
                invalidated: false,
                valid_to: valid_to_yesterday.timestamp(),
                ..order_row()
            }),
            OrderStatus::Expired
        );

        // Expired - partial fill - sell
        assert_eq!(
            calculate_status(&FullOrder {
                kind: DbOrderKind::Sell,
                sell_amount: BigDecimal::from(2),
                sum_sell: BigDecimal::from(1),
                invalidated: false,
                valid_to: valid_to_yesterday.timestamp(),
                ..order_row()
            }),
            OrderStatus::Expired
        );

        // Expired - partial fill - buy
        assert_eq!(
            calculate_status(&FullOrder {
                kind: DbOrderKind::Buy,
                buy_amount: BigDecimal::from(2),
                sum_buy: BigDecimal::from(1),
                invalidated: false,
                valid_to: valid_to_yesterday.timestamp(),
                ..order_row()
            }),
            OrderStatus::Expired
        );

        // Expired - with pending presignature
        assert_eq!(
            calculate_status(&FullOrder {
                signing_scheme: DbSigningScheme::PreSign,
                invalidated: false,
                valid_to: valid_to_yesterday.timestamp(),
                presignature_pending: true,
                ..order_row()
            }),
            OrderStatus::Expired
        );
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_replace_order() {
        let owner = H160([0x77; 20]);

        let db = Postgres::new("postgresql://").unwrap();
        database::clear_DANGER(&db.pool).await.unwrap();

        let old_order = Order {
            data: OrderData {
                valid_to: u32::MAX,
                ..Default::default()
            },
            metadata: OrderMetadata {
                owner,
                uid: OrderUid([1; 56]),
                ..Default::default()
            },
            ..Default::default()
        };
        db.insert_order(&old_order, None).await.unwrap();

        let new_order = Order {
            data: OrderData {
                valid_to: u32::MAX,
                ..Default::default()
            },
            metadata: OrderMetadata {
                owner,
                uid: OrderUid([2; 56]),
                creation_date: Utc::now(),
                ..Default::default()
            },
            ..Default::default()
        };
        db.replace_order(&old_order.metadata.uid, &new_order, None)
            .await
            .unwrap();

        let order_statuses = db
            .user_orders(&owner, 0, None)
            .await
            .unwrap()
            .iter()
            .map(|order| (order.metadata.uid, order.metadata.status))
            .collect::<Vec<_>>();
        assert_eq!(
            order_statuses,
            vec![
                (new_order.metadata.uid, OrderStatus::Open),
                (old_order.metadata.uid, OrderStatus::Cancelled),
            ]
        );

        let (old_order_cancellation,): (Option<DateTime<Utc>>,) =
            sqlx::query_as("SELECT cancellation_timestamp FROM orders;")
                .bind(old_order.metadata.uid.0.as_ref())
                .fetch_one(&db.pool)
                .await
                .unwrap();
        assert_eq!(
            old_order_cancellation.unwrap().timestamp_millis(),
            new_order.metadata.creation_date.timestamp_millis(),
        );
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_replace_order_no_cancellation_on_error() {
        let owner = H160([0x77; 20]);

        let db = Postgres::new("postgresql://").unwrap();
        database::clear_DANGER(&db.pool).await.unwrap();

        let old_order = Order {
            metadata: OrderMetadata {
                owner,
                uid: OrderUid([1; 56]),
                ..Default::default()
            },
            ..Default::default()
        };
        db.insert_order(&old_order, None).await.unwrap();

        let new_order = Order {
            metadata: OrderMetadata {
                owner,
                uid: OrderUid([2; 56]),
                creation_date: Utc::now(),
                ..Default::default()
            },
            ..Default::default()
        };
        db.insert_order(&new_order, None).await.unwrap();

        // Attempt to replace an old order with one that already exists should fail.
        let err = db
            .replace_order(&old_order.metadata.uid, &new_order, None)
            .await
            .unwrap_err();
        assert!(matches!(err, InsertionError::DuplicatedRecord));

        // Old order cancellation status should remain unchanged.
        let (old_order_cancellation,): (Option<DateTime<Utc>>,) =
            sqlx::query_as("SELECT cancellation_timestamp FROM orders;")
                .bind(old_order.metadata.uid.0.as_ref())
                .fetch_one(&db.pool)
                .await
                .unwrap();
        assert_eq!(old_order_cancellation, None);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_presignature_status() {
        let db = Postgres::new("postgresql://").unwrap();
        database::clear_DANGER(&db.pool).await.unwrap();
        let uid = OrderUid([0u8; 56]);
        let order = Order {
            data: OrderData {
                valid_to: u32::MAX,
                ..Default::default()
            },
            metadata: OrderMetadata {
                uid,
                ..Default::default()
            },
            signature: Signature::default_with(SigningScheme::PreSign),
            ..Default::default()
        };
        db.insert_order(&order, None).await.unwrap();

        let order_status = || async {
            db.single_order(&order.metadata.uid)
                .await
                .unwrap()
                .unwrap()
                .metadata
                .status
        };
        let block_number = AtomicI64::new(0);
        let insert_presignature = |signed: bool| {
            let db = db.clone();
            let block_number = &block_number;
            let owner = order.metadata.owner.as_bytes();
            async move {
                sqlx::query(
                    "INSERT INTO presignature_events \
                    (block_number, log_index, owner, order_uid, signed) \
                 VALUES \
                    ($1, $2, $3, $4, $5)",
                )
                .bind(block_number.fetch_add(1, Ordering::SeqCst))
                .bind(0i64)
                .bind(owner)
                .bind(&uid.0[..])
                .bind(signed)
                .execute(&db.pool)
                .await
                .unwrap();
            }
        };

        // "presign" order with no signature events has pending status.
        assert_eq!(order_status().await, OrderStatus::PresignaturePending);

        // Inserting a presignature event changes the order status.
        insert_presignature(true).await;
        assert_eq!(order_status().await, OrderStatus::Open);

        // "unsigning" the presignature makes the signature pending again.
        insert_presignature(false).await;
        assert_eq!(order_status().await, OrderStatus::PresignaturePending);

        // Multiple "unsign" events keep the signature pending.
        insert_presignature(false).await;
        assert_eq!(order_status().await, OrderStatus::PresignaturePending);

        // Re-signing sets the status back to open.
        insert_presignature(true).await;
        assert_eq!(order_status().await, OrderStatus::Open);

        // Re-signing sets the status back to open.
        insert_presignature(true).await;
        assert_eq!(order_status().await, OrderStatus::Open);
    }
}
