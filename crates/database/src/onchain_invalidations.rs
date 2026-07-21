use {
    crate::{
        OrderUid,
        PgTransaction,
        dedup_keep_last,
        events::EventIndex,
        order_events::{OrderEventLabel, insert_order_events},
    },
    chrono::Utc,
    sqlx::{Executor, PgConnection, QueryBuilder},
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
    const BATCH_SIZE: usize = 5000;
    // Keep the last invalidation per uid so the batched upsert never targets the
    // same order twice in one statement.
    let events = dedup_keep_last(events, |(_, uid)| *uid);
    for chunk in events.chunks(BATCH_SIZE) {
        let mut builder = QueryBuilder::new(
            "INSERT INTO onchain_order_invalidations (block_number, log_index, uid) ",
        );
        builder.push_values(chunk, |mut builder, (index, uid)| {
            builder
                .push_bind(index.block_number)
                .push_bind(index.log_index)
                .push_bind(uid);
        });
        builder.push(
            " ON CONFLICT (uid) DO UPDATE SET block_number = EXCLUDED.block_number, log_index = \
             EXCLUDED.log_index",
        );
        builder.build().execute(ex.deref_mut()).await?;
    }

    // Record a "cancelled" order event for every invalidated order in one shot.
    // `insert_order_events` binds a single `unnest` array (no bind-parameter
    // limit to worry about) and already skips orders whose latest event is
    // already `Cancelled`, matching the previous per-row `insert_order_event`.
    let uids: Vec<OrderUid> = events.iter().map(|(_, uid)| *uid).collect();
    insert_order_events(
        ex,
        &uids,
        // It would be more correct to use the timestamp of the event's block, but passing
        // this is more involved, and now() should be good enough.
        Utc::now(),
        OrderEventLabel::Cancelled,
        None,
    )
    .await?;
    Ok(())
}

#[instrument(skip_all)]
pub async fn delete_invalidations(
    ex: &mut PgTransaction<'_>,
    block_number: i64,
) -> Result<(), sqlx::Error> {
    const QUERY_INVALIDATION: &str =
        "DELETE FROM onchain_order_invalidations WHERE block_number >= $1;";
    ex.execute(sqlx::query(QUERY_INVALIDATION).bind(block_number))
        .await?;
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

    #[tokio::test]
    #[ignore]
    async fn postgres_insert_onchain_invalidations_dedups_and_emits_events() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let uid1: OrderUid = ByteArray([1; 56]);
        let uid2: OrderUid = ByteArray([2; 56]);
        let ev = |block, log| EventIndex {
            block_number: block,
            log_index: log,
        };

        // `uid1` appears twice in one batch; the last occurrence must win and the
        // statement must not error with "cannot affect row a second time".
        insert_onchain_invalidations(
            &mut db,
            &[(ev(1, 0), uid1), (ev(2, 0), uid2), (ev(5, 1), uid1)],
        )
        .await
        .unwrap();

        let row = read_onchain_invalidation(&mut db, &uid1)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.block_number, 5);
        assert_eq!(row.log_index, 1);

        // Exactly one "cancelled" order event per distinct invalidated order.
        for uid in [uid1, uid2] {
            let count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM order_events WHERE order_uid = $1 AND label = 'cancelled'",
            )
            .bind(uid)
            .fetch_one(&mut *db)
            .await
            .unwrap();
            assert_eq!(count, 1);
        }
    }
}
