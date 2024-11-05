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
    SELECT ARRAY_AGG(uid) AS ids FROM orders WHERE owner = $1
),
onchain_orders AS (
    SELECT ARRAY_AGG(uid) AS ids FROM onchain_placed_orders WHERE sender = $1
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
       (SELECT ca.price_values[idx] 
        FROM competition_auctions ca,
             UNNEST(ca.price_tokens, ca.price_values) WITH ORDINALITY AS t(token, value, idx)
        WHERE ca.id = oe.auction_id
          AND t.token = CASE kind
              WHEN 'sell' THEN o.buy_token
              WHEN 'buy' THEN o.sell_token
          LIMIT 1
       ) AS surplus_token_native_price
    FROM orders o
    JOIN trades t ON o.uid = t.order_uid
    JOIN order_execution oe ON o.uid = oe.order_uid
    -- use this weird construction instead of `where owner=address or sender=address` to help postgres make efficient use of indices
    WHERE uid = ANY(ARRAY_CAT((SELECT ids FROM regular_orders), (SELECT ids FROM onchain_orders)))

    UNION ALL

    -- Additional query for jit_orders
    SELECT
       CASE j.kind
          WHEN 'sell' THEN t.buy_amount
          WHEN 'buy' THEN t.sell_amount - t.fee_amount
       END AS trade_amount,
       CASE j.kind
          WHEN 'sell' THEN (t.sell_amount - t.fee_amount) * j.buy_amount / j.sell_amount
          WHEN 'buy' THEN t.buy_amount * j.sell_amount / j.buy_amount
       END AS limit_amount,
       j.kind,
       CASE j.kind
          WHEN 'sell' THEN (SELECT price FROM auction_prices ap WHERE ap.token = j.buy_token AND ap.auction_id = oe.auction_id)
          WHEN 'buy' THEN (SELECT price FROM auction_prices ap WHERE ap.token = j.sell_token AND ap.auction_id = oe.auction_id)
       END AS surplus_token_native_price
    FROM jit_orders j
    JOIN trades t ON j.uid = t.order_uid
    JOIN order_execution oe ON j.uid = oe.order_uid
    WHERE j.owner = $1 AND NOT EXISTS (
        SELECT 1 
        FROM orders o 
        WHERE o.uid = j.uid
    )
),
trade_surplus AS (
    SELECT
        CASE kind
            -- amounts refer to tokens bought; more is better
            WHEN 'sell' THEN (trade_amount - limit_amount) * surplus_token_native_price
            -- amounts refer to tokens sold; less is better
            WHEN 'buy' THEN (limit_amount - trade_amount) * surplus_token_native_price
        END / POWER(10, 18) AS surplus_in_wei
    FROM trade_components
)
SELECT
   COALESCE(SUM(surplus_in_wei), 0) AS total_surplus_in_wei
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
