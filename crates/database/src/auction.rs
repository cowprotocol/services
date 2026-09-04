use {
    crate::{Address, OrderUid},
    bigdecimal::BigDecimal,
    sqlx::{Connection, PgConnection, types::JsonValue},
    std::ops::DerefMut,
    tracing::instrument,
};

pub type AuctionId = i64;

pub async fn load_most_recent(
    ex: &mut PgConnection,
) -> Result<Option<(AuctionId, JsonValue)>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT id, json
FROM auctions
ORDER BY id DESC
LIMIT 1
    ;"#;
    sqlx::query_as(QUERY).fetch_optional(ex).await
}

pub async fn last_used_auction_id(ex: &mut PgConnection) -> Result<Option<i64>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT id
FROM auctions
ORDER BY id DESC
LIMIT 1
    ;"#;
    sqlx::query_scalar(QUERY).fetch_optional(ex).await
}

pub async fn get_next_auction_id(ex: &mut PgConnection) -> Result<AuctionId, sqlx::Error> {
    const QUERY: &str =
        r#"SELECT nextval(pg_get_serial_sequence('auctions', 'id'))::bigint as next_id;"#;

    let (id,) = sqlx::query_as(QUERY).fetch_one(ex).await?;
    Ok(id)
}

pub async fn insert_auction_with_id(
    ex: &mut PgConnection,
    id: AuctionId,
    json: &str,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
WITH deleted AS (
    DELETE FROM auctions
)
INSERT INTO auctions (id, json)
VALUES ($1, $2::jsonb);
    "#;

    sqlx::query(QUERY).bind(id).bind(json).execute(ex).await?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct Auction {
    pub id: AuctionId,
    pub block: i64,
    pub deadline: i64,
    pub order_uids: Vec<OrderUid>,
    // External native prices
    pub price_tokens: Vec<Address>,
    pub price_values: Vec<BigDecimal>,
    pub surplus_capturing_jit_order_owners: Vec<Address>,
    /// Caps on the penalty for not executing an order, in native
    /// token wei, mapped one-to-one with `order_uids`. `None` when penalties
    /// were disabled at auction creation.
    pub penalty_caps_native: Option<Vec<BigDecimal>>,
}

pub async fn save(ex: &mut PgConnection, auction: Auction) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
INSERT INTO competition_auctions (id, block, deadline, order_uids, price_tokens, price_values, surplus_capturing_jit_order_owners, penalty_caps_native)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
    ;"#;

    sqlx::query(QUERY)
        .bind(auction.id)
        .bind(auction.block)
        .bind(auction.deadline)
        .bind(auction.order_uids)
        .bind(auction.price_tokens)
        .bind(auction.price_values)
        .bind(auction.surplus_capturing_jit_order_owners)
        .bind(auction.penalty_caps_native)
        .execute(ex)
        .await?;

    Ok(())
}

pub async fn fetch(ex: &mut PgConnection, id: AuctionId) -> Result<Option<Auction>, sqlx::Error> {
    const QUERY: &str = r#"SELECT * FROM competition_auctions WHERE id = $1;"#;
    sqlx::query_as(QUERY).bind(id).fetch_optional(ex).await
}

pub async fn fetch_multiple(
    ex: &mut PgConnection,
    ids: &[AuctionId],
) -> Result<Vec<Auction>, sqlx::Error> {
    const QUERY: &str = r#"SELECT * FROM competition_auctions WHERE id = ANY($1) ORDER BY id"#;
    sqlx::query_as(QUERY).bind(ids).fetch_all(ex).await
}

pub async fn get_order_uids(
    ex: &mut PgConnection,
    auction_id: AuctionId,
) -> Result<Option<Vec<OrderUid>>, sqlx::Error> {
    const QUERY: &str = r#"SELECT order_uids FROM competition_auctions WHERE id = $1;"#;
    let record: Option<(Vec<OrderUid>,)> = sqlx::query_as(QUERY)
        .bind(auction_id)
        .fetch_optional(ex)
        .await?;
    Ok(record.map(|(order_uids,)| order_uids))
}

pub async fn fetch_auction_ids_by_order_uid(
    ex: &mut PgConnection,
    order_uid: &OrderUid,
) -> Result<Vec<AuctionId>, sqlx::Error> {
    const QUERY: &str =
        "SELECT id FROM competition_auctions WHERE order_uids @> ARRAY[$1::bytea] ORDER BY id";
    let rows: Vec<(AuctionId,)> = sqlx::query_as(QUERY).bind(order_uid).fetch_all(ex).await?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

/// External token price for a given auction.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct NativePrice {
    pub auction_id: AuctionId,
    pub token: Address,
    pub price: BigDecimal,
}

#[instrument(skip_all)]
pub async fn fetch_latest_prices(ex: &mut PgConnection) -> Result<Vec<NativePrice>, sqlx::Error> {
    const QUERY: &str = r#"
    SELECT
        c.id AS auction_id,
        unnest(c.price_tokens) AS token,
        unnest(c.price_values) AS price
    FROM competition_auctions c
    WHERE c.id = (
        SELECT MAX(id) FROM competition_auctions
    )
    "#;
    sqlx::query_as(QUERY).fetch_all(ex).await
}

/// Native price of `token` in the most recent auction that priced it.
#[instrument(skip_all)]
pub async fn fetch_latest_token_price(
    ex: &mut PgConnection,
    token: Address,
) -> Result<Option<BigDecimal>, sqlx::Error> {
    // TODO: tokens priced in the newest auctions are much cheaper to resolve if
    // we add a lookback and fall to this query on misses
    const QUERY: &str = r#"
    SELECT price_values[array_position(price_tokens, $1)]
    FROM competition_auctions
    WHERE id = (
        SELECT max(id)
        FROM (
            SELECT id
            FROM competition_auctions
            WHERE price_tokens @> ARRAY[$1]
            -- `OFFSET 0` fences the subquery so the planner uses the `price_tokens` GIN
            -- index; flattened, it picks a backward primary key scan it costs at ~1.
            OFFSET 0
        ) matches
    )
    "#;

    // Transaction is necessary for the SET LOCAL
    let mut ex = ex.begin().await?;
    // The GIN scan returns a bitmap of matching tuples. Past `work_mem` it does
    // not spill to disk, it degrades pages to "something here matched", and
    // those pages recheck `@>` per tuple — reading `price_tokens` back out of
    // TOAST every time. 32MB holds a bitmap over the whole heap.
    sqlx::query("SET LOCAL work_mem = '32MB'")
        .execute(ex.deref_mut())
        .await?;
    let price = sqlx::query_scalar(QUERY)
        .bind(token)
        .fetch_optional(ex.deref_mut())
        .await?;

    Ok(price)
}

#[cfg(test)]
mod tests {
    use {super::*, crate::byte_array::ByteArray};

    #[tokio::test]
    #[ignore]
    async fn postgres_roundtrip() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let value = JsonValue::Number(1.into());
        let id = get_next_auction_id(&mut db).await.unwrap();
        let value_str = serde_json::to_string(&value).unwrap();
        insert_auction_with_id(&mut db, id, &value_str)
            .await
            .unwrap();
        let (id_, value_) = load_most_recent(&mut db).await.unwrap().unwrap();
        assert_eq!(id, id_);
        assert_eq!(value, value_);

        let value = JsonValue::Number(2.into());
        let id_ = get_next_auction_id(&mut db).await.unwrap();
        assert_eq!(id + 1, id_);
        let value_str = serde_json::to_string(&value).unwrap();
        insert_auction_with_id(&mut db, id_, &value_str)
            .await
            .unwrap();
        let (id, value_) = load_most_recent(&mut db).await.unwrap().unwrap();
        assert_eq!(value, value_);
        assert_eq!(id_, id);

        // let's assume the second auction contains a valid competition data so
        // it's meaningful to save it into `competition_auctions` table
        // as well
        let auction = Auction {
            id: id_,
            block: 1,
            deadline: 2,
            order_uids: vec![ByteArray([1u8; 56]), ByteArray([2u8; 56])],
            price_tokens: vec![ByteArray([1u8; 20])],
            price_values: vec![BigDecimal::from(1)],
            surplus_capturing_jit_order_owners: vec![ByteArray([1u8; 20])],
            penalty_caps_native: Some(vec![
                BigDecimal::from(400_000_000_000_000_u64),
                BigDecimal::from(0),
            ]),
        };
        save(&mut db, auction.clone()).await.unwrap();
        let auction_ = fetch(&mut db, id_).await.unwrap().unwrap();
        assert_eq!(auction, auction_);

        let order_uids = get_order_uids(&mut db, id_).await.unwrap().unwrap();
        assert_eq!(auction.order_uids, order_uids);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_prices_roundtrip() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let native_price_1 = vec![
            NativePrice {
                auction_id: 1,
                token: ByteArray([2; 20]),
                price: 1.into(),
            },
            NativePrice {
                auction_id: 1,
                token: ByteArray([3; 20]),
                price: 2.into(),
            },
        ];
        let native_price_2 = vec![NativePrice {
            auction_id: 2,
            token: ByteArray([2; 20]),
            price: 3.into(),
        }];
        let native_price_3 = vec![
            NativePrice {
                auction_id: 3,
                token: ByteArray([3; 20]),
                price: 4.into(),
            },
            NativePrice {
                auction_id: 3,
                token: ByteArray([4; 20]),
                price: 5.into(),
            },
        ];

        // Prices are stored as the parallel arrays of `competition_auctions`.
        for prices in [&native_price_1, &native_price_2, &native_price_3] {
            save(
                &mut db,
                Auction {
                    id: prices[0].auction_id,
                    block: 0,
                    deadline: 0,
                    order_uids: vec![],
                    price_tokens: prices.iter().map(|price| price.token).collect(),
                    price_values: prices.iter().map(|price| price.price.clone()).collect(),
                    surplus_capturing_jit_order_owners: vec![],
                    penalty_caps_native: None,
                },
            )
            .await
            .unwrap();
        }

        // check that all auctions are there
        for prices in [&native_price_1, &native_price_2, &native_price_3] {
            let stored = fetch(&mut db, prices[0].auction_id).await.unwrap().unwrap();
            let tokens: Vec<_> = prices.iter().map(|price| price.token).collect();
            let values: Vec<_> = prices.iter().map(|price| price.price.clone()).collect();
            assert_eq!(stored.price_tokens, tokens);
            assert_eq!(stored.price_values, values);
        }
        // non-existent auction
        assert!(fetch(&mut db, 4).await.unwrap().is_none());
        // latest prices
        let output = fetch_latest_prices(&mut db).await.unwrap();
        assert_eq!(output, native_price_3);
        // latest token price
        let output = fetch_latest_token_price(&mut db, ByteArray([2; 20]))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(output, BigDecimal::from(3));
        // a token that was never priced
        let output = fetch_latest_token_price(&mut db, ByteArray([9; 20]))
            .await
            .unwrap();
        assert_eq!(output, None);
    }
}
