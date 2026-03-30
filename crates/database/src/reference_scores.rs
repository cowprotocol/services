use {
    crate::{Address, PgTransaction, auction::AuctionId},
    bigdecimal::BigDecimal,
    sqlx::{PgConnection, QueryBuilder},
    std::ops::DerefMut,
    tracing::instrument,
};

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct Score {
    pub auction_id: AuctionId,
    pub solver: Address,
    pub reference_score: BigDecimal,
}

#[instrument(skip_all)]
pub async fn insert(ex: &mut PgTransaction<'_>, scores: &[Score]) -> Result<(), sqlx::Error> {
    const QUERY: &str = "INSERT INTO reference_scores (auction_id, solver, reference_score) ";

    if scores.is_empty() {
        return Ok(());
    }

    let mut query_builder = QueryBuilder::new(QUERY);
    query_builder.push_values(scores, |mut builder, score| {
        builder
            .push_bind(score.auction_id)
            .push_bind(score.solver)
            .push_bind(score.reference_score.clone());
    });

    query_builder.build().execute(ex.deref_mut()).await?;

    Ok(())
}

#[instrument(skip_all)]
pub async fn fetch(
    ex: &mut PgConnection,
    auction_id: AuctionId,
) -> Result<Vec<Score>, sqlx::Error> {
    const QUERY: &str = r#"SELECT * FROM reference_scores WHERE auction_id = $1"#;
    sqlx::query_as(QUERY).bind(auction_id).fetch_all(ex).await
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

        let input = vec![];
        insert(&mut db, &input).await.unwrap();
        let output = fetch(&mut db, 1).await.unwrap();
        assert!(output.is_empty());

        let input = vec![
            Score {
                auction_id: 1,
                solver: ByteArray([2; 20]),
                reference_score: 9.into(),
            },
            Score {
                auction_id: 1,
                solver: ByteArray([3; 20]),
                reference_score: 10.into(),
            },
        ];
        insert(&mut db, &input).await.unwrap();

        let output = fetch(&mut db, 1).await.unwrap();
        assert!(output.contains(&input[0]));
        assert!(output.contains(&input[1]));
    }
}
