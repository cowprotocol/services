use crate::TransactionHash;
use sqlx::{types::JsonValue, PgConnection};

pub type SolverCompetitionId = i64;

pub async fn save(
    ex: &mut PgConnection,
    data: &JsonValue,
    tx_hash: Option<&TransactionHash>,
) -> Result<SolverCompetitionId, sqlx::Error> {
    const QUERY: &str = r#"
INSERT INTO solver_competitions (json, tx_hash)
VALUES ($1, $2)
RETURNING id
    "#;
    let (id,) = sqlx::query_as(QUERY)
        .bind(data)
        .bind(tx_hash)
        .fetch_one(ex)
        .await?;
    Ok(id)
}

pub async fn load_by_id(
    ex: &mut PgConnection,
    id: SolverCompetitionId,
) -> Result<Option<JsonValue>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT json
FROM solver_competitions
WHERE id = $1
    ;"#;
    let solver_competition: Option<(JsonValue,)> =
        sqlx::query_as(QUERY).bind(id).fetch_optional(ex).await?;
    Ok(solver_competition.map(|inner| inner.0))
}

pub async fn load_by_tx_hash(
    ex: &mut PgConnection,
    tx_hash: &TransactionHash,
) -> Result<Option<JsonValue>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT json
FROM solver_competitions
WHERE tx_hash = $1
    ;"#;
    let solver_competition: Option<(JsonValue,)> = sqlx::query_as(QUERY)
        .bind(tx_hash)
        .fetch_optional(ex)
        .await?;
    Ok(solver_competition.map(|inner| inner.0))
}

pub async fn next_solver_competition(
    ex: &mut PgConnection,
) -> Result<SolverCompetitionId, sqlx::Error> {
    // The sequence we created for the serial `id` column can be queried for
    // the next value it will produce [0]. Note that the exact semenatics of
    // the sequence's next value depend on its `is_called` flag [1].
    //
    // [0]: <https://www.postgresql.org/docs/current/sql-createsequence.html>
    // [1]: <https://www.postgresql.org/docs/14/functions-sequence.html>
    const QUERY: &str = r#"
SELECT
    CASE
        WHEN is_called THEN last_value + 1
        ELSE last_value
    END AS next
FROM solver_competitions_id_seq
    ;"#;
    let (id,): (SolverCompetitionId,) = sqlx::query_as(QUERY).fetch_one(ex).await?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::byte_array::ByteArray;
    use sqlx::Connection;

    #[tokio::test]
    #[ignore]
    async fn postgres_roundtrip() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let id = next_solver_competition(&mut db).await.unwrap();
        let id_ = next_solver_competition(&mut db).await.unwrap();
        assert_eq!(id, id_);

        let value = JsonValue::Bool(true);
        let id_ = save(&mut db, &value, None).await.unwrap();
        assert_eq!(id, id_);

        let value_ = load_by_id(&mut db, id).await.unwrap().unwrap();
        assert_eq!(value, value_);

        assert!(load_by_id(&mut db, id + 1).await.unwrap().is_none());
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_by_hash() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let value = JsonValue::Bool(true);
        let hash = ByteArray([1u8; 32]);
        let id = save(&mut db, &value, Some(&hash)).await.unwrap();

        let value_by_id = load_by_id(&mut db, id).await.unwrap().unwrap();
        let value_by_hash = load_by_tx_hash(&mut db, &hash).await.unwrap().unwrap();
        assert_eq!(value, value_by_id);
        assert_eq!(value, value_by_hash);

        let not_found = load_by_tx_hash(&mut db, &ByteArray([2u8; 32]))
            .await
            .unwrap();
        assert!(not_found.is_none());
    }
}
