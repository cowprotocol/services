use {
    crate::{auction::AuctionId, Address, PgTransaction},
    futures::stream::BoxStream,
    sqlx::{PgConnection, QueryBuilder},
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
    const BATCH_SIZE: usize = 5000;
    const QUERY: &str = "INSERT INTO auction_participants (auction_id, participant) ";

    for chunk in participants.chunks(BATCH_SIZE) {
        let mut query_builder = QueryBuilder::new(QUERY);

        query_builder.push_values(chunk, |mut builder, participant| {
            builder
                .push_bind(participant.auction_id)
                .push_bind(participant.participant);
        });

        query_builder.build().execute(ex.deref_mut()).await?;
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

#[derive(sqlx::FromRow)]
pub struct ParticipantAddress(pub Address);

pub fn latest_auction_participants(
    ex: &mut PgConnection,
) -> BoxStream<'_, Result<ParticipantAddress, sqlx::Error>> {
    const QUERY: &str = r#"
SELECT participant
FROM auction_participants
WHERE auction_id = (
    SELECT MAX(auction_id)
    FROM auction_participants
)
    "#;

    sqlx::query_as(QUERY).fetch(ex)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::byte_array::ByteArray,
        futures::TryStreamExt,
        maplit::hashset,
        sqlx::Connection,
        std::collections::HashSet,
    };

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
            Participant {
                auction_id: 2,
                participant: ByteArray([4; 20]),
            },
            Participant {
                auction_id: 2,
                participant: ByteArray([5; 20]),
            },
        ];
        insert(&mut db, &input).await.unwrap();
        let output = fetch(&mut db, 1).await.unwrap();
        assert_eq!(vec![input[0].clone(), input[1].clone()], output);
        let output = fetch(&mut db, 2).await.unwrap();
        assert_eq!(vec![input[2].clone(), input[3].clone()], output);
        let output: HashSet<_> = latest_auction_participants(&mut db)
            .map_ok(|p| p.0)
            .try_collect()
            .await
            .unwrap();
        assert_eq!(hashset![input[2].participant, input[3].participant], output);
    }
}
