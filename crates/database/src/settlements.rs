use sqlx::PgConnection;

use crate::TransactionHash;

pub async fn recent_settlement_tx_hashes(
    ex: &mut PgConnection,
    start_block: i64,
) -> Result<Vec<TransactionHash>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT tx_hash
FROM settlements
WHERE block_number >= $1
    "#;
    sqlx::query_scalar::<_, TransactionHash>(QUERY)
        .bind(start_block)
        .fetch_all(ex)
        .await
}

#[cfg(test)]
mod tests {
    use sqlx::Connection;

    use crate::{
        byte_array::ByteArray,
        events::{Event, EventIndex, Settlement},
    };

    use super::*;

    #[tokio::test]
    #[ignore]
    async fn postgres_gets_event() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        crate::events::append(
            &mut db,
            &[
                (
                    EventIndex {
                        block_number: 0,
                        log_index: 0,
                    },
                    Event::Settlement(Settlement {
                        solver: Default::default(),
                        transaction_hash: ByteArray([0u8; 32]),
                    }),
                ),
                (
                    EventIndex {
                        block_number: 1,
                        log_index: 0,
                    },
                    Event::Settlement(Settlement {
                        solver: Default::default(),
                        transaction_hash: ByteArray([1u8; 32]),
                    }),
                ),
                (
                    EventIndex {
                        block_number: 2,
                        log_index: 0,
                    },
                    Event::Settlement(Settlement {
                        solver: Default::default(),
                        transaction_hash: ByteArray([2u8; 32]),
                    }),
                ),
            ],
        )
        .await
        .unwrap();

        let results = recent_settlement_tx_hashes(&mut db, 1).await.unwrap();
        assert_eq!(results, &[ByteArray([1u8; 32]), ByteArray([2u8; 32])]);

        let results = recent_settlement_tx_hashes(&mut db, 2).await.unwrap();
        assert_eq!(results, &[ByteArray([2u8; 32])]);

        let results = recent_settlement_tx_hashes(&mut db, 3).await.unwrap();
        assert_eq!(results, &[]);
    }
}
