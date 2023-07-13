use {
    crate::{auction::AuctionId, Address, PgTransaction},
    sqlx::PgConnection,
    std::ops::DerefMut,
};

/// Participant of a solver competition for a given auction.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct Participant {
    pub auction_id: AuctionId,
    pub participant: Address,
}

pub async fn insert(
    ex: &mut PgTransaction<'_>,
    participants: &[Participant],
) -> Result<(), sqlx::Error> {
    const QUERY: &str =
        r#"INSERT INTO auction_participants (auction_id, participant) VALUES ($1, $2);"#;
    for participant in participants {
        sqlx::query(QUERY)
            .bind(participant.auction_id)
            .bind(participant.participant)
            .execute(ex.deref_mut())
            .await?;
    }
    Ok(())
}

pub async fn fetch(
    ex: &mut PgConnection,
    auction_id: AuctionId,
) -> Result<Vec<Participant>, sqlx::Error> {
    const QUERY: &str = r#"SELECT * FROM auction_participants WHERE auction_id = $1"#;
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

        let input = vec![
            Participant {
                auction_id: 1,
                participant: ByteArray([2; 20]),
            },
            Participant {
                auction_id: 1,
                participant: ByteArray([3; 20]),
            },
        ];
        insert(&mut db, &input).await.unwrap();
        let output = fetch(&mut db, 1).await.unwrap();
        assert_eq!(input, output);
    }
}
