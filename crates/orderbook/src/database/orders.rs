use {
    super::Postgres,
    anyhow::{Context as _, Result},
    async_trait::async_trait,
    chrono::{DateTime, Utc},
    database::byte_array::ByteArray,
    ethcontract::H256,
    futures::{stream::TryStreamExt, FutureExt, StreamExt},
    model::{
        order::{LimitOrderClass, Order, OrderClass, OrderUid},
        time::now_in_epoch_seconds,
    },
    number_conversions::u256_to_big_decimal,
    primitive_types::H160,
    shared::{
        db_order_conversions::{
            buy_token_destination_into,
            full_order_into_model_order,
            order_class_into,
            order_kind_into,
            sell_token_source_into,
            signing_scheme_into,
        },
        order_quoting::Quote,
        order_validation::LimitOrderCounting,
    },
    sqlx::{Connection, PgConnection},
    std::convert::TryInto,
};

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait OrderStoring: Send + Sync {
    async fn insert_order(&self, order: &Order, quote: Option<Quote>)
        -> Result<(), InsertionError>;
    async fn cancel_orders(&self, order_uids: Vec<OrderUid>, now: DateTime<Utc>) -> Result<()>;
    async fn cancel_order(&self, order_uid: &OrderUid, now: DateTime<Utc>) -> Result<()>;
    async fn replace_order(
        &self,
        old_order: &OrderUid,
        new_order: &Order,
        new_quote: Option<Quote>,
    ) -> Result<(), InsertionError>;
    async fn orders_for_tx(&self, tx_hash: &H256) -> Result<Vec<Order>>;
    async fn single_order(&self, uid: &OrderUid) -> Result<Option<Order>>;
    /// All orders of a single user ordered by creation date descending (newest
    /// orders first).
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
        class: order_class_into(&order.metadata.class),
        partially_fillable: order.data.partially_fillable,
        signature: order.signature.to_bytes(),
        signing_scheme: signing_scheme_into(order.signature.scheme()),
        settlement_contract: ByteArray(order.metadata.settlement_contract.0),
        sell_token_balance: sell_token_source_into(order.data.sell_token_balance),
        buy_token_balance: buy_token_destination_into(order.data.buy_token_balance),
        full_fee_amount: u256_to_big_decimal(&order.metadata.full_fee_amount),
        cancellation_timestamp: None,
        surplus_fee: match order.metadata.class {
            OrderClass::Limit(LimitOrderClass { surplus_fee, .. }) => {
                surplus_fee.as_ref().map(u256_to_big_decimal)
            }
            _ => None,
        },
        surplus_fee_timestamp: match order.metadata.class {
            OrderClass::Limit(LimitOrderClass {
                surplus_fee_timestamp,
                ..
            }) => surplus_fee_timestamp,
            _ => None,
        },
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

    async fn cancel_orders(&self, order_uids: Vec<OrderUid>, now: DateTime<Utc>) -> Result<()> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["cancel_orders"])
            .start_timer();

        let mut connection = self.pool.acquire().await?;
        connection
            .transaction(move |ex| {
                async move {
                    for order_uid in order_uids {
                        let uid = ByteArray(order_uid.0);
                        database::orders::cancel_order(ex, &uid, now).await?;
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
            .with_label_values(&["count_limit_orders_by_owner"])
            .start_timer();

        let mut ex = self.pool.acquire().await?;
        Ok(database::orders::count_limit_orders_by_owner(
            &mut ex,
            now_in_epoch_seconds().try_into().unwrap(),
            &ByteArray(owner.0),
        )
        .await?
        .try_into()
        .unwrap())
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        model::{
            order::{Order, OrderData, OrderMetadata, OrderStatus, OrderUid},
            signature::{Signature, SigningScheme},
        },
        std::sync::atomic::{AtomicI64, Ordering},
    };

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
                    "INSERT INTO presignature_events (block_number, log_index, owner, order_uid, \
                     signed) VALUES ($1, $2, $3, $4, $5)",
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

    #[tokio::test]
    #[ignore]
    async fn postgres_cancel_orders() {
        let db = Postgres::new("postgresql://").unwrap();
        database::clear_DANGER(&db.pool).await.unwrap();

        // Define some helper closures to make the test easier to read.
        let uid = |byte: u8| OrderUid([byte; 56]);
        let order = |byte: u8| Order {
            data: OrderData {
                valid_to: u32::MAX,
                ..Default::default()
            },
            metadata: OrderMetadata {
                uid: uid(byte),
                ..Default::default()
            },
            ..Default::default()
        };
        let order_status = |byte: u8| {
            let db = &db;
            let uid = &uid;
            async move {
                db.single_order(&uid(byte))
                    .await
                    .unwrap()
                    .unwrap()
                    .metadata
                    .status
            }
        };

        db.insert_order(&order(1), None).await.unwrap();
        db.insert_order(&order(2), None).await.unwrap();
        db.insert_order(&order(3), None).await.unwrap();

        assert_eq!(order_status(1).await, OrderStatus::Open);
        assert_eq!(order_status(2).await, OrderStatus::Open);
        assert_eq!(order_status(3).await, OrderStatus::Open);

        db.cancel_orders(vec![uid(1), uid(2)], Utc::now())
            .await
            .unwrap();

        assert_eq!(order_status(1).await, OrderStatus::Cancelled);
        assert_eq!(order_status(2).await, OrderStatus::Cancelled);
        assert_eq!(order_status(3).await, OrderStatus::Open);
    }
}
