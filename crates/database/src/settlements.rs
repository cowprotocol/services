use {
    crate::{Address, PgTransaction, TransactionHash},
    sqlx::{Executor, PgConnection},
    tracing::instrument,
};

#[instrument(skip_all)]
pub async fn find_settlement_transaction(
    ex: &mut PgConnection,
    auction_id: i64,
    solver: Address,
) -> Result<Option<TransactionHash>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT tx_hash
FROM settlements
WHERE
    auction_id = $1 AND solver = $2
    "#;
    sqlx::query_as(QUERY)
        .bind(auction_id)
        .bind(solver)
        .fetch_optional(ex)
        .await
}

#[derive(Debug, sqlx::FromRow)]
pub struct SettlementEvent {
    pub block_number: i64,
    pub log_index: i64,
    pub tx_hash: TransactionHash,
}

#[instrument(skip_all)]
pub async fn get_settlement_without_auction(
    ex: &mut PgConnection,
) -> Result<Option<SettlementEvent>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT block_number, log_index, tx_hash
FROM settlements
WHERE auction_id IS NULL
ORDER BY block_number ASC
LIMIT 1
    "#;
    sqlx::query_as(QUERY).fetch_optional(ex).await
}

#[instrument(skip_all)]
pub async fn update_settlement_auction(
    ex: &mut PgConnection,
    block_number: i64,
    log_index: i64,
    auction_id: i64,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
UPDATE settlements
SET auction_id = $1
WHERE block_number = $2 AND log_index = $3
    ;"#;
    sqlx::query(QUERY)
        .bind(auction_id)
        .bind(block_number)
        .bind(log_index)
        .execute(ex)
        .await
        .map(|_| ())
}

#[instrument(skip_all)]
pub async fn update_settlement_solver(
    ex: &mut PgConnection,
    block_number: i64,
    log_index: i64,
    solver: Address,
    solution_uid: i64,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
UPDATE settlements
SET solver = $1, solution_uid = $2
WHERE block_number = $3 AND log_index = $4
    ;"#;
    sqlx::query(QUERY)
        .bind(solver)
        .bind(solution_uid)
        .bind(block_number)
        .bind(log_index)
        .execute(ex)
        .await
        .map(|_| ())
}

/// Deletes all database data that referenced the deleted settlement events.
#[instrument(skip_all)]
pub async fn delete(
    ex: &mut PgTransaction<'_>,
    delete_from_block_number: u64,
) -> Result<(), sqlx::Error> {
    let delete_from_block_number = i64::try_from(delete_from_block_number).unwrap_or(i64::MAX);
    const QUERY_OBSERVATIONS: &str =
        "DELETE FROM settlement_observations WHERE block_number >= $1;";
    ex.execute(sqlx::query(QUERY_OBSERVATIONS).bind(delete_from_block_number))
        .await?;

    const QUERY_ORDER_EXECUTIONS: &str = "DELETE FROM order_execution WHERE block_number >= $1;";
    ex.execute(sqlx::query(QUERY_ORDER_EXECUTIONS).bind(delete_from_block_number))
        .await?;

    const QUERY_JIT_ORDERS: &str = "DELETE FROM jit_orders WHERE block_number >= $1;";
    ex.execute(sqlx::query(QUERY_JIT_ORDERS).bind(delete_from_block_number))
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            byte_array::ByteArray,
            events::{Event, EventIndex, Settlement},
        },
        sqlx::Connection,
    };

    async fn all_settlement_tx_hashes(
        ex: &mut PgConnection,
    ) -> Result<Vec<TransactionHash>, sqlx::Error> {
        const QUERY: &str = "SELECT tx_hash FROM settlements";
        sqlx::query_scalar::<_, TransactionHash>(QUERY)
            .fetch_all(ex)
            .await
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_gets_event() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        crate::events::append(
            &mut db,
            &[
                (
                    EventIndex {
                        block_number: 0,
                        log_index: 0,
                    },
                    Event::Settlement(Settlement {
                        solver: Default::default(),
                        transaction_hash: ByteArray([0u8; 32]),
                    }),
                ),
                (
                    EventIndex {
                        block_number: 1,
                        log_index: 0,
                    },
                    Event::Settlement(Settlement {
                        solver: Default::default(),
                        transaction_hash: ByteArray([1u8; 32]),
                    }),
                ),
                (
                    EventIndex {
                        block_number: 2,
                        log_index: 0,
                    },
                    Event::Settlement(Settlement {
                        solver: Default::default(),
                        transaction_hash: ByteArray([2u8; 32]),
                    }),
                ),
            ],
        )
        .await
        .unwrap();

        let results = all_settlement_tx_hashes(&mut db).await.unwrap();
        assert_eq!(
            results,
            &[
                ByteArray([0u8; 32]),
                ByteArray([1u8; 32]),
                ByteArray([2u8; 32])
            ]
        );
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_settlement_auction() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let event = Default::default();
        crate::events::insert_settlement(&mut db, &event, &Default::default())
            .await
            .unwrap();

        let settlement = get_settlement_without_auction(&mut db)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(settlement.block_number, event.block_number);
        assert_eq!(settlement.log_index, event.log_index);

        update_settlement_auction(&mut db, event.block_number, event.log_index, 1)
            .await
            .unwrap();

        let settlement = get_settlement_without_auction(&mut db).await.unwrap();

        assert!(settlement.is_none());
    }
}
