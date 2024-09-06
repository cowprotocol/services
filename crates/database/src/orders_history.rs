use {
    crate::{jit_orders, orders, Address},
    futures::stream::BoxStream,
    sqlx::PgConnection,
};

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
"(SELECT ", orders::ORDERS_SELECT,
" FROM ", orders::ORDERS_FROM,
" LEFT OUTER JOIN onchain_placed_orders onchain_o on onchain_o.uid = o.uid",
" WHERE o.owner = $1",
" ORDER BY creation_timestamp DESC LIMIT $2 + $3 ) ",
" UNION ",
" (SELECT ", orders::ORDERS_SELECT,
" FROM ", orders::ORDERS_FROM,
" LEFT OUTER JOIN onchain_placed_orders onchain_o on onchain_o.uid = o.uid",
" WHERE onchain_o.sender = $1 ",
" ORDER BY creation_timestamp DESC LIMIT $2 + $3 ) ",
" UNION ",
" (SELECT ", jit_orders::ORDERS_SELECT,
" FROM jit_orders o",
" WHERE o.owner = $1 AND NOT EXISTS (SELECT 1 FROM orders ord WHERE o.uid = ord.uid)",
" ORDER BY creation_timestamp DESC LIMIT $2 + $3 ) ",
" ORDER BY creation_timestamp DESC ",
" LIMIT $2 ",
" OFFSET $3 ",
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
            onchain_broadcasted_orders::{insert_onchain_order, OnchainOrderPlacement},
        },
        chrono::{DateTime, Utc},
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
}
