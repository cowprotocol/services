use crate::{OrderUid, PgTransaction};
use sqlx::PgConnection;

#[derive(Clone, Debug, Default, sqlx::FromRow, PartialEq)]
pub struct EthOrderPlacement {
    pub uid: OrderUid,
    pub valid_to: i64,
    pub is_refunded: bool,
    pub validity_duration: i64,
    pub slippage: f64,
}

pub async fn append(
    ex: &mut PgTransaction<'_>,
    placements: &[EthOrderPlacement],
) -> Result<(), sqlx::Error> {
    for placement in placements {
        insert_ethflow_order(ex, placement).await?;
    }
    Ok(())
}

pub async fn insert_ethflow_order(
    ex: &mut PgConnection,
    placement: &EthOrderPlacement,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = "\
        INSERT INTO ethflow_orders (uid, valid_to, is_refunded, validity_duration, slippage)\
        VALUES ($1, $2, $3, $4, $5) \
        ON CONFLICT (uid) DO UPDATE SET valid_to = $2, is_refunded = $3,\
        validity_duration = $4, slippage = $5;";
    sqlx::query(QUERY)
        .bind(placement.uid)
        .bind(placement.valid_to)
        .bind(placement.is_refunded)
        .bind(placement.validity_duration)
        .bind(placement.slippage)
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

pub async fn refundable_orders(
    ex: &mut PgConnection,
    max_valid_to: i64,
    min_order_validity_duration: i64,
    min_slippage: f64,
) -> Result<Vec<EthOrderPlacement>, sqlx::Error> {
    const QUERY: &str = r#"
        SELECT * FROM ethflow_orders eo
        LEFT JOIN trades t on eo.uid = t.order_uid
        WHERE eo.valid_to < $1
        AND eo.validity_duration >= $2
        AND eo.slippage >= $3
        AND eo.is_refunded = false
        AND t.order_uid is null
    "#;
    sqlx::query_as(QUERY)
        .bind(max_valid_to)
        .bind(min_order_validity_duration)
        .bind(min_slippage)
        .fetch_all(ex)
        .await
}

#[cfg(test)]
mod tests {
    use crate::{
        byte_array::ByteArray,
        events::{insert_trade, EventIndex, Trade},
    };

    use super::*;
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
    async fn postgres_refundable_ethflow_orders() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let order_1 = EthOrderPlacement {
            uid: ByteArray([1u8; 56]),
            valid_to: 1,
            is_refunded: false,
            validity_duration: 3,
            slippage: 0.1f64,
        };
        let order_2 = EthOrderPlacement {
            uid: ByteArray([2u8; 56]),
            valid_to: 2,
            is_refunded: false,
            validity_duration: 4,
            slippage: 0.1f64,
        };
        let order_3 = EthOrderPlacement {
            uid: ByteArray([3u8; 56]),
            valid_to: 3,
            is_refunded: true,
            validity_duration: 5,
            slippage: 0.1f64,
        };
        let order_4 = EthOrderPlacement {
            uid: ByteArray([4u8; 56]),
            valid_to: 10,
            is_refunded: true,
            validity_duration: 11,
            slippage: 0.0001f64,
        };

        append(
            &mut db,
            vec![order_1.clone(), order_2.clone(), order_3, order_4].as_slice(),
        )
        .await
        .unwrap();
        let orders = refundable_orders(&mut db, 3, 3, 0.01f64).await.unwrap();
        assert_eq!(orders, vec![order_1.clone(), order_2]);
        let orders = refundable_orders(&mut db, 2, 3, 0.01f64).await.unwrap();
        assert_eq!(orders, vec![order_1.clone()]);
        let trade = Trade {
            order_uid: ByteArray([2u8; 56]),
            ..Default::default()
        };
        insert_trade(&mut db, &EventIndex::default(), &trade)
            .await
            .unwrap();
        let orders = refundable_orders(&mut db, 3, 3, 0.01f64).await.unwrap();
        assert_eq!(orders, vec![order_1]);
        let orders = refundable_orders(&mut db, 3, 10, 0.01f64).await.unwrap();
        assert_eq!(orders, Vec::new());
    }
}
