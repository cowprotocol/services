use {
    crate::{Address, auction::AuctionId},
    bigdecimal::BigDecimal,
    sqlx::{Connection, PgConnection},
    std::ops::DerefMut,
    tracing::instrument,
};

/// External token price for a given auction.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct AuctionPrice {
    pub auction_id: AuctionId,
    pub token: Address,
    pub price: BigDecimal,
}

#[instrument(skip_all)]
pub async fn fetch_latest_prices(ex: &mut PgConnection) -> Result<Vec<AuctionPrice>, sqlx::Error> {
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
    ex.commit().await?;

    Ok(price)
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

        let auction_1 = vec![
            AuctionPrice {
                auction_id: 1,
                token: ByteArray([2; 20]),
                price: 1.into(),
            },
            AuctionPrice {
                auction_id: 1,
                token: ByteArray([3; 20]),
                price: 2.into(),
            },
        ];
        let auction_2 = vec![AuctionPrice {
            auction_id: 2,
            token: ByteArray([2; 20]),
            price: 3.into(),
        }];
        let auction_3 = vec![
            AuctionPrice {
                auction_id: 3,
                token: ByteArray([3; 20]),
                price: 4.into(),
            },
            AuctionPrice {
                auction_id: 3,
                token: ByteArray([4; 20]),
                price: 5.into(),
            },
        ];

        // Prices are stored as the parallel arrays of `competition_auctions`.
        for prices in [&auction_1, &auction_2, &auction_3] {
            crate::auction::save(
                &mut db,
                crate::auction::Auction {
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
        for prices in [&auction_1, &auction_2, &auction_3] {
            let stored = crate::auction::fetch(&mut db, prices[0].auction_id)
                .await
                .unwrap()
                .unwrap();
            let tokens: Vec<_> = prices.iter().map(|price| price.token).collect();
            let values: Vec<_> = prices.iter().map(|price| price.price.clone()).collect();
            assert_eq!(stored.price_tokens, tokens);
            assert_eq!(stored.price_values, values);
        }
        // non-existent auction
        assert!(crate::auction::fetch(&mut db, 4).await.unwrap().is_none());
        // latest prices
        let output = fetch_latest_prices(&mut db).await.unwrap();
        assert_eq!(output, auction_3);
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
