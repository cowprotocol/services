use {
    crate::{auction::AuctionId, Address},
    sqlx::PgConnection,
};

/// Participants of a solver competition for a given auction.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct Participants {
    pub auction_id: AuctionId,
    pub participants: Vec<Address>,
}

pub async fn insert(ex: &mut PgConnection, data: Participants) -> Result<(), sqlx::Error> {
    const QUERY: &str =
        r#"INSERT INTO auction_participants (auction_id, participants) VALUES ($1, $2);"#;
    sqlx::query(QUERY)
        .bind(data.auction_id)
        .bind(data.participants)
        .execute(ex)
        .await?;
    Ok(())
}

pub async fn fetch(
    ex: &mut PgConnection,
    auction_id: AuctionId,
) -> Result<Option<Participants>, sqlx::Error> {
    const QUERY: &str = r#"SELECT * FROM auction_participants WHERE auction_id = $1"#;
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

        let data = Participants {
            auction_id: 1,
            participants: (0u8..3).map(|i| ByteArray([i; 20])).collect::<Vec<_>>(),
        };
        insert(&mut db, data.clone()).await.unwrap();

        let result = fetch(&mut db, 1).await.unwrap().unwrap();
        assert_eq!(result, data);
    }
}
