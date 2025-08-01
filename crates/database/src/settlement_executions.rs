use {
    crate::{Address, auction::AuctionId},
    chrono::{DateTime, Utc},
    sqlx::PgConnection,
    tracing::instrument,
};

#[instrument(skip_all)]
pub async fn insert(
    ex: &mut PgConnection,
    auction_id: AuctionId,
    solver: Address,
    solution_uid: i64,
    start_timestamp: DateTime<Utc>,
    start_block: i64,
    deadline_block: i64,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
INSERT INTO settlement_executions (auction_id, solver, solution_uid, start_timestamp, start_block, deadline_block)
VALUES ($1, $2, $3, $4, $5, $6)
    ;"#;

    sqlx::query(QUERY)
        .bind(auction_id)
        .bind(solver)
        .bind(solution_uid)
        .bind(start_timestamp)
        .bind(start_block)
        .bind(deadline_block)
        .execute(ex)
        .await?;

    Ok(())
}

#[instrument(skip_all)]
pub async fn update(
    ex: &mut PgConnection,
    auction_id: AuctionId,
    solver: Address,
    solution_uid: i64,
    end_timestamp: DateTime<Utc>,
    end_block: i64,
    outcome: String,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
UPDATE settlement_executions
SET end_timestamp = $4, end_block = $5, outcome = $6
WHERE auction_id = $1 AND solver = $2 AND solution_uid = $3
    ;"#;

    sqlx::query(QUERY)
        .bind(auction_id)
        .bind(solver)
        .bind(solution_uid)
        .bind(end_timestamp)
        .bind(end_block)
        .bind(outcome)
        .execute(ex)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::byte_array::ByteArray,
        chrono::Timelike,
        sqlx::{Connection, PgConnection},
    };

    #[tokio::test]
    #[ignore]
    async fn postgres_roundtrip() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let auction_id = 1;
        let solver_a = ByteArray([1u8; 20]);
        let solver_b = ByteArray([2u8; 20]);
        let start_timestamp = now_truncated_to_microseconds();
        let start_block = 1;
        let deadline_block = 10;

        insert(
            &mut db,
            auction_id,
            solver_a,
            1,
            start_timestamp,
            start_block,
            deadline_block,
        )
        .await
        .unwrap();

        insert(
            &mut db,
            auction_id,
            solver_a,
            2,
            start_timestamp,
            start_block,
            deadline_block,
        )
        .await
        .unwrap();
        insert(
            &mut db,
            auction_id,
            solver_b,
            1,
            start_timestamp,
            start_block,
            deadline_block,
        )
        .await
        .unwrap();

        let output = fetch(&mut db, auction_id).await.unwrap();
        assert_eq!(output.len(), 3);
        let expected_a = ExecutionRow {
            auction_id,
            solver: solver_a,
            solution_uid: 1,
            start_timestamp,
            end_timestamp: None,
            start_block,
            end_block: None,
            deadline_block,
            outcome: None,
        };
        let expected_b = ExecutionRow {
            auction_id,
            solver: solver_a,
            solution_uid: 2,
            start_timestamp,
            end_timestamp: None,
            start_block,
            end_block: None,
            deadline_block,
            outcome: None,
        };
        let expected_c = ExecutionRow {
            auction_id,
            solver: solver_b,
            solution_uid: 1,
            start_timestamp,
            end_timestamp: None,
            start_block,
            end_block: None,
            deadline_block,
            outcome: None,
        };
        assert!(output.contains(&expected_a));
        assert!(output.contains(&expected_b));
        assert!(output.contains(&expected_c));

        let end_timestamp_a = now_truncated_to_microseconds();
        let end_block_a = 8;
        let success_outcome = "success".to_string();
        let failure_outcome = "failure".to_string();
        update(
            &mut db,
            auction_id,
            solver_a,
            1,
            end_timestamp_a,
            end_block_a,
            success_outcome.clone(),
        )
        .await
        .unwrap();
        update(
            &mut db,
            auction_id,
            solver_a,
            2,
            end_timestamp_a,
            end_block_a,
            failure_outcome.clone(),
        )
        .await
        .unwrap();

        let end_timestamp_b = now_truncated_to_microseconds();
        let end_block_b = 10;
        update(
            &mut db,
            auction_id,
            solver_b,
            1,
            end_timestamp_b,
            end_block_b,
            success_outcome.clone(),
        )
        .await
        .unwrap();

        let output = fetch(&mut db, auction_id).await.unwrap();
        assert_eq!(output.len(), 3);
        let expected_a = ExecutionRow {
            auction_id,
            solver: solver_a,
            solution_uid: 1,
            start_timestamp,
            end_timestamp: Some(end_timestamp_a),
            start_block,
            end_block: Some(end_block_a),
            deadline_block,
            outcome: Some(success_outcome.clone()),
        };
        let expected_b = ExecutionRow {
            auction_id,
            solver: solver_a,
            solution_uid: 2,
            start_timestamp,
            end_timestamp: Some(end_timestamp_a),
            start_block,
            end_block: Some(end_block_a),
            deadline_block,
            outcome: Some(failure_outcome),
        };
        let expected_c = ExecutionRow {
            auction_id,
            solver: solver_b,
            solution_uid: 1,
            start_timestamp,
            end_timestamp: Some(end_timestamp_b),
            start_block,
            end_block: Some(end_block_b),
            deadline_block,
            outcome: Some(success_outcome),
        };
        assert!(output.contains(&expected_a));
        assert!(output.contains(&expected_b));
        assert!(output.contains(&expected_c));
    }

    #[derive(Debug, Clone, Eq, PartialEq, sqlx::FromRow)]
    struct ExecutionRow {
        pub auction_id: AuctionId,
        pub solver: Address,
        pub solution_uid: i64,
        pub start_timestamp: DateTime<Utc>,
        pub end_timestamp: Option<DateTime<Utc>>,
        pub start_block: i64,
        pub end_block: Option<i64>,
        pub deadline_block: i64,
        pub outcome: Option<String>,
    }

    async fn fetch(
        ex: &mut PgConnection,
        auction_id: AuctionId,
    ) -> Result<Vec<ExecutionRow>, sqlx::Error> {
        const QUERY: &str = r#"SELECT * FROM settlement_executions WHERE auction_id = $1;"#;

        sqlx::query_as(QUERY).bind(auction_id).fetch_all(ex).await
    }

    /// In the DB we use `timestampz` which doesn't store nanoseconds, so we
    /// truncate them to make the comparison work.
    fn now_truncated_to_microseconds() -> DateTime<Utc> {
        Utc::now().with_nanosecond(0).unwrap()
    }
}
