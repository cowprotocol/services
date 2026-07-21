use {
    crate::{Address, OrderUid, PgTransaction, TransactionHash},
    sqlx::{Executor, PgConnection, QueryBuilder, types::BigDecimal},
    tracing::instrument,
};

/// Maximum number of rows inserted per batched `INSERT` statement. Keeps the
/// number of bind parameters comfortably below Postgres' hard limit of 65535
/// (the widest event table below has 6 columns, i.e. 30000 parameters here).
const INSERT_BATCH_SIZE: usize = 5000;

#[derive(Clone, Debug)]
pub enum Event {
    Trade(Trade),
    Invalidation(Invalidation),
    Settlement(Settlement),
    PreSignature(PreSignature),
}

#[derive(Clone, Debug, Default)]
pub struct Trade {
    pub order_uid: OrderUid,
    pub sell_amount_including_fee: BigDecimal,
    pub buy_amount: BigDecimal,
    pub fee_amount: BigDecimal,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Invalidation {
    pub order_uid: OrderUid,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Settlement {
    pub solver: Address,
    pub transaction_hash: TransactionHash,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PreSignature {
    pub owner: Address,
    pub order_uid: OrderUid,
    pub signed: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, sqlx::FromRow)]
pub struct EventIndex {
    pub block_number: i64,
    pub log_index: i64,
}

#[instrument(skip_all)]
pub async fn delete(
    ex: &mut PgTransaction<'_>,
    delete_from_block_number: u64,
) -> Result<(), sqlx::Error> {
    let delete_from_block_number = i64::try_from(delete_from_block_number).unwrap_or(i64::MAX);
    const QUERY_INVALIDATION: &str = "DELETE FROM invalidations WHERE block_number >= $1;";
    ex.execute(sqlx::query(QUERY_INVALIDATION).bind(delete_from_block_number))
        .await?;

    const QUERY_TRADE: &str = "DELETE FROM trades WHERE block_number >= $1;";
    ex.execute(sqlx::query(QUERY_TRADE).bind(delete_from_block_number))
        .await?;

    const QUERY_SETTLEMENTS: &str = "DELETE FROM settlements WHERE block_number >= $1;";
    ex.execute(sqlx::query(QUERY_SETTLEMENTS).bind(delete_from_block_number))
        .await?;

    const QUERY_PRESIGNATURES: &str = "DELETE FROM presignature_events WHERE block_number >= $1;";
    ex.execute(sqlx::query(QUERY_PRESIGNATURES).bind(delete_from_block_number))
        .await?;

    Ok(())
}

#[instrument(skip_all)]
pub async fn append(
    ex: &mut PgTransaction<'_>,
    events: &[(EventIndex, Event)],
) -> Result<(), sqlx::Error> {
    // Group by event type so each table is written with a single (chunked)
    // multi-row `INSERT` instead of one round-trip per event. A settlement
    // typically emits one `Settlement` and many `Trade` events, so batching the
    // trades is the biggest win. All four tables have `PRIMARY KEY
    // (block_number, log_index)` and a log maps to exactly one event, so no two
    // rows in a single batch can share the conflict key; `ON CONFLICT DO
    // NOTHING` is preserved unchanged.
    let mut trades = Vec::new();
    let mut invalidations = Vec::new();
    let mut settlements = Vec::new();
    let mut presignatures = Vec::new();
    for (index, event) in events {
        match event {
            Event::Trade(event) => trades.push((index, event)),
            Event::Invalidation(event) => invalidations.push((index, event)),
            Event::Settlement(event) => settlements.push((index, event)),
            Event::PreSignature(event) => presignatures.push((index, event)),
        };
    }

    insert_trades(ex, &trades).await?;
    insert_invalidations(ex, &invalidations).await?;
    insert_settlements(ex, &settlements).await?;
    insert_presignatures(ex, &presignatures).await?;
    Ok(())
}

async fn insert_trades(
    ex: &mut PgConnection,
    trades: &[(&EventIndex, &Trade)],
) -> Result<(), sqlx::Error> {
    // `chunks` never yields an empty slice, so an empty input inserts nothing.
    for chunk in trades.chunks(INSERT_BATCH_SIZE) {
        let mut builder = QueryBuilder::new(
            "INSERT INTO trades (block_number, log_index, order_uid, sell_amount, buy_amount, \
             fee_amount) ",
        );
        builder.push_values(chunk, |mut builder, (index, event)| {
            builder
                .push_bind(index.block_number)
                .push_bind(index.log_index)
                .push_bind(event.order_uid)
                .push_bind(event.sell_amount_including_fee.clone())
                .push_bind(event.buy_amount.clone())
                .push_bind(event.fee_amount.clone());
        });
        builder.push(" ON CONFLICT DO NOTHING");
        builder.build().execute(&mut *ex).await?;
    }
    Ok(())
}

async fn insert_invalidations(
    ex: &mut PgConnection,
    invalidations: &[(&EventIndex, &Invalidation)],
) -> Result<(), sqlx::Error> {
    for chunk in invalidations.chunks(INSERT_BATCH_SIZE) {
        let mut builder =
            QueryBuilder::new("INSERT INTO invalidations (block_number, log_index, order_uid) ");
        builder.push_values(chunk, |mut builder, (index, event)| {
            builder
                .push_bind(index.block_number)
                .push_bind(index.log_index)
                .push_bind(event.order_uid);
        });
        builder.push(" ON CONFLICT DO NOTHING");
        builder.build().execute(&mut *ex).await?;
    }
    Ok(())
}

async fn insert_settlements(
    ex: &mut PgConnection,
    settlements: &[(&EventIndex, &Settlement)],
) -> Result<(), sqlx::Error> {
    for chunk in settlements.chunks(INSERT_BATCH_SIZE) {
        let mut builder = QueryBuilder::new(
            "INSERT INTO settlements (tx_hash, block_number, log_index, solver) ",
        );
        builder.push_values(chunk, |mut builder, (index, event)| {
            builder
                .push_bind(event.transaction_hash)
                .push_bind(index.block_number)
                .push_bind(index.log_index)
                .push_bind(event.solver);
        });
        builder.push(" ON CONFLICT DO NOTHING");
        builder.build().execute(&mut *ex).await?;
    }
    Ok(())
}

async fn insert_presignatures(
    ex: &mut PgConnection,
    presignatures: &[(&EventIndex, &PreSignature)],
) -> Result<(), sqlx::Error> {
    for chunk in presignatures.chunks(INSERT_BATCH_SIZE) {
        let mut builder = QueryBuilder::new(
            "INSERT INTO presignature_events (block_number, log_index, owner, order_uid, signed) ",
        );
        builder.push_values(chunk, |mut builder, (index, event)| {
            builder
                .push_bind(index.block_number)
                .push_bind(index.log_index)
                .push_bind(event.owner)
                .push_bind(event.order_uid)
                .push_bind(event.signed);
        });
        builder.push(" ON CONFLICT DO NOTHING");
        builder.build().execute(&mut *ex).await?;
    }
    Ok(())
}

#[instrument(skip_all)]
pub async fn insert_trade(
    ex: &mut PgConnection,
    index: &EventIndex,
    event: &Trade,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = "\
        INSERT INTO trades (block_number, log_index, order_uid, sell_amount, buy_amount, \
                         fee_amount) VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT DO NOTHING;";
    sqlx::query(QUERY)
        .bind(index.block_number)
        .bind(index.log_index)
        .bind(event.order_uid)
        .bind(&event.sell_amount_including_fee)
        .bind(&event.buy_amount)
        .bind(&event.fee_amount)
        .execute(ex)
        .await?;
    Ok(())
}

#[instrument(skip_all)]
pub async fn insert_settlement(
    ex: &mut PgConnection,
    index: &EventIndex,
    event: &Settlement,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = "\
    INSERT INTO settlements (tx_hash, block_number, log_index, solver) VALUES ($1, $2, $3, $4) ON \
                         CONFLICT DO NOTHING;";
    sqlx::query(QUERY)
        .bind(event.transaction_hash)
        .bind(index.block_number)
        .bind(index.log_index)
        .bind(event.solver)
        .execute(ex)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use {super::*, crate::byte_array::ByteArray, sqlx::Connection};

    async fn count(ex: &mut PgConnection, table: &str) -> i64 {
        sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(ex)
            .await
            .unwrap()
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_append_batches_all_event_types() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let ev = |block: i64, log: i64| EventIndex {
            block_number: block,
            log_index: log,
        };
        let events = vec![
            (
                ev(1, 0),
                Event::Settlement(Settlement {
                    solver: ByteArray([1; 20]),
                    transaction_hash: ByteArray([2; 32]),
                }),
            ),
            (
                ev(1, 1),
                Event::Trade(Trade {
                    order_uid: ByteArray([3; 56]),
                    sell_amount_including_fee: BigDecimal::from(10),
                    buy_amount: BigDecimal::from(20),
                    fee_amount: BigDecimal::from(1),
                }),
            ),
            (
                ev(1, 2),
                Event::Trade(Trade {
                    order_uid: ByteArray([4; 56]),
                    sell_amount_including_fee: BigDecimal::from(30),
                    buy_amount: BigDecimal::from(40),
                    fee_amount: BigDecimal::from(2),
                }),
            ),
            (
                ev(1, 3),
                Event::Invalidation(Invalidation {
                    order_uid: ByteArray([5; 56]),
                }),
            ),
            (
                ev(1, 4),
                Event::PreSignature(PreSignature {
                    owner: ByteArray([6; 20]),
                    order_uid: ByteArray([7; 56]),
                    signed: true,
                }),
            ),
        ];

        // Empty input is a no-op.
        append(&mut db, &[]).await.unwrap();

        append(&mut db, &events).await.unwrap();
        assert_eq!(count(&mut db, "trades").await, 2);
        assert_eq!(count(&mut db, "settlements").await, 1);
        assert_eq!(count(&mut db, "invalidations").await, 1);
        assert_eq!(count(&mut db, "presignature_events").await, 1);

        // Re-appending the same events is a no-op thanks to `ON CONFLICT DO
        // NOTHING` on the (block_number, log_index) primary key.
        append(&mut db, &events).await.unwrap();
        assert_eq!(count(&mut db, "trades").await, 2);
        assert_eq!(count(&mut db, "settlements").await, 1);
        assert_eq!(count(&mut db, "invalidations").await, 1);
        assert_eq!(count(&mut db, "presignature_events").await, 1);
    }
}
