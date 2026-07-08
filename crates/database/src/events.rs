use {
    crate::{Address, OrderUid, PgTransaction, TransactionHash},
    sqlx::{Executor, PgConnection, types::BigDecimal},
    tracing::instrument,
};

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
    pub tx_hash: TransactionHash,
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
    // TODO: there might be a more efficient way to do this like execute_many or
    // COPY but my tests show that even if we sleep during the transaction it
    // does not block other connections from using the database, so it's not
    // high priority.
    for (index, event) in events {
        match event {
            Event::Trade(event) => insert_trade(ex, index, event).await?,
            Event::Invalidation(event) => insert_invalidation(ex, index, event).await?,
            Event::Settlement(event) => insert_settlement(ex, index, event).await?,
            Event::PreSignature(event) => insert_presignature(ex, index, event).await?,
        };
    }
    Ok(())
}

#[instrument(skip_all)]
async fn insert_invalidation(
    ex: &mut PgConnection,
    index: &EventIndex,
    event: &Invalidation,
) -> Result<(), sqlx::Error> {
    // We use ON CONFLICT so that multiple updates running at the same do not error
    // because of events already existing. This can happen when multiple
    // orderbook apis run in HPA. See #444 .
    const QUERY: &str = "INSERT INTO invalidations (block_number, log_index, order_uid) VALUES \
                         ($1, $2, $3) ON CONFLICT DO NOTHING;";
    sqlx::query(QUERY)
        .bind(index.block_number)
        .bind(index.log_index)
        .bind(event.order_uid)
        .execute(ex)
        .await?;
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
                         fee_amount, tx_hash) VALUES ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT DO \
                         NOTHING;";
    sqlx::query(QUERY)
        .bind(index.block_number)
        .bind(index.log_index)
        .bind(event.order_uid)
        .bind(&event.sell_amount_including_fee)
        .bind(&event.buy_amount)
        .bind(&event.fee_amount)
        .bind(event.tx_hash)
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

#[instrument(skip_all)]
async fn insert_presignature(
    ex: &mut PgConnection,
    index: &EventIndex,
    event: &PreSignature,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = "\
        INSERT INTO presignature_events (block_number, log_index, owner, order_uid, signed) VALUES \
                         ($1, $2, $3, $4, $5) ON CONFLICT DO NOTHING;";
    sqlx::query(QUERY)
        .bind(index.block_number)
        .bind(index.log_index)
        .bind(event.owner)
        .bind(event.order_uid)
        .bind(event.signed)
        .execute(ex)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use {super::*, crate::byte_array::ByteArray, sqlx::Connection};

    #[tokio::test]
    #[ignore]
    async fn postgres_insert_trade_stores_tx_hash() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let index = EventIndex {
            block_number: 1,
            log_index: 0,
        };
        let event = Trade {
            order_uid: ByteArray([1u8; 56]),
            tx_hash: ByteArray([2u8; 32]),
            ..Default::default()
        };
        insert_trade(&mut db, &index, &event).await.unwrap();

        let tx_hash: TransactionHash = sqlx::query_scalar("SELECT tx_hash FROM trades")
            .fetch_one(&mut *db)
            .await
            .unwrap();
        assert_eq!(tx_hash, event.tx_hash);
    }
}
