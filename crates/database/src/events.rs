use crate::{Address, OrderUid, PgTransaction, TransactionHash};
use sqlx::{types::BigDecimal, Executor, PgConnection};

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

#[derive(Clone, Copy, Debug, Default)]
pub struct EventIndex {
    pub block_number: i64,
    pub log_index: i64,
}

pub async fn last_block(ex: &mut PgConnection) -> Result<i64, sqlx::Error> {
    const QUERY: &str = "\
            SELECT GREATEST( \
                (SELECT COALESCE(MAX(block_number), 0) FROM trades), \
                (SELECT COALESCE(MAX(block_number), 0) FROM settlements), \
                (SELECT COALESCE(MAX(block_number), 0) FROM invalidations), \
                (SELECT COALESCE(MAX(block_number), 0) FROM presignature_events));";
    sqlx::query_scalar(QUERY).fetch_one(ex).await
}

pub async fn delete(
    ex: &mut PgTransaction<'_>,
    delete_from_block_number: i64,
) -> Result<(), sqlx::Error> {
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

pub async fn append(
    ex: &mut PgTransaction<'_>,
    events: &[(EventIndex, Event)],
) -> Result<(), sqlx::Error> {
    // TODO: there might be a more efficient way to do this like execute_many or COPY but my
    // tests show that even if we sleep during the transaction it does not block other
    // connections from using the database, so it's not high priority.
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

async fn insert_invalidation(
    ex: &mut PgConnection,
    index: &EventIndex,
    event: &Invalidation,
) -> Result<(), sqlx::Error> {
    // We use ON CONFLICT so that multiple updates running at the same do not error because of
    // events already existing. This can happen when multiple orderbook apis run in HPA.
    // See #444 .
    const QUERY: &str =
        "INSERT INTO invalidations (block_number, log_index, order_uid) VALUES ($1, $2, $3) \
         ON CONFLICT DO NOTHING;";
    sqlx::query(QUERY)
        .bind(index.block_number)
        .bind(index.log_index)
        .bind(event.order_uid)
        .execute(ex)
        .await?;
    Ok(())
}

async fn insert_trade(
    ex: &mut PgConnection,
    index: &EventIndex,
    event: &Trade,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = "\
        INSERT INTO trades (block_number, log_index, order_uid, sell_amount, buy_amount, fee_amount) VALUES ($1, $2, $3, $4, $5, $6) \
        ON CONFLICT DO NOTHING;";
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

async fn insert_settlement(
    ex: &mut PgConnection,
    index: &EventIndex,
    event: &Settlement,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = "\
        INSERT INTO settlements (tx_hash, block_number, log_index, solver) VALUES ($1, $2, $3, $4) \
        ON CONFLICT DO NOTHING;";
    sqlx::query(QUERY)
        .bind(event.transaction_hash)
        .bind(index.block_number)
        .bind(index.log_index)
        .bind(event.solver)
        .execute(ex)
        .await?;
    Ok(())
}

async fn insert_presignature(
    ex: &mut PgConnection,
    index: &EventIndex,
    event: &PreSignature,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = "\
        INSERT INTO presignature_events (block_number, log_index, owner, order_uid, signed) VALUES ($1, $2, $3, $4, $5) \
        ON CONFLICT DO NOTHING;";
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
    use super::*;
    use sqlx::Connection;

    #[tokio::test]
    #[ignore]
    async fn postgres_events() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        assert_eq!(last_block(&mut db).await.unwrap(), 0);

        let mut event_index = EventIndex {
            block_number: 1,
            log_index: 0,
        };
        append(
            &mut db,
            &[(event_index, Event::Invalidation(Default::default()))],
        )
        .await
        .unwrap();
        assert_eq!(last_block(&mut db).await.unwrap(), 1);

        event_index.block_number = 2;
        append(&mut db, &[(event_index, Event::Trade(Default::default()))])
            .await
            .unwrap();
        assert_eq!(last_block(&mut db).await.unwrap(), 2);

        event_index.block_number = 3;
        append(
            &mut db,
            &[(event_index, Event::PreSignature(Default::default()))],
        )
        .await
        .unwrap();
        assert_eq!(last_block(&mut db).await.unwrap(), 3);

        event_index.block_number = 4;
        append(
            &mut db,
            &[(event_index, Event::Settlement(Default::default()))],
        )
        .await
        .unwrap();
        assert_eq!(last_block(&mut db).await.unwrap(), 4);

        delete(&mut db, 5).await.unwrap();
        assert_eq!(last_block(&mut db).await.unwrap(), 4);

        delete(&mut db, 3).await.unwrap();
        assert_eq!(last_block(&mut db).await.unwrap(), 2);

        delete(&mut db, 0).await.unwrap();
        assert_eq!(last_block(&mut db).await.unwrap(), 0);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_repeated_event_insert_ignored() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();
        async fn append(con: &mut PgTransaction<'_>, log_index: i64, event: Event) {
            super::append(
                con,
                &[(
                    EventIndex {
                        block_number: 2,
                        log_index,
                    },
                    event,
                )],
            )
            .await
            .unwrap()
        }
        for _ in 0..2 {
            append(&mut db, 0, Event::Trade(Default::default())).await;
            append(&mut db, 1, Event::Invalidation(Default::default())).await;
            append(&mut db, 2, Event::Settlement(Default::default())).await;
            append(&mut db, 3, Event::PreSignature(Default::default())).await;
        }
        assert_eq!(last_block(&mut db).await.unwrap(), 2);
    }
}
