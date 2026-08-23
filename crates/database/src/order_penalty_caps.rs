use {
    crate::{OrderUid, auction::AuctionId},
    bigdecimal::BigDecimal,
    sqlx::{PgConnection, QueryBuilder},
    tracing::instrument,
};

/// Cap on the penalty a solver can incur for winning an order but failing to
/// execute it, denominated in the native token (CIP-87).
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct OrderPenaltyCap {
    pub auction_id: AuctionId,
    pub order_uid: OrderUid,
    pub penalty_cap_native: BigDecimal,
}

#[instrument(skip_all)]
pub async fn insert_batch(
    ex: &mut PgConnection,
    penalty_caps: impl IntoIterator<Item = OrderPenaltyCap>,
) -> Result<(), sqlx::Error> {
    let mut penalty_caps = penalty_caps.into_iter().peekable();
    if penalty_caps.peek().is_none() {
        return Ok(());
    }

    let mut query_builder = QueryBuilder::new(
        "INSERT INTO order_penalty_caps (auction_id, order_uid, penalty_cap_native)",
    );

    query_builder.push_values(penalty_caps, |mut b, penalty_cap| {
        b.push_bind(penalty_cap.auction_id)
            .push_bind(penalty_cap.order_uid)
            .push_bind(penalty_cap.penalty_cap_native);
    });

    query_builder.build().execute(ex).await.map(|_| ())
}

#[instrument(skip_all)]
pub async fn fetch(
    ex: &mut PgConnection,
    auction_id: AuctionId,
) -> Result<Vec<OrderPenaltyCap>, sqlx::Error> {
    const QUERY: &str = "SELECT * FROM order_penalty_caps WHERE auction_id = $1";
    sqlx::query_as(QUERY).bind(auction_id).fetch_all(ex).await
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

        assert!(fetch(&mut db, 1).await.unwrap().is_empty());

        let penalty_cap_1 = OrderPenaltyCap {
            auction_id: 1,
            order_uid: ByteArray([1; 56]),
            penalty_cap_native: BigDecimal::from(400_000_000_000_000_u64),
        };
        let penalty_cap_2 = OrderPenaltyCap {
            auction_id: 1,
            order_uid: ByteArray([2; 56]),
            penalty_cap_native: BigDecimal::from(0),
        };
        let other_auction = OrderPenaltyCap {
            auction_id: 2,
            order_uid: ByteArray([1; 56]),
            penalty_cap_native: BigDecimal::from(1),
        };

        insert_batch(
            &mut db,
            [
                penalty_cap_1.clone(),
                penalty_cap_2.clone(),
                other_auction.clone(),
            ],
        )
        .await
        .unwrap();

        let mut output = fetch(&mut db, 1).await.unwrap();
        output.sort_by_key(|cap| cap.order_uid.0);
        assert_eq!(output, vec![penalty_cap_1, penalty_cap_2]);

        assert_eq!(fetch(&mut db, 2).await.unwrap(), vec![other_auction]);
    }
}
