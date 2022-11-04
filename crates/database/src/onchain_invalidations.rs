use crate::{events::EventIndex, OrderUid, PgTransaction};
use sqlx::{Executor, PgConnection};

#[derive(Clone, Debug, Default, sqlx::FromRow, Eq, PartialEq)]
pub struct OnchainCancellationRow {
    pub uid: OrderUid,
    pub is_reorged: bool,
    pub block_number: i64,
    pub log_index: i64,
}

pub async fn append(
    ex: &mut PgTransaction<'_>,
    events: &[(EventIndex, OrderUid)],
) -> Result<(), sqlx::Error> {
    for (index, event) in events {
        insert_onchain_invalidation(ex, index, event).await?;
    }
    Ok(())
}

pub async fn mark_as_reorged(
    ex: &mut PgTransaction<'_>,
    mark_from_block_number: i64,
) -> Result<(), sqlx::Error> {
    const QUERY_ONCHAIN_ORDERS: &str =
        "UPDATE onchain_order_cancellations SET is_reorged = true WHERE block_number >= $1;";
    ex.execute(sqlx::query(QUERY_ONCHAIN_ORDERS).bind(mark_from_block_number))
        .await?;
    Ok(())
}

pub async fn insert_onchain_invalidation(
    ex: &mut PgConnection,
    index: &EventIndex,
    order_uid: &OrderUid,
) -> Result<(), sqlx::Error> {
    const QUERY: &str =
        "INSERT INTO onchain_order_cancellations (block_number, log_index, uid, is_reorged) VALUES ($1, $2, $3, false) \
        ON CONFLICT (uid) DO UPDATE SET
            is_reorged = false, block_number = $1, log_index = $2;
    ;";
    sqlx::query(QUERY)
        .bind(index.block_number)
        .bind(index.log_index)
        .bind(order_uid)
        .execute(ex)
        .await?;
    Ok(())
}

pub async fn read_onchain_invalidation(
    ex: &mut PgConnection,
    id: &OrderUid,
) -> Result<Option<OnchainCancellationRow>, sqlx::Error> {
    const QUERY: &str = r#"
        SELECT * FROM onchain_order_cancellations
        WHERE uid = $1
    "#;
    sqlx::query_as(QUERY).bind(id).fetch_optional(ex).await
}

#[cfg(test)]
mod tests {
    use crate::byte_array::ByteArray;

    use super::*;
    use sqlx::Connection;

    #[tokio::test]
    #[ignore]
    async fn postgres_invalidation_roundtrip() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let order_uid = OrderUid::default();
        let event_index = EventIndex::default();
        insert_onchain_invalidation(&mut db, &event_index, &order_uid)
            .await
            .unwrap();
        let row = read_onchain_invalidation(&mut db, &order_uid)
            .await
            .unwrap()
            .unwrap();
        let expected_row = OnchainCancellationRow {
            uid: order_uid,
            is_reorged: false,
            block_number: event_index.block_number,
            log_index: event_index.log_index,
        };
        assert_eq!(expected_row, row);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_sets_is_reorged_to_true() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let event_index_1 = EventIndex {
            block_number: 1,
            log_index: 0,
        };
        let event_index_2 = EventIndex {
            block_number: 2,
            log_index: 0,
        };

        let order_uid_1: OrderUid = ByteArray([1; 56]);
        let order_uid_2: OrderUid = ByteArray([2; 56]);
        append(
            &mut db,
            &[(event_index_1, order_uid_1), (event_index_2, order_uid_2)],
        )
        .await
        .unwrap();
        mark_as_reorged(&mut db, 2).await.unwrap();
        let row = read_onchain_invalidation(&mut db, &order_uid_1)
            .await
            .unwrap()
            .unwrap();
        let expected_row = OnchainCancellationRow {
            uid: order_uid_1,
            is_reorged: false,
            block_number: event_index_1.block_number,
            log_index: event_index_1.log_index,
        };
        assert_eq!(expected_row, row);
        let row = read_onchain_invalidation(&mut db, &order_uid_2)
            .await
            .unwrap()
            .unwrap();
        let expected_row = OnchainCancellationRow {
            uid: order_uid_2,
            is_reorged: true, // <-- difference is here
            block_number: event_index_2.block_number,
            log_index: event_index_2.log_index,
        };
        assert_eq!(expected_row, row);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_insert_order_conflict_handling() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let event_index_1 = EventIndex {
            block_number: 1,
            log_index: 0,
        };
        let event_index_2 = EventIndex {
            block_number: 2,
            log_index: 1,
        };
        let order_uid = ByteArray([1; 56]);
        append(&mut db, &[(event_index_1, order_uid)])
            .await
            .unwrap();
        mark_as_reorged(&mut db, 1).await.unwrap();
        let row = read_onchain_invalidation(&mut db, &order_uid)
            .await
            .unwrap()
            .unwrap();
        let expected_row = OnchainCancellationRow {
            uid: order_uid,
            is_reorged: true,
            block_number: event_index_1.block_number,
            log_index: event_index_1.log_index,
        };
        assert_eq!(expected_row, row);
        let reorged_order = order_uid;
        // Now, we insert the order again and then it should no longer be reorged
        append(&mut db, &[(event_index_2, reorged_order)])
            .await
            .unwrap();
        let row = read_onchain_invalidation(&mut db, &order_uid)
            .await
            .unwrap()
            .unwrap();
        let expected_row = OnchainCancellationRow {
            uid: order_uid,
            is_reorged: false,
            block_number: event_index_2.block_number,
            log_index: event_index_2.log_index,
        };
        assert_eq!(expected_row, row);
    }
}
