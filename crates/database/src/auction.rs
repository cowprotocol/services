use sqlx::{types::JsonValue, PgConnection};

pub type AuctionId = i64;

pub async fn load_most_recent(
    ex: &mut PgConnection,
) -> Result<Option<(AuctionId, JsonValue)>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT id, json
FROM auctions
ORDER BY id DESC
LIMIT 1
    ;"#;
    sqlx::query_as(QUERY).fetch_optional(ex).await
}

pub async fn replace_auction(
    ex: &mut PgConnection,
    data: &JsonValue,
) -> Result<AuctionId, sqlx::Error> {
    const QUERY: &str = r#"
WITH deleted AS (
    DELETE FROM auctions
)
INSERT INTO auctions (json)
VALUES ($1)
RETURNING id;
    "#;

    let (id,) = sqlx::query_as(QUERY).bind(data).fetch_one(ex).await?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use {super::*, sqlx::Connection};

    #[tokio::test]
    #[ignore]
    async fn postgres_roundtrip() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let value = JsonValue::Number(1.into());
        let id = replace_auction(&mut db, &value).await.unwrap();
        let (id_, value_) = load_most_recent(&mut db).await.unwrap().unwrap();
        assert_eq!(id, id_);
        assert_eq!(value, value_);

        let value = JsonValue::Number(2.into());
        let id_ = replace_auction(&mut db, &value).await.unwrap();
        assert_eq!(id + 1, id_);
        let (id, value_) = load_most_recent(&mut db).await.unwrap().unwrap();
        assert_eq!(value, value_);
        assert_eq!(id_, id);
    }
}
