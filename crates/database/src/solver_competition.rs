use sqlx::{types::JsonValue, PgConnection};

pub type SolverCompetitionId = i64;

pub async fn save(
    ex: &mut PgConnection,
    data: &JsonValue,
) -> Result<SolverCompetitionId, sqlx::Error> {
    const QUERY: &str = r#"
INSERT INTO solver_competitions (json)
VALUES ($1)
RETURNING id
    ;"#;
    let (id,) = sqlx::query_as(QUERY).bind(data).fetch_one(ex).await?;
    Ok(id)
}

pub async fn load(
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
        let id_ = save(&mut db, &value).await.unwrap();
        assert_eq!(id, id_);

        let value_ = load(&mut db, id).await.unwrap().unwrap();
        assert_eq!(value, value_);

        assert!(load(&mut db, id + 1).await.unwrap().is_none());
    }
}
