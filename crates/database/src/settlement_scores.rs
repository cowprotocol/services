use {
    crate::{Address, PgTransaction, auction::AuctionId},
    bigdecimal::BigDecimal,
    sqlx::PgConnection,
    std::ops::DerefMut,
    tracing::instrument,
};

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct Score {
    pub auction_id: AuctionId,
    pub winner: Address,
    pub winning_score: BigDecimal,
    pub reference_score: BigDecimal,
    pub block_deadline: i64,
    pub simulation_block: i64,
}

#[instrument(skip_all)]
pub async fn insert(ex: &mut PgTransaction<'_>, score: Score) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"INSERT INTO settlement_scores (auction_id, winner, winning_score, reference_score, block_deadline, simulation_block) VALUES ($1, $2, $3, $4, $5, $6);"#;
    sqlx::query(QUERY)
        .bind(score.auction_id)
        .bind(score.winner)
        .bind(score.winning_score)
        .bind(score.reference_score)
        .bind(score.block_deadline)
        .bind(score.simulation_block)
        .execute(ex.deref_mut())
        .await?;
    Ok(())
}

#[instrument(skip_all)]
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
    use {super::*, crate::byte_array::ByteArray, sqlx::Connection};

    #[tokio::test]
    #[ignore]
    async fn postgres_roundtrip() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let input = Score {
            auction_id: 1,
            winner: ByteArray([2; 20]),
            winning_score: 10.into(),
            reference_score: 9.into(),
            block_deadline: 1000,
            simulation_block: 2000,
        };
        insert(&mut db, input.clone()).await.unwrap();

        let output = fetch(&mut db, 1).await.unwrap().unwrap();
        assert_eq!(input, output);
    }
}
