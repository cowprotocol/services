use {
    crate::{
        OrderUid,
        PgTransaction,
        events::EventIndex,
        order_events::{OrderEvent, OrderEventLabel, insert_order_event},
    },
    chrono::Utc,
    sqlx::PgConnection,
    std::ops::DerefMut,
    tracing::instrument,
};

#[derive(Clone, Debug, Default, sqlx::FromRow, Eq, PartialEq)]
pub struct OnchainInvalidationRow {
    pub uid: OrderUid,
    pub block_number: i64,
    pub log_index: i64,
}

#[instrument(skip_all)]
pub async fn insert_onchain_invalidations(
    ex: &mut PgTransaction<'_>,
    events: &[(EventIndex, OrderUid)],
) -> Result<(), sqlx::Error> {
    for (index, event) in events {
        insert_onchain_invalidation(ex, index, event).await?;
        insert_order_event(
            ex,
            &OrderEvent {
                label: OrderEventLabel::Cancelled,
                // It would be more correct to use the timestamp of the event's
                // block, but passing this is more involved, and now() should be
                // good enough.
                timestamp: Utc::now(),
                order_uid: *event,
                reason: None,
            },
        )
        .await?;
    }
    Ok(())
}

#[instrument(skip_all)]
pub async fn delete_invalidations(
    ex: &mut PgTransaction<'_>,
    block_number: i64,
) -> Result<(), sqlx::Error> {
    const QUERY_INVALIDATION: &str =
        "DELETE FROM onchain_order_invalidations WHERE block_number >= $1 RETURNING uid;";
    let deleted_uids: Vec<OrderUid> = sqlx::query_scalar(QUERY_INVALIDATION)
        .bind(block_number)
        .fetch_all(ex.deref_mut())
        .await?;

    if !deleted_uids.is_empty() {
        const DELETE_ORDER_EVENTS: &str = r#"
            DELETE FROM order_events
            WHERE order_uid = ANY($1)
              AND label = 'cancelled'
              AND NOT EXISTS (
                  SELECT 1 FROM orders
                  WHERE orders.uid = order_events.order_uid
                    AND orders.cancellation_timestamp IS NOT NULL
              );
        "#;
        sqlx::query(DELETE_ORDER_EVENTS)
            .bind(&deleted_uids)
            .execute(ex.deref_mut())
            .await?;
    }

    Ok(())
}

#[instrument(skip_all)]
pub async fn insert_onchain_invalidation(
    ex: &mut PgConnection,
    index: &EventIndex,
    order_uid: &OrderUid,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = "INSERT INTO onchain_order_invalidations (block_number, log_index, uid) \
                         VALUES ($1, $2, $3) ON CONFLICT (uid) DO UPDATE SET
         block_number = $1, log_index = $2;
    ;";
    sqlx::query(QUERY)
        .bind(index.block_number)
        .bind(index.log_index)
        .bind(order_uid)
        .execute(ex)
        .await?;
    Ok(())
}

#[instrument(skip_all)]
pub async fn read_onchain_invalidation(
    ex: &mut PgConnection,
    id: &OrderUid,
) -> Result<Option<OnchainInvalidationRow>, sqlx::Error> {
    const QUERY: &str = r#"
        SELECT * FROM onchain_order_invalidations
        WHERE uid = $1
    "#;
    sqlx::query_as(QUERY).bind(id).fetch_optional(ex).await
}

#[cfg(test)]
mod tests {
    use {super::*, crate::byte_array::ByteArray, sqlx::Connection};

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
        let expected_row = OnchainInvalidationRow {
            uid: order_uid,
            block_number: event_index.block_number,
            log_index: event_index.log_index,
        };
        assert_eq!(expected_row, row);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_delete_invalidations() {
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
        insert_onchain_invalidations(
            &mut db,
            &[(event_index_1, order_uid_1), (event_index_2, order_uid_2)],
        )
        .await
        .unwrap();
        delete_invalidations(&mut db, 2).await.unwrap();
        let row = read_onchain_invalidation(&mut db, &order_uid_1)
            .await
            .unwrap()
            .unwrap();
        let expected_row = OnchainInvalidationRow {
            uid: order_uid_1,
            block_number: event_index_1.block_number,
            log_index: event_index_1.log_index,
        };
        assert_eq!(expected_row, row);
        let row = read_onchain_invalidation(&mut db, &order_uid_2)
            .await
            .unwrap();
        assert_eq!(None, row);

        let event_1 = crate::order_events::get_latest(&mut db, &order_uid_1)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(event_1.label, OrderEventLabel::Cancelled);

        let event_2 = crate::order_events::get_latest(&mut db, &order_uid_2)
            .await
            .unwrap();
        assert_eq!(None, event_2);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_reorg_invalidation_restores_prior_order_event_status() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let order_uid: OrderUid = ByteArray([3; 56]);
        let event_index = EventIndex {
            block_number: 5,
            log_index: 0,
        };

        insert_order_event(
            &mut db,
            &OrderEvent {
                label: OrderEventLabel::Created,
                timestamp: Utc::now(),
                order_uid,
                reason: None,
            },
        )
        .await
        .unwrap();

        let latest = crate::order_events::get_latest(&mut db, &order_uid)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(latest.label, OrderEventLabel::Created);

        insert_onchain_invalidations(&mut db, &[(event_index, order_uid)])
            .await
            .unwrap();

        let latest = crate::order_events::get_latest(&mut db, &order_uid)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(latest.label, OrderEventLabel::Cancelled);

        delete_invalidations(&mut db, 5).await.unwrap();

        assert_eq!(read_onchain_invalidation(&mut db, &order_uid).await.unwrap(), None);
        let restored = crate::order_events::get_latest(&mut db, &order_uid)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(restored.label, OrderEventLabel::Created);
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
        insert_onchain_invalidations(&mut db, &[(event_index_1, order_uid)])
            .await
            .unwrap();
        let reorged_order = order_uid;
        // Now, we insert the order again
        insert_onchain_invalidations(&mut db, &[(event_index_2, reorged_order)])
            .await
            .unwrap();
        let row = read_onchain_invalidation(&mut db, &order_uid)
            .await
            .unwrap()
            .unwrap();
        let expected_row = OnchainInvalidationRow {
            uid: order_uid,
            block_number: event_index_2.block_number,
            log_index: event_index_2.log_index,
        };
        assert_eq!(expected_row, row);
    }
}
