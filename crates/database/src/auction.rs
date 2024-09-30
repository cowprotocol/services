use {
    crate::{Address, OrderUid},
    bigdecimal::BigDecimal,
    sqlx::{types::JsonValue, PgConnection},
};

pub type AuctionId = i64;

pub async fn load_most_recent(
    ex: &mut PgConnection,
) -> Result<Option<(AuctionId, JsonValue)>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT id, json
FROM latest_auction
ORDER BY id DESC
LIMIT 1
    ;"#;
    sqlx::query_as(QUERY).fetch_optional(ex).await
}

pub async fn replace_auction(
    ex: &mut PgConnection,
    data: &JsonValue,
) -> Result<AuctionId, sqlx::Error> {
    const QUERY: &str = r#"
WITH deleted AS (
    DELETE FROM latest_auction
)
INSERT INTO latest_auction (json)
VALUES ($1)
RETURNING id;
    "#;

    let (id,) = sqlx::query_as(QUERY).bind(data).fetch_one(ex).await?;
    Ok(id)
}

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct Auction {
    pub id: AuctionId,
    pub block: i64,
    pub deadline: i64,
    pub orders: Vec<OrderUid>,
    pub price_tokens: Vec<Address>,
    pub price_values: Vec<BigDecimal>,
    pub surplus_capturing_jit_order_owners: Vec<Address>,
}

pub async fn save(ex: &mut PgConnection, auction: Auction) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
INSERT INTO auctions (id, block, deadline, orders, price_tokens, price_values, surplus_capturing_jit_order_owners)
VALUES ($1, $2, $3, $4, $5, $6, $7)
    ;"#;

    sqlx::query(QUERY)
        .bind(auction.id)
        .bind(auction.block)
        .bind(auction.deadline)
        .bind(auction.orders)
        .bind(auction.price_tokens)
        .bind(auction.price_values)
        .bind(auction.surplus_capturing_jit_order_owners)
        .execute(ex)
        .await?;

    Ok(())
}

pub async fn fetch(ex: &mut PgConnection, id: AuctionId) -> Result<Option<Auction>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT id, block, deadline, orders, price_tokens, price_values, surplus_capturing_jit_order_owners
FROM auctions
WHERE id = $1
    ;"#;

    sqlx::query_as(QUERY).bind(id).fetch_optional(ex).await
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

        let value = JsonValue::Number(1.into());
        let id = replace_auction(&mut db, &value).await.unwrap();
        let (id_, value_) = load_most_recent(&mut db).await.unwrap().unwrap();
        assert_eq!(id, id_);
        assert_eq!(value, value_);

        let value = JsonValue::Number(2.into());
        let id_ = replace_auction(&mut db, &value).await.unwrap();
        assert_eq!(id + 1, id_);
        let (id, value_) = load_most_recent(&mut db).await.unwrap().unwrap();
        assert_eq!(value, value_);
        assert_eq!(id_, id);

        // let's assume the second auction contains a valid competition data so it's
        // meaningful to save it into `auctions` table as well
        let auction = Auction {
            id: id_,
            block: 1,
            deadline: 2,
            orders: vec![ByteArray([1u8; 56])],
            price_tokens: vec![ByteArray([1u8; 20])],
            price_values: vec![BigDecimal::from(1)],
            surplus_capturing_jit_order_owners: vec![ByteArray([1u8; 20])],
        };
        save(&mut db, auction.clone()).await.unwrap();
        let auction_ = fetch(&mut db, id_).await.unwrap().unwrap();
        assert_eq!(auction, auction_);
    }
}
