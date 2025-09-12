use {
    crate::{Address, OrderUid, PgTransaction, TransactionHash},
    sqlx::{Executor, PgConnection, Postgres, QueryBuilder, types::BigDecimal},
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
    ex: &mut PgConnection,
    events: &[(EventIndex, Event)],
) -> Result<(), sqlx::Error> {
    if events.is_empty() {
        return Ok(());
    }

    let mut settlements = vec![];
    let mut trades = vec![];
    let mut invalidations = vec![];
    let mut pre_signatures = vec![];

    for (index, event) in events {
        match event {
            Event::Settlement(settlement) => settlements.push((index, settlement)),
            Event::Trade(trade) => trades.push((index, trade)),
            Event::Invalidation(invalidation) => invalidations.push((index, invalidation)),
            Event::PreSignature(pre_signature) => pre_signatures.push((index, pre_signature)),
        }
    }

    // In order to minimize round trips we insert into multiple tables at once using
    // a CTE chain like:
    // WITH insertion1 AS (INSERT INTO <TABLE> (field1, field2) VALUES ($1, $2)),
    // insertion3 AS (INSERT INTO <TABLE2> (field1, field2) VALUES ($1, $2)),
    // SELECT 1;

    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new("WITH ");
    if !settlements.is_empty() {
        qb.push(
            "insSettlements AS (INSERT INTO settlements (tx_hash, block_number, log_index, \
             solver) ",
        );
        qb.push_values(settlements, |mut builder, (index, settlement)| {
            builder.push_bind(settlement.transaction_hash);
            builder.push_bind(index.block_number);
            builder.push_bind(index.log_index);
            builder.push_bind(settlement.solver);
        });
    }

    if !trades.is_empty() {
        qb.push(
            "), insTrades AS (INSERT INTO trades (block_number, log_index, order_uid, \
             sell_amount, buy_amount, fee_amount)",
        );
        qb.push_values(trades, |mut builder, (index, trade)| {
            builder.push_bind(index.block_number);
            builder.push_bind(index.log_index);
            builder.push_bind(trade.order_uid);
            builder.push_bind(&trade.sell_amount_including_fee);
            builder.push_bind(&trade.buy_amount);
            builder.push_bind(&trade.fee_amount);
        });
    }

    if !invalidations.is_empty() {
        qb.push(
            "), insInvalidations AS (INSERT INTO invalidations (block_number, log_index, \
             order_uid) ",
        );
        qb.push_values(invalidations, |mut builder, (index, invalidation)| {
            builder.push_bind(index.block_number);
            builder.push_bind(index.log_index);
            builder.push_bind(invalidation.order_uid);
        });
    }

    if !pre_signatures.is_empty() {
        qb.push(
            "), insPreSignatures AS (INSERT INTO presignature_events (block_number, log_index, \
             owner, order_uid, signed)",
        );
        qb.push_values(pre_signatures, |mut builder, (index, pre_signature)| {
            builder.push_bind(index.block_number);
            builder.push_bind(index.log_index);
            builder.push_bind(pre_signature.owner);
            builder.push_bind(pre_signature.order_uid);
            builder.push_bind(pre_signature.signed);
        });
    }

    // final select just to complete the CTE chain
    qb.push(") SELECT 1;");

    let start = std::time::Instant::now();
    let query = qb.sql().to_string();
    tracing::error!(time = ?start.elapsed(), query, "finished insert");
    qb.build().execute(ex).await?;
    tracing::error!(time = ?start.elapsed(), query, "finished insert");

    Ok(())
}

#[instrument(skip_all)]
async fn insert_invalidation(
    ex: &mut PgConnection,
    index: &EventIndex,
    event: &Invalidation,
) -> Result<(), sqlx::Error> {
    append(ex, &[(*index, Event::Invalidation(*event))]).await
}

#[instrument(skip_all)]
pub async fn insert_trade(
    ex: &mut PgConnection,
    index: &EventIndex,
    event: &Trade,
) -> Result<(), sqlx::Error> {
    append(ex, &[(*index, Event::Trade(event.clone()))]).await
}

#[instrument(skip_all)]
pub async fn insert_settlement(
    ex: &mut PgConnection,
    index: &EventIndex,
    event: &Settlement,
) -> Result<(), sqlx::Error> {
    append(ex, &[(*index, Event::Settlement(*event))]).await
}

#[instrument(skip_all)]
async fn insert_presignature(
    ex: &mut PgConnection,
    index: &EventIndex,
    event: &PreSignature,
) -> Result<(), sqlx::Error> {
    append(ex, &[(*index, Event::PreSignature(*event))]).await
}
