use crate::{OrderUid, PgTransaction};
use sqlx::PgConnection;

#[derive(Clone, Debug, Default, sqlx::FromRow, Eq, PartialEq)]
pub struct EthOrderPlacement {
    pub uid: OrderUid,
    pub valid_to: i64,
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

async fn insert_ethflow_order(
    ex: &mut PgConnection,
    event: &EthOrderPlacement,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = "\
        INSERT INTO ethflow_orders (uid, valid_to) VALUES ($1, $2) \
        ON CONFLICT (uid) DO UPDATE SET valid_to = $2;";
    sqlx::query(QUERY)
        .bind(event.uid)
        .bind(event.valid_to)
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

#[cfg(test)]
mod tests {
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
}
