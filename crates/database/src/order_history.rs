use {
    crate::{Address, jit_orders, orders},
    futures::stream::BoxStream,
    sqlx::PgConnection,
    tracing::instrument,
};

#[instrument(skip_all)]
pub fn user_orders<'a>(
    ex: &'a mut PgConnection,
    owner: &'a Address,
    offset: i64,
    limit: Option<i64>,
) -> BoxStream<'a, Result<orders::FullOrder, sqlx::Error>> {
    // As a future consideration for this query we could move from offset to an
    // approach called keyset pagination where the offset is identified by "key"
    // of the previous query. In our case that would be the lowest
    // creation_timestamp. This way the database can start immediately at the
    // offset through the index without enumerating the first N elements
    // before as is the case with OFFSET.
    // On the other hand that approach is less flexible so we will consider if we
    // see that these queries are taking too long in practice.
    #[rustfmt::skip]
    const QUERY: &str = const_format::concatcp!(
        // Phase 1: find the page of UIDs using cheap index scans.
        // Because we `UNION` the sub-query results before determining the
        // window requested by the user we need to fetch LIMIT + OFFSET results
        // in each sub-query.
        "WITH page_uids AS (",
            " SELECT uid, min(creation_timestamp) as creation_timestamp FROM (",
                // regular orders with that owner (relies on the
                // `user_order_creation_timestamp` index)
                " (",
                "  SELECT o.uid, o.creation_timestamp",
                "  FROM orders o",
                "  WHERE o.owner = $1",
                "  ORDER BY creation_timestamp DESC",
                "  LIMIT $2 + $3",
                " )",
                " UNION ALL",
                // onchain placed orders from that sender - gets a dedicated
                // subquery to avoid having to needlessly LEFT JOIN potentially
                // thousands of onchain_placed_orders on orders because those
                // are relatively rare
                //
                // relies on the `order_sender` index
                " (",
                "  SELECT o.uid, o.creation_timestamp",
                "  FROM onchain_placed_orders opo",
                "  JOIN orders o ON opo.uid = o.uid",
                "  WHERE opo.sender = $1 AND o.owner != $1",
                "  ORDER BY creation_timestamp DESC",
                "  LIMIT $2 + $3",
                " )",
                " UNION ALL",
                // JIT orders with that owner (relies on the
                // `jit_order_creation_timestamp` index)
                " (",
                "  SELECT jit_o.uid, jit_o.creation_timestamp",
                "  FROM jit_orders jit_o",
                "  WHERE jit_o.owner = $1",
                "  ORDER BY creation_timestamp DESC",
                "  LIMIT $2 + $3",
                " )",
            " ) combined",
            " GROUP BY uid",
            " ORDER BY creation_timestamp DESC",
            " LIMIT $2 OFFSET $3",
        ") ",
        // Phase 2: fetch full rows for the relevant UIDs only
        " (",
        "  SELECT ", orders::SELECT, crate::trades::ORDER_GAS_COST_COLUMN,
        "  FROM ", orders::FROM,
        "  WHERE o.uid IN (SELECT uid FROM page_uids)",
        " )",
        " UNION ALL",
        " (",
        "  SELECT ", jit_orders::SELECT, crate::trades::ORDER_GAS_COST_COLUMN,
        "  FROM ", jit_orders::FROM,
        "  WHERE o.uid IN (SELECT uid FROM page_uids)",
        // despite already handling duplicates in phase 1 we need to handle
        // them here again. Because JIT orders are very rare we check that
        // the order does not exist in the regular orders table instead of the
        // other way around.
        "    AND NOT EXISTS (SELECT 1 FROM orders ord WHERE o.uid = ord.uid)",
        " )",
        " ORDER BY creation_timestamp DESC",
    );
    sqlx::query_as(QUERY)
        .bind(owner)
        .bind(limit)
        .bind(offset)
        .fetch(ex)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            byte_array::ByteArray,
            events::EventIndex,
            onchain_broadcasted_orders::{OnchainOrderPlacement, insert_onchain_order},
        },
        chrono::{DateTime, Duration, Utc},
        futures::StreamExt,
        sqlx::Connection,
    };

    type Data = ([u8; 56], Address, DateTime<Utc>);
    async fn user_orders(
        ex: &mut PgConnection,
        owner: &Address,
        offset: i64,
        limit: Option<i64>,
    ) -> Vec<Data> {
        super::user_orders(ex, owner, offset, limit)
            .map(|o| {
                let o = o.unwrap();
                (o.uid.0, o.owner, o.creation_timestamp)
            })
            .collect::<Vec<_>>()
            .await
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_user_orders_performance_many_users_with_some_orders() {
        // The following test can be used as performance test,
        // if the values for i and j are increased ->i=100
        // and j=1000 the query should still 10 ms
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        for i in 0..1u32 {
            let mut owner_bytes = i.to_ne_bytes().to_vec();
            owner_bytes.append(&mut vec![0; 20 - owner_bytes.len()]);
            let owner = ByteArray(owner_bytes.try_into().unwrap());
            for j in 0..10u32 {
                let mut i_as_bytes = i.to_ne_bytes().to_vec();
                let mut j_as_bytes = j.to_ne_bytes().to_vec();
                let mut order_uid_info = vec![0; 56 - i_as_bytes.len() - j_as_bytes.len()];
                order_uid_info.append(&mut j_as_bytes);
                i_as_bytes.append(&mut order_uid_info);
                let uid = ByteArray(i_as_bytes.try_into().unwrap());
                let order = orders::Order {
                    owner,
                    uid,
                    creation_timestamp: Utc::now(),
                    ..Default::default()
                };
                orders::insert_order(&mut db, &order).await.unwrap();
                if j % 10 == 0 {
                    let onchain_order = OnchainOrderPlacement {
                        order_uid: uid,
                        sender: owner,
                        placement_error: None,
                    };
                    let event_index = EventIndex::default();
                    insert_onchain_order(&mut db, &event_index, &onchain_order)
                        .await
                        .unwrap();
                }
            }
        }

        let now = std::time::Instant::now();
        let number_of_query_executions = 100;
        for _ in 0..number_of_query_executions {
            let _result = user_orders(&mut db, &ByteArray([2u8; 20]), 10, Some(10)).await;
        }
        let elapsed = now.elapsed();
        println!(
            "Time per execution {:?}",
            elapsed / number_of_query_executions
        );
        assert!(elapsed / number_of_query_executions < std::time::Duration::from_secs(1));
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_user_orders_performance_user_with_many_orders() {
        // The following test can be used as performance test close to prod env,
        // if the values for j increased ->j=100_000 query should still finish
        // below 200 ms
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        for i in 0..1u32 {
            let mut owner_bytes = i.to_ne_bytes().to_vec();
            owner_bytes.append(&mut vec![0; 20 - owner_bytes.len()]);
            let owner = ByteArray(owner_bytes.try_into().unwrap());
            for j in 0..10u32 {
                let mut i_as_bytes = i.to_ne_bytes().to_vec();
                let mut j_as_bytes = j.to_ne_bytes().to_vec();
                let mut order_uid_info = vec![0; 56 - i_as_bytes.len() - j_as_bytes.len()];
                order_uid_info.append(&mut j_as_bytes);
                i_as_bytes.append(&mut order_uid_info);
                let order = orders::Order {
                    owner,
                    uid: ByteArray(i_as_bytes.try_into().unwrap()),
                    creation_timestamp: Utc::now(),
                    ..Default::default()
                };
                orders::insert_order(&mut db, &order).await.unwrap();
            }
        }

        let now = std::time::Instant::now();
        let number_of_query_executions = 100;
        for _ in 0..number_of_query_executions {
            let _result = user_orders(&mut db, &ByteArray([0u8; 20]), 10, Some(10)).await;
        }
        let elapsed = now.elapsed();
        println!(
            "Time per execution {:?}",
            elapsed / number_of_query_executions
        );
        assert!(elapsed / number_of_query_executions < std::time::Duration::from_secs(1));
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_user_orders_correctness() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let owner = ByteArray([1u8; 20]);
        let other_owner = ByteArray([2u8; 20]);
        let now = Utc::now();
        let t = |secs: i64| now - Duration::seconds(secs);

        // Ordered newest→oldest: uid_a, uid_b, uid_c, uid_d, uid_e
        let uid_a = ByteArray([0xaau8; 56]); // orders only
        let uid_b = ByteArray([0xbbu8; 56]); // both orders AND jit_orders — must appear once
        let uid_c = ByteArray([0xccu8; 56]); // jit_orders only
        let uid_d = ByteArray([0xddu8; 56]); // orders only
        let uid_e = ByteArray([0xeeu8; 56]); // orders, owned by other_owner, sender = owner
        let uid_x = ByteArray([0xffu8; 56]); // other_owner — must never appear

        orders::insert_order(
            &mut db,
            &orders::Order {
                uid: uid_a,
                owner,
                creation_timestamp: t(1),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        orders::insert_order(
            &mut db,
            &orders::Order {
                uid: uid_b,
                owner,
                creation_timestamp: t(2),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        // jit_orders primary key is (block_number, log_index) — use distinct values.
        jit_orders::insert(
            &mut db,
            &[
                jit_orders::JitOrder {
                    uid: uid_b,
                    owner,
                    creation_timestamp: t(2),
                    block_number: 1,
                    ..Default::default()
                },
                jit_orders::JitOrder {
                    uid: uid_c,
                    owner,
                    creation_timestamp: t(3),
                    block_number: 2,
                    ..Default::default()
                },
            ],
        )
        .await
        .unwrap();

        orders::insert_order(
            &mut db,
            &orders::Order {
                uid: uid_d,
                owner,
                creation_timestamp: t(4),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        orders::insert_order(
            &mut db,
            &orders::Order {
                uid: uid_e,
                owner: other_owner,
                creation_timestamp: t(5),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        insert_onchain_order(
            &mut db,
            &EventIndex::default(),
            &OnchainOrderPlacement {
                order_uid: uid_e,
                sender: owner,
                placement_error: None,
            },
        )
        .await
        .unwrap();

        orders::insert_order(
            &mut db,
            &orders::Order {
                uid: uid_x,
                owner: other_owner,
                creation_timestamp: t(0),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let uids_of = |v: Vec<Data>| v.into_iter().map(|(uid, _, _)| uid).collect::<Vec<_>>();

        // All results in order.
        let all = uids_of(user_orders(&mut db, &owner, 0, Some(100)).await);
        assert_eq!(all, vec![uid_a.0, uid_b.0, uid_c.0, uid_d.0, uid_e.0]);

        // uid_b is in both tables but must appear exactly once.
        assert_eq!(all.iter().filter(|&&u| u == uid_b.0).count(), 1);

        // First page: both rows come from orders.
        let page1 = uids_of(user_orders(&mut db, &owner, 0, Some(2)).await);
        assert_eq!(page1, vec![uid_a.0, uid_b.0]);

        // Second page: crosses the table boundary — uid_c from jit_orders, uid_d from
        // orders.
        let page2 = uids_of(user_orders(&mut db, &owner, 2, Some(2)).await);
        assert_eq!(page2, vec![uid_c.0, uid_d.0]);

        // Last page: onchain-sender order.
        let page3 = uids_of(user_orders(&mut db, &owner, 4, Some(10)).await);
        assert_eq!(page3, vec![uid_e.0]);

        // Unrelated address returns nothing.
        let none = user_orders(&mut db, &ByteArray([0xabu8; 20]), 0, Some(100)).await;
        assert!(none.is_empty());
    }
}
