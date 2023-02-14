use {crate::auction::AuctionId, bigdecimal::BigDecimal, sqlx::PgConnection};

#[derive(Debug, PartialEq, sqlx::FromRow)]
pub struct Score {
    pub auction_id: AuctionId,
    pub winning_score: BigDecimal,
    pub reference_score: BigDecimal,
}

pub async fn insert(
    ex: &mut PgConnection,
    auction_id: AuctionId,
    winning_score: BigDecimal,
    reference_score: BigDecimal,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"INSERT INTO settlement_scores (auction_id, winning_score, reference_score) VALUES ($1, $2, $3);"#;
    sqlx::query(QUERY)
        .bind(auction_id)
        .bind(winning_score)
        .bind(reference_score)
        .execute(ex)
        .await?;
    Ok(())
}

pub async fn fetch(
    ex: &mut PgConnection,
    auction_id: AuctionId,
) -> Result<Option<Score>, sqlx::Error> {
    const QUERY: &str = r#"SELECT * FROM settlement_scores WHERE auction_id = $1"#;
    sqlx::query_as(QUERY)
        .bind(auction_id)
        .fetch_optional(ex)
        .await
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

        insert(&mut db, 1, 10.into(), 9.into()).await.unwrap();

        let result = fetch(&mut db, 1).await.unwrap();
        assert_eq!(
            result,
            Some(Score {
                auction_id: 1,
                winning_score: 10.into(),
                reference_score: 9.into(),
            })
        );
    }
}
