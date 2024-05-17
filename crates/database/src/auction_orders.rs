use {
    crate::{auction::AuctionId, OrderUid, PgTransaction},
    sqlx::PgConnection,
    std::ops::DerefMut,
};

/// List of included orders for a given auction.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct AuctionOrder {
    pub auction_id: AuctionId,
    pub order_uid: OrderUid,
}

pub async fn insert(
    ex: &mut PgTransaction<'_>,
    orders: &[AuctionOrder],
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"INSERT INTO auction_orders (auction_id, order_uid) VALUES ($1, $2);"#;
    for price in orders {
        sqlx::query(QUERY)
            .bind(price.auction_id)
            .bind(price.order_uid)
            .execute(ex.deref_mut())
            .await?;
    }
    Ok(())
}

pub async fn fetch(
    ex: &mut PgConnection,
    auction_id: AuctionId,
) -> Result<Vec<AuctionOrder>, sqlx::Error> {
    const QUERY: &str = "SELECT * FROM auction_orders WHERE auction_id = $1";
    let orders = sqlx::query_as(QUERY).bind(auction_id).fetch_all(ex).await?;
    Ok(orders)
}

#[cfg(test)]
mod tests {
    use {super::*, crate::byte_array::ByteArray, sqlx::Connection};

    #[tokio::test]
    #[ignore]
    async fn postgres_roundtrip() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let auction = vec![
            AuctionOrder {
                auction_id: 1,
                order_uid: ByteArray([1; 56]),
            },
            AuctionOrder {
                auction_id: 1,
                order_uid: ByteArray([2; 56]),
            },
        ];

        insert(&mut db, &auction).await.unwrap();

        let output = fetch(&mut db, 1).await.unwrap();
        assert_eq!(output, auction);

        // non-existent auction
        let output = fetch(&mut db, 2).await.unwrap();
        assert!(output.is_empty());
    }
}
