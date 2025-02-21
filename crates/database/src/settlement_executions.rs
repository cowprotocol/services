use {
    crate::{auction::AuctionId, Address},
    chrono::{DateTime, Utc},
    sqlx::PgConnection,
};

pub async fn insert(
    ex: &mut PgConnection,
    auction_id: AuctionId,
    solver: Address,
    start_timestamp: DateTime<Utc>,
    deadline_block: i64,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
INSERT INTO settlement_executions (auction_id, solver, start_timestamp, deadline_block)
VALUES ($1, $2, $3, $4)
    ;"#;

    sqlx::query(QUERY)
        .bind(auction_id)
        .bind(solver)
        .bind(start_timestamp)
        .bind(deadline_block)
        .execute(ex)
        .await?;

    Ok(())
}

pub async fn update(
    ex: &mut PgConnection,
    auction_id: AuctionId,
    solver: Address,
    end_timestamp: DateTime<Utc>,
    outcome: String,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
UPDATE settlement_executions
SET end_timestamp = $3, outcome = $4
WHERE auction_id = $1 AND solver = $2
    ;"#;

    sqlx::query(QUERY)
        .bind(auction_id)
        .bind(solver)
        .bind(end_timestamp)
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
        let deadline_block = 10;

        insert(
            &mut db,
            auction_id,
            solver_a,
            start_timestamp,
            deadline_block,
        )
        .await
        .unwrap();
        insert(
            &mut db,
            auction_id,
            solver_b,
            start_timestamp,
            deadline_block,
        )
        .await
        .unwrap();

        let output = fetch(&mut db, auction_id).await.unwrap();
        assert_eq!(output.len(), 2);
        let expected_a = ExecutionRow {
            auction_id,
            solver: solver_a,
            start_timestamp,
            end_timestamp: None,
            deadline_block,
            outcome: None,
        };
        let expected_b = ExecutionRow {
            auction_id,
            solver: solver_b,
            start_timestamp,
            end_timestamp: None,
            deadline_block,
            outcome: None,
        };
        assert!(output.contains(&expected_a));
        assert!(output.contains(&expected_b));

        let end_timestamp_a = now_truncated_to_microseconds();
        let outcome_a = "success".to_string();
        update(
            &mut db,
            auction_id,
            solver_a,
            end_timestamp_a,
            outcome_a.clone(),
        )
        .await
        .unwrap();

        let end_timestamp_b = now_truncated_to_microseconds();
        let outcome_b = "failure".to_string();
        update(
            &mut db,
            auction_id,
            solver_b,
            end_timestamp_b,
            outcome_b.clone(),
        )
        .await
        .unwrap();

        let output = fetch(&mut db, auction_id).await.unwrap();
        assert_eq!(output.len(), 2);
        let expected_a = ExecutionRow {
            auction_id,
            solver: solver_a,
            start_timestamp,
            end_timestamp: Some(end_timestamp_a),
            deadline_block,
            outcome: Some(outcome_a),
        };
        let expected_b = ExecutionRow {
            auction_id,
            solver: solver_b,
            start_timestamp,
            end_timestamp: Some(end_timestamp_b),
            deadline_block,
            outcome: Some(outcome_b),
        };
        assert!(output.contains(&expected_a));
        assert!(output.contains(&expected_b));
    }

    #[derive(Debug, Clone, Eq, PartialEq, sqlx::FromRow)]
    struct ExecutionRow {
        pub auction_id: AuctionId,
        pub solver: Address,
        pub start_timestamp: DateTime<Utc>,
        pub end_timestamp: Option<DateTime<Utc>>,
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
        let now = Utc::now();
        now.with_nanosecond((now.nanosecond() / 1_000) * 1_000)
            .unwrap()
    }
}
