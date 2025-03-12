use {
    super::Postgres,
    crate::dto::TokenMetadata,
    anyhow::{Context as _, Result},
    async_trait::async_trait,
    bigdecimal::ToPrimitive,
    chrono::{DateTime, Utc},
    database::{
        byte_array::ByteArray,
        order_events::{OrderEvent, OrderEventLabel, insert_order_event},
        orders::{self, FullOrder, OrderKind as DbOrderKind},
    },
    ethcontract::H256,
    futures::{FutureExt, StreamExt, stream::TryStreamExt},
    model::{
        order::{Order, OrderUid},
        time::now_in_epoch_seconds,
    },
    number::conversions::{big_decimal_to_u256, u256_to_big_decimal},
    primitive_types::{H160, U256},
    shared::{
        db_order_conversions::{
            buy_token_destination_into,
            order_class_into,
            order_kind_into,
            sell_token_source_into,
            signing_scheme_into,
        },
        fee::FeeParameters,
        order_validation::{Amounts, LimitOrderCounting, is_order_outside_market_price},
    },
    sqlx::{Connection, PgConnection},
    std::convert::TryInto,
};

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait OrderStoring: Send + Sync {
    async fn insert_order(&self, order: &Order) -> Result<(), InsertionError>;
    async fn cancel_orders(&self, order_uids: Vec<OrderUid>, now: DateTime<Utc>) -> Result<()>;
    async fn cancel_order(&self, order_uid: &OrderUid, now: DateTime<Utc>) -> Result<()>;
    async fn replace_order(
        &self,
        old_order: &OrderUid,
        new_order: &Order,
    ) -> Result<(), InsertionError>;
    async fn orders_for_tx(&self, tx_hash: &H256) -> Result<Vec<Order>>;
    /// All orders of a single user ordered by creation date descending (newest
    /// orders first).
    async fn user_orders(
        &self,
        owner: &H160,
        offset: u64,
        limit: Option<u64>,
    ) -> Result<Vec<Order>>;
    async fn latest_order_event(&self, order_uid: &OrderUid) -> Result<Option<OrderEvent>>;
    async fn single_order(&self, uid: &OrderUid) -> Result<Option<Order>>;
}

#[derive(Debug)]
pub enum InsertionError {
    DuplicatedRecord,
    DbError(sqlx::Error),
    /// Full app data to be inserted doesn't match existing.
    AppDataMismatch(Vec<u8>),
    MetadataSerializationFailed(serde_json::Error),
}

impl From<sqlx::Error> for InsertionError {
    fn from(err: sqlx::Error) -> Self {
        Self::DbError(err)
    }
}

/// Applies the needed DB modification to cancel a single order.
async fn cancel_order(
    ex: &mut PgConnection,
    order_uid: &OrderUid,
    now: DateTime<Utc>,
) -> Result<()> {
    let uid = ByteArray(order_uid.0);
    insert_order_event(
        ex,
        &OrderEvent {
            order_uid: uid,
            timestamp: now,
            label: OrderEventLabel::Cancelled,
        },
    )
    .await?;
    database::orders::cancel_order(ex, &uid, now).await?;
    Ok(())
}

async fn insert_order(order: &Order, ex: &mut PgConnection) -> Result<(), InsertionError> {
    let order_uid = ByteArray(order.metadata.uid.0);
    insert_order_event(
        ex,
        &OrderEvent {
            order_uid,
            timestamp: Utc::now(),
            label: OrderEventLabel::Created,
        },
    )
    .await?;
    let interactions = std::iter::empty()
        .chain(
            order
                .interactions
                .pre
                .iter()
                .map(|interaction| (interaction, database::orders::ExecutionTime::Pre)),
        )
        .chain(
            order
                .interactions
                .post
                .iter()
                .map(|interaction| (interaction, database::orders::ExecutionTime::Post)),
        )
        .enumerate()
        .map(
            |(index, (interaction, execution))| database::orders::Interaction {
                target: ByteArray(interaction.target.0),
                value: u256_to_big_decimal(&interaction.value),
                data: interaction.call_data.clone(),
                index: index
                    .try_into()
                    .expect("interactions count cannot overflow a i32"),
                execution,
            },
        )
        .collect::<Vec<_>>();

    let db_order = database::orders::Order {
        uid: order_uid,
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
        cancellation_timestamp: None,
    };

    database::orders::insert_order(ex, &db_order)
        .await
        .map_err(|err| {
            if database::orders::is_duplicate_record_error(&err) {
                InsertionError::DuplicatedRecord
            } else {
                InsertionError::DbError(err)
            }
        })?;
    database::orders::insert_interactions(ex, &db_order.uid, &interactions)
        .await
        .map_err(InsertionError::DbError)?;

    if let Some(quote) = order.metadata.quote.as_ref() {
        let db_quote = database::orders::Quote {
            order_uid,
            // safe to unwrap as these values were converted from f64 previously
            gas_amount: quote.gas_amount.to_f64().unwrap(),
            gas_price: quote.gas_price.to_f64().unwrap(),
            sell_token_price: quote.sell_token_price.to_f64().unwrap(),
            sell_amount: u256_to_big_decimal(&quote.sell_amount),
            buy_amount: u256_to_big_decimal(&quote.buy_amount),
            solver: ByteArray(quote.solver.0),
            verified: quote.verified,
            metadata: quote.metadata.clone(),
        };
        database::orders::insert_quote(ex, &db_quote)
            .await
            .map_err(InsertionError::DbError)?;
    }

    Ok(())
}

#[async_trait::async_trait]
impl OrderStoring for Postgres {
    async fn insert_order(&self, order: &Order) -> Result<(), InsertionError> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["insert_order"])
            .start_timer();

        let order = order.clone();
        let mut connection = self.pool.acquire().await?;
        let mut ex = connection.begin().await?;

        insert_order(&order, &mut ex).await?;
        Self::insert_order_app_data(&order, &mut ex).await?;

        ex.commit().await?;
        Ok(())
    }

    async fn cancel_orders(&self, order_uids: Vec<OrderUid>, now: DateTime<Utc>) -> Result<()> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["cancel_orders"])
            .start_timer();

        let mut connection = self.pool.begin().await?;
        for order_uid in order_uids {
            cancel_order(&mut connection, &order_uid, now).await?;
        }
        connection
            .commit()
            .await
            .context("commit cancel multiple orders")
    }

    async fn cancel_order(&self, order_uid: &OrderUid, now: DateTime<Utc>) -> Result<()> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["cancel_order"])
            .start_timer();

        let mut ex = self.pool.begin().await?;
        cancel_order(&mut ex, order_uid, now).await?;
        ex.commit().await.context("commit cancel single order")
    }

    async fn replace_order(
        &self,
        old_order: &model::order::OrderUid,
        new_order: &model::order::Order,
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
                    Self::insert_order_app_data(&new_order, ex).await?;

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

        match orders::single_full_order_with_quote(&mut ex, &ByteArray(uid.0)).await? {
            Some(order_with_quote) => {
                let (order, quote) = order_with_quote.into_order_and_quote();
                Some(shared::db_order_conversions::full_order_into_model_order(
                    order,
                    quote.as_ref(),
                ))
            }
            None => {
                // try to find the order in the JIT orders table
                database::jit_orders::get_by_id(&mut ex, &ByteArray(uid.0))
                    .await?
                    .map(full_order_into_model_order)
            }
        }
        .transpose()
    }

    async fn orders_for_tx(&self, tx_hash: &H256) -> Result<Vec<Order>> {
        tokio::try_join!(
            self.user_order_for_tx(tx_hash),
            self.jit_orders_for_tx(tx_hash)
        )
        .map(|(mut user_orders, jit_orders)| {
            user_orders.extend(jit_orders);
            user_orders
        })
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
        database::order_history::user_orders(
            &mut ex,
            &ByteArray(owner.0),
            i64::try_from(offset).unwrap_or(i64::MAX),
            limit.map(|l| i64::try_from(l).unwrap_or(i64::MAX)),
        )
        .map(|result| match result {
            Ok(order) => full_order_into_model_order(order),
            Err(err) => Err(anyhow::Error::from(err)),
        })
        .try_collect()
        .await
    }

    async fn latest_order_event(&self, order_uid: &OrderUid) -> Result<Option<OrderEvent>> {
        let mut ex = self.pool.begin().await.context("could not init tx")?;
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["latest_order_event"])
            .start_timer();

        database::order_events::get_latest(&mut ex, &ByteArray(order_uid.0))
            .await
            .context("order_events::get_latest")
    }
}

impl Postgres {
    /// Retrieve all user posted orders for a given transaction.
    pub async fn user_order_for_tx(&self, tx_hash: &H256) -> Result<Vec<Order>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["user_order_for_tx"])
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

    /// Retrieve all JIT orders for a given transaction.
    pub async fn jit_orders_for_tx(&self, tx_hash: &H256) -> Result<Vec<Order>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["jit_orders_for_tx"])
            .start_timer();

        let mut ex = self.pool.acquire().await?;
        database::jit_orders::get_by_tx(&mut ex, &ByteArray(tx_hash.0))
            .await?
            .into_iter()
            .map(full_order_into_model_order)
            .collect::<Result<Vec<_>>>()
    }

    pub async fn token_metadata(&self, token: &H160) -> Result<TokenMetadata> {
        let (first_trade_block, native_price): (Option<u32>, Option<U256>) = tokio::try_join!(
            self.execute_instrumented("token_first_trade_block", async {
                let mut ex = self.pool.acquire().await?;
                database::trades::token_first_trade_block(&mut ex, ByteArray(token.0))
                    .await
                    .map_err(anyhow::Error::from)?
                    .map(u32::try_from)
                    .transpose()
                    .map_err(anyhow::Error::from)
            }),
            self.execute_instrumented("fetch_latest_token_price", async {
                let mut ex = self.pool.acquire().await?;
                Ok(
                    database::auction_prices::fetch_latest_token_price(&mut ex, ByteArray(token.0))
                        .await
                        .map_err(anyhow::Error::from)?
                        .and_then(|price| big_decimal_to_u256(&price)),
                )
            })
        )?;

        Ok(TokenMetadata {
            first_trade_block,
            native_price,
        })
    }

    async fn execute_instrumented<F, T>(&self, label: &str, f: F) -> Result<T>
    where
        F: std::future::Future<Output = Result<T>>,
    {
        let timer = super::Metrics::get()
            .database_queries
            .with_label_values(&[label])
            .start_timer();

        let result = f.await?;

        timer.observe_duration();

        Ok(result)
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
        Ok(database::orders::user_orders_with_quote(
            &mut ex,
            now_in_epoch_seconds().into(),
            &ByteArray(owner.0),
        )
        .await?
        .into_iter()
        .filter(|order_with_quote| {
            is_order_outside_market_price(
                &Amounts {
                    sell: big_decimal_to_u256(&order_with_quote.order_sell_amount).unwrap(),
                    buy: big_decimal_to_u256(&order_with_quote.order_buy_amount).unwrap(),
                    fee: 0.into(),
                },
                &Amounts {
                    sell: big_decimal_to_u256(&order_with_quote.quote_sell_amount).unwrap(),
                    buy: big_decimal_to_u256(&order_with_quote.quote_buy_amount).unwrap(),
                    fee: FeeParameters {
                        gas_amount: order_with_quote.quote_gas_amount,
                        gas_price: order_with_quote.quote_gas_price,
                        sell_token_price: order_with_quote.quote_sell_token_price,
                    }
                    .fee(),
                },
                match order_with_quote.order_kind {
                    DbOrderKind::Buy => model::order::OrderKind::Buy,
                    DbOrderKind::Sell => model::order::OrderKind::Sell,
                },
            )
        })
        .count()
        .try_into()
        .unwrap())
    }
}

fn full_order_into_model_order(order: FullOrder) -> Result<Order> {
    shared::db_order_conversions::full_order_into_model_order(order, None)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        model::{
            interaction::InteractionData,
            order::{Interactions, Order, OrderData, OrderMetadata, OrderStatus, OrderUid},
            signature::{Signature, SigningScheme},
        },
        primitive_types::U256,
        shared::order_quoting::{Quote, QuoteData, QuoteMetadataV1},
        std::sync::atomic::{AtomicI64, Ordering},
    };

    #[tokio::test]
    #[ignore]
    async fn postgres_replace_order() {
        let owner = H160([0x77; 20]);

        let db = Postgres::try_new("postgresql://").unwrap();
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
        db.insert_order(&old_order).await.unwrap();

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
        db.replace_order(&old_order.metadata.uid, &new_order)
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

        let db = Postgres::try_new("postgresql://").unwrap();
        database::clear_DANGER(&db.pool).await.unwrap();

        let old_order = Order {
            metadata: OrderMetadata {
                owner,
                uid: OrderUid([1; 56]),
                ..Default::default()
            },
            ..Default::default()
        };
        db.insert_order(&old_order).await.unwrap();

        let new_order = Order {
            metadata: OrderMetadata {
                owner,
                uid: OrderUid([2; 56]),
                creation_date: Utc::now(),
                ..Default::default()
            },
            ..Default::default()
        };
        db.insert_order(&new_order).await.unwrap();

        // Attempt to replace an old order with one that already exists should fail.
        let err = db
            .replace_order(&old_order.metadata.uid, &new_order)
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
        let db = Postgres::try_new("postgresql://").unwrap();
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
        db.insert_order(&order).await.unwrap();

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
        let db = Postgres::try_new("postgresql://").unwrap();
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

        db.insert_order(&order(1)).await.unwrap();
        db.insert_order(&order(2)).await.unwrap();
        db.insert_order(&order(3)).await.unwrap();

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

    #[tokio::test]
    #[ignore]
    async fn postgres_insert_orders_with_interactions() {
        let db = Postgres::try_new("postgresql://").unwrap();
        database::clear_DANGER(&db.pool).await.unwrap();

        let interaction = |byte: u8| InteractionData {
            target: H160([byte; 20]),
            value: byte.into(),
            call_data: vec![byte; byte as _],
        };

        let quote = Quote {
            id: Some(5),
            sell_amount: U256::from(1),
            buy_amount: U256::from(2),
            data: QuoteData {
                fee_parameters: FeeParameters {
                    sell_token_price: 2.5,
                    gas_amount: 0.01,
                    gas_price: 0.003,
                },
                ..Default::default()
            },
            ..Default::default()
        };

        let uid = OrderUid([0x42; 56]);
        let order = Order {
            data: OrderData {
                valid_to: u32::MAX,
                ..Default::default()
            },
            metadata: OrderMetadata {
                uid,
                quote: Some(quote.try_to_model_order_quote().unwrap()),
                ..Default::default()
            },
            interactions: Interactions {
                pre: vec![interaction(1), interaction(2), interaction(3)],
                post: vec![interaction(4), interaction(5)],
            },
            ..Default::default()
        };

        db.insert_order(&order).await.unwrap();

        let single_order = db.single_order(&uid).await.unwrap().unwrap();
        assert_eq!(
            single_order.metadata.quote,
            Some(quote.try_to_model_order_quote().unwrap())
        );
        assert_eq!(single_order, order);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_insert_orders_with_interactions_and_verified() {
        let db = Postgres::try_new("postgresql://").unwrap();
        database::clear_DANGER(&db.pool).await.unwrap();

        let quote = Quote {
            id: Some(5),
            sell_amount: U256::from(1),
            buy_amount: U256::from(2),
            data: QuoteData {
                verified: true,
                metadata: QuoteMetadataV1 {
                    interactions: vec![
                        InteractionData {
                            target: H160([1; 20]),
                            value: U256::from(100),
                            call_data: vec![1, 20],
                        },
                        InteractionData {
                            target: H160([2; 20]),
                            value: U256::from(10),
                            call_data: vec![2, 20],
                        },
                    ],
                    pre_interactions: vec![InteractionData {
                        target: H160([3; 20]),
                        value: U256::from(30),
                        call_data: vec![3, 20],
                    }],
                    jit_orders: vec![],
                }
                .into(),
                ..Default::default()
            },
            ..Default::default()
        };

        let uid = OrderUid([0x42; 56]);
        let order = Order {
            data: OrderData {
                valid_to: u32::MAX,
                ..Default::default()
            },
            metadata: OrderMetadata {
                uid,
                quote: Some(quote.try_to_model_order_quote().unwrap()),
                ..Default::default()
            },
            ..Default::default()
        };

        db.insert_order(&order).await.unwrap();

        let single_order = db.single_order(&uid).await.unwrap().unwrap();

        assert_eq!(
            single_order.metadata.quote,
            Some(quote.try_to_model_order_quote().unwrap())
        );
        assert_eq!(single_order, order);
    }
}
