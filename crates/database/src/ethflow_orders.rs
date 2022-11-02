use crate::{OrderUid, PgTransaction};
use sqlx::{Executor, PgConnection};

#[derive(Clone, Debug, Default, sqlx::FromRow, Eq, PartialEq)]
pub struct EthOrderPlacement {
    pub uid: OrderUid,
    pub valid_to: i64,
    pub is_refunded: bool,
}

pub async fn append(
    ex: &mut PgTransaction<'_>,
    events: &[EthOrderPlacement],
) -> Result<(), sqlx::Error> {
    for event in events {
        insert_ethflow_order(ex, event).await?;
    }
    Ok(())
}

pub async fn insert_ethflow_order(
    ex: &mut PgConnection,
    event: &EthOrderPlacement,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = "\
        INSERT INTO ethflow_orders (uid, valid_to, is_refunded) VALUES ($1, $2, $3) \
        ON CONFLICT (uid) DO UPDATE SET valid_to = $2, is_refunded = $3;";
    sqlx::query(QUERY)
        .bind(event.uid)
        .bind(event.valid_to)
        .bind(event.is_refunded)
        .execute(ex)
        .await?;
    Ok(())
}

pub async fn read_order(
    ex: &mut PgConnection,
    id: &OrderUid,
) -> Result<Option<EthOrderPlacement>, sqlx::Error> {
    const QUERY: &str = r#"
        SELECT * FROM ethflow_orders
        WHERE uid = $1
    "#;
    sqlx::query_as(QUERY).bind(id).fetch_optional(ex).await
}

pub async fn mark_eth_orders_as_refunded(
    ex: &mut PgTransaction<'_>,
    uids: &[OrderUid],
) -> Result<(), sqlx::Error> {
    for uid in uids.iter() {
        mark_eth_order_as_refunded(ex, uid).await?;
    }
    Ok(())
}

pub async fn mark_eth_order_as_refunded(
    ex: &mut PgTransaction<'_>,
    uid: &OrderUid,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
        UPDATE ethflow_orders
        SET is_refunded = true
        WHERE uid = $1
    "#;

    ex.execute(sqlx::query(QUERY).bind(uid)).await?;
    Ok(())
}

pub async fn refundable_orders(
    ex: &mut PgConnection,
    since_valid_to: i64,
    min_validity_duration: i64,
    min_slippage: f64,
) -> Result<Vec<EthOrderPlacement>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT eo.uid, eo.valid_to, eo.is_refunded from orders o 
INNER JOIN ethflow_orders eo on eo.uid = o.uid 
INNER JOIN order_quotes oq on o.uid = oq.order_uid
LEFT JOIN trades t on o.uid = t.order_uid
WHERE 
eo.is_refunded = false
AND t.order_uid is null
AND eo.valid_to < $1
AND o.sell_amount = oq.sell_amount
AND (1.0 - o.buy_amount / oq.buy_amount) > $3
AND eo.valid_to - extract(epoch from creation_timestamp)::int > $2
    "#;
    sqlx::query_as(QUERY)
        .bind(since_valid_to)
        .bind(min_validity_duration)
        .bind(min_slippage)
        .fetch_all(ex)
        .await
}

#[cfg(test)]
mod tests {
    use crate::{
        byte_array::ByteArray,
        events::{insert_trade, EventIndex, Trade},
        orders::{insert_order, insert_quote, Order, Quote},
    };

    use super::*;
    use bigdecimal::BigDecimal;
    use chrono::{TimeZone, Utc};
    use sqlx::Connection;

    #[tokio::test]
    #[ignore]
    async fn postgres_order_roundtrip() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let order = EthOrderPlacement::default();
        insert_ethflow_order(&mut db, &order).await.unwrap();
        let order_ = read_order(&mut db, &order.uid).await.unwrap().unwrap();
        assert_eq!(order, order_);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_insert_eth_flow_order_on_conflict() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let order_1 = EthOrderPlacement::default();
        let order_2 = EthOrderPlacement {
            valid_to: order_1.valid_to + 2,
            ..Default::default()
        };

        append(&mut db, vec![order_1.clone(), order_2.clone()].as_slice())
            .await
            .unwrap();
        let order_ = read_order(&mut db, &order_1.uid).await.unwrap().unwrap();
        assert_eq!(order_2, order_);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_mark_eth_orders_as_refunded() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let order_1 = EthOrderPlacement {
            uid: ByteArray([1u8; 56]),
            valid_to: 1,
            is_refunded: false,
        };
        let order_2 = EthOrderPlacement {
            uid: ByteArray([2u8; 56]),
            valid_to: 2,
            is_refunded: false,
        };

        append(&mut db, vec![order_1.clone(), order_2.clone()].as_slice())
            .await
            .unwrap();
        mark_eth_orders_as_refunded(&mut db, vec![order_1.uid, order_2.uid].as_slice())
            .await
            .unwrap();
        // Check that "is_refunded" was changed
        let order_1 = read_order(&mut db, &order_1.uid).await.unwrap().unwrap();
        assert!(order_1.is_refunded);
        let order_2 = read_order(&mut db, &order_2.uid).await.unwrap().unwrap();
        assert!(order_2.is_refunded);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_mark_eth_order_as_refunded() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let order_1 = EthOrderPlacement {
            uid: ByteArray([1u8; 56]),
            valid_to: 1,
            is_refunded: false,
        };
        let order_2 = EthOrderPlacement {
            uid: ByteArray([2u8; 56]),
            valid_to: 2,
            is_refunded: false,
        };

        append(&mut db, vec![order_1.clone(), order_2.clone()].as_slice())
            .await
            .unwrap();
        mark_eth_order_as_refunded(&mut db, &order_1.uid)
            .await
            .unwrap();
        // Check that "is_refunded" was changed
        let order_1 = read_order(&mut db, &order_1.uid).await.unwrap().unwrap();
        assert!(order_1.is_refunded);
        // Check that other orders are not affected from the change
        let order_2 = read_order(&mut db, &order_2.uid).await.unwrap().unwrap();
        assert!(!order_2.is_refunded);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_refundable_ethflow_orders() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let order_uid_1 = ByteArray([1u8; 56]);
        let eth_order_1 = EthOrderPlacement {
            uid: order_uid_1,
            valid_to: 4,
            is_refunded: false,
        };
        insert_ethflow_order(&mut db, &eth_order_1).await.unwrap();
        let order_1 = Order {
            uid: order_uid_1,
            buy_amount: BigDecimal::from(1),
            sell_amount: BigDecimal::from(100u32),
            creation_timestamp: Utc.timestamp(1, 0),
            ..Default::default()
        };
        insert_order(&mut db, &order_1).await.unwrap();
        let quote_1 = Quote {
            order_uid: order_uid_1,
            buy_amount: BigDecimal::from(2),
            sell_amount: BigDecimal::from(100u32),
            ..Default::default()
        };
        insert_quote(&mut db, &quote_1).await.unwrap();

        // all criteria are fulfilled
        let orders = refundable_orders(&mut db, 5, 1, 0.01).await.unwrap();
        assert_eq!(orders, vec![eth_order_1.clone()]);
        // slippage is not fulfilled
        let orders = refundable_orders(&mut db, 5, 1, 0.53).await.unwrap();
        assert_eq!(orders, Vec::new());
        // min_validity is not fulfilled
        let orders = refundable_orders(&mut db, 1, 1, 0.01).await.unwrap();
        assert_eq!(orders, Vec::new());
        // min_duration is not fulfilled
        let orders = refundable_orders(&mut db, 5, 3, 0.01).await.unwrap();
        assert_eq!(orders, Vec::new());
        // order already settled
        let trade = Trade {
            order_uid: order_uid_1,
            ..Default::default()
        };
        insert_trade(&mut db, &EventIndex::default(), &trade)
            .await
            .unwrap();
        let orders = refundable_orders(&mut db, 5, 1, 0.01).await.unwrap();
        assert_eq!(orders, Vec::new());
        let order_uid_2 = ByteArray([2u8; 56]);
        let eth_order_2 = EthOrderPlacement {
            uid: order_uid_2,
            valid_to: 4,
            is_refunded: true, // <-- sole change in setup
        };
        insert_ethflow_order(&mut db, &eth_order_2).await.unwrap();
        let order_2 = Order {
            uid: order_uid_2,
            buy_amount: BigDecimal::from(1),
            sell_amount: BigDecimal::from(100u32),
            creation_timestamp: Utc.timestamp(1, 0),
            ..Default::default()
        };
        insert_order(&mut db, &order_2).await.unwrap();
        let quote_2 = Quote {
            order_uid: order_uid_2,
            buy_amount: BigDecimal::from(2),
            sell_amount: BigDecimal::from(100u32),
            ..Default::default()
        };
        insert_quote(&mut db, &quote_2).await.unwrap();
        // is_refunded is not fulfilled
        let orders = refundable_orders(&mut db, 5, 1, 0.01).await.unwrap();
        assert_eq!(orders, Vec::new());
        let order_uid_3 = ByteArray([3u8; 56]);
        let eth_order_3 = EthOrderPlacement {
            uid: order_uid_3,
            valid_to: 4,
            is_refunded: false,
        };
        insert_ethflow_order(&mut db, &eth_order_3).await.unwrap();
        let order_3 = Order {
            uid: order_uid_3,
            buy_amount: BigDecimal::from(1),
            sell_amount: BigDecimal::from(99u32), // Single change here
            creation_timestamp: Utc.timestamp(1, 0),
            ..Default::default()
        };
        insert_order(&mut db, &order_3).await.unwrap();
        let quote_3 = Quote {
            order_uid: order_uid_3,
            buy_amount: BigDecimal::from(2),
            sell_amount: BigDecimal::from(100u32),
            ..Default::default()
        };
        insert_quote(&mut db, &quote_3).await.unwrap();
        // sell_amount is not fulfilled
        let orders = refundable_orders(&mut db, 5, 1, 0.01).await.unwrap();
        assert_eq!(orders, Vec::new());
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_refundable_orders_performance() {
        // The following test can be used as performance test,
        // if the limit is set to->100000u32, the query should still finish
        // below 13 ms
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let limit = 10u32;
        for i in 0..limit {
            let mut owner_bytes = i.to_ne_bytes().to_vec();
            owner_bytes.append(&mut vec![0; 20 - owner_bytes.len()]);
            let owner = ByteArray(owner_bytes.try_into().unwrap());
            let mut i_as_bytes = i.to_ne_bytes().to_vec();
            let mut order_uid_info = vec![0; 56 - i_as_bytes.len()];
            i_as_bytes.append(&mut order_uid_info);
            let order_uid = ByteArray(i_as_bytes.try_into().unwrap());
            let trade = Trade {
                order_uid,
                ..Default::default()
            };
            // for 3/4 of the orders, we assume that they are actually settling
            if i > limit / 4 * 3 {
                let event_index = EventIndex::default();
                insert_trade(&mut db, &event_index, &trade).await.unwrap();
            }
            let ethflow_order = EthOrderPlacement {
                uid: order_uid,
                valid_to: i as i64,
                is_refunded: (i % 10u32 == 0),
            };
            insert_ethflow_order(&mut db, &ethflow_order).await.unwrap();
            let order = Order {
                uid: order_uid,
                owner,
                creation_timestamp: Utc::now(),
                buy_amount: BigDecimal::from(100u32),
                sell_amount: BigDecimal::from(100u32),
                ..Default::default()
            };
            insert_order(&mut db, &order).await.unwrap();
            let quote = Quote {
                order_uid,
                buy_amount: BigDecimal::from(100u32 - i % 3),
                sell_amount: BigDecimal::from(100u32),
                ..Default::default()
            };
            insert_quote(&mut db, &quote).await.unwrap();
        }

        let now = std::time::Instant::now();
        refundable_orders(&mut db, 1, 1, 1.0).await.unwrap();
        let elapsed = now.elapsed();
        println!("{:?}", elapsed);
        assert!(elapsed < std::time::Duration::from_secs(1));
    }
}
