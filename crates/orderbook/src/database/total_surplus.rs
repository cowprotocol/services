use {
    anyhow::Result,
    database::{byte_array::ByteArray, Address},
    primitive_types::{H160, U256},
    sqlx::PgConnection,
};

/// Computes a user's total surplus received (price improvement over limit price
/// and **NOT** quoted price) since march 2023.
async fn fetch_total_surplus(ex: &mut PgConnection, user: &Address) -> Result<f64, sqlx::Error> {
    const TOTAL_SURPLUS_QUERY: &str = r#"
WITH regular_orders AS (
    SELECT ARRAY_AGG(uid) AS ids FROM orders WHERE owner = '\x5fc79e21ceca2aa0f7a0aac71ef3ddde8f004e9e'
),
onchain_orders AS (
    SELECT ARRAY_AGG(uid) AS ids FROM onchain_placed_orders WHERE sender = '\x5fc79e21ceca2aa0f7a0aac71ef3ddde8f004e9e'
),
trade_components AS (
    SELECT
       CASE kind
          -- so much was actually bought
          WHEN 'sell' THEN t.buy_amount
          -- so much was actually converted to buy tokens
          WHEN 'buy' THEN t.sell_amount - t.fee_amount
       END AS trade_amount,
       CASE kind
          -- so much had to be bought at least (given exeucted amount and limit price)
          WHEN 'sell' THEN (t.sell_amount - t.fee_amount) * o.buy_amount / o.sell_amount
          -- so much could be converted to buy_token at most (given executed amount and limit price)
          WHEN 'buy' THEN t.buy_amount * o.sell_amount / o.buy_amount
       END AS limit_amount,
       o.kind,
       (SELECT price FROM auction_prices ap WHERE ap.token = o.sell_token AND ap.auction_id = oe.auction_id) AS native_sell_price,
       (SELECT price FROM auction_prices ap WHERE ap.token = o.buy_token AND ap.auction_id = oe.auction_id) AS native_buy_price
    FROM orders o
    JOIN trades t ON o.uid = t.order_uid
    JOIN order_execution oe ON o.uid = oe.order_uid
    -- use this weird construction instead of `where owner=address or sender=address` to help postgres make efficient use of indices
    where uid = ANY(array_cat((select ids from regular_orders), (select ids from onchain_orders)))
),
trade_surplus AS (
    SELECT
        CASE kind
            -- amounts refer to tokens bought; more is better
            WHEN 'sell' THEN (trade_amount - limit_amount) * native_buy_price
            -- amounts refer to tokens sold; less is better
            WHEN 'buy' THEN (limit_amount - trade_amount) * native_sell_price
        END / POWER(10, 18) AS surplus_in_wei,
        limit_amount,
        trade_amount
    FROM trade_components
)
SELECT
   SUM(surplus_in_wei) AS total_surplus_in_wei
FROM trade_surplus
"#;

    sqlx::query_scalar(TOTAL_SURPLUS_QUERY)
        .bind(user)
        .fetch_one(ex)
        .await
}

impl super::Postgres {
    pub async fn total_surplus(&self, user: &H160) -> Result<U256> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["get_total_surplus"])
            .start_timer();

        let mut ex = self.pool.acquire().await?;
        let surplus = fetch_total_surplus(&mut ex, &ByteArray(user.0)).await?;
        Ok(U256::from_f64_lossy(surplus))
    }
}
