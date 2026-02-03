use {
    alloy::primitives::U256,
    anyhow::Result,
    database::{Address, byte_array::ByteArray},
    sqlx::PgConnection,
};

/// Computes a user's total surplus received (price improvement over limit price
/// and **NOT** quoted price) since march 2023.
async fn fetch_total_surplus(ex: &mut PgConnection, user: &Address) -> Result<f64, sqlx::Error> {
    const TOTAL_SURPLUS_QUERY: &str = r#"
-- historical context: array_agg was used which caused the whole array to be materialized in memory
-- furthermore, arrays are not estimatable and default values (~10 rows) would be VERY wrong
-- solution: duplicating the CTEs and avoiding arrays leads to more accurate estimations and plans
WITH regular_order_trades AS (
    SELECT
        CASE o.kind
            -- so much was actually bought
            WHEN 'sell' THEN t.buy_amount
            -- so much was actually converted to buy tokens
            WHEN 'buy' THEN t.sell_amount - t.fee_amount
        END AS trade_amount,
        CASE o.kind
            -- so much had to be bought at least (given exeucted amount and limit price)
            WHEN 'sell' THEN (t.sell_amount - t.fee_amount) * o.buy_amount / o.sell_amount
            -- so much could be converted to buy_token at most (given executed amount and limit price)
            WHEN 'buy' THEN t.buy_amount * o.sell_amount / o.buy_amount
        END AS limit_amount,
        o.kind,
        CASE o.kind
            WHEN 'sell' THEN (SELECT price FROM auction_prices ap WHERE ap.token = o.buy_token AND ap.auction_id = oe.auction_id)
            WHEN 'buy' THEN (SELECT price FROM auction_prices ap WHERE ap.token = o.sell_token AND ap.auction_id = oe.auction_id)
        END AS surplus_token_native_price,
        o.uid as uid
    FROM orders o
    JOIN trades t ON t.order_uid = o.uid
    JOIN order_execution oe ON oe.order_uid = o.uid
    WHERE o.owner = $1
),
onchain_order_trades AS (
    SELECT
        CASE o.kind
            -- so much was actually bought
            WHEN 'sell' THEN t.buy_amount
            -- so much was actually converted to buy tokens
            WHEN 'buy' THEN t.sell_amount - t.fee_amount
        END AS trade_amount,
        CASE o.kind
            -- so much had to be bought at least (given exeucted amount and limit price)
            WHEN 'sell' THEN (t.sell_amount - t.fee_amount) * o.buy_amount / o.sell_amount
            -- so much could be converted to buy_token at most (given executed amount and limit price)
            WHEN 'buy' THEN t.buy_amount * o.sell_amount / o.buy_amount
        END AS limit_amount,
        o.kind,
        CASE o.kind
            WHEN 'sell' THEN (SELECT price FROM auction_prices ap WHERE ap.token = o.buy_token AND ap.auction_id = oe.auction_id)
            WHEN 'buy' THEN (SELECT price FROM auction_prices ap WHERE ap.token = o.sell_token AND ap.auction_id = oe.auction_id)
        END AS surplus_token_native_price,
        o.uid as uid
    FROM orders o
    JOIN trades t ON o.uid = t.order_uid
    JOIN order_execution oe ON o.uid = oe.order_uid
    -- deduplicate orders
    WHERE o.owner != $1
      AND EXISTS (
          SELECT 1 FROM onchain_placed_orders opo
          WHERE opo.uid = o.uid AND opo.sender = $1
      )
),
jit_order_trades AS (
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
        END AS surplus_token_native_price,
        j.uid as uid
    FROM jit_orders j
    JOIN trades t ON j.uid = t.order_uid
    JOIN order_execution oe ON j.uid = oe.order_uid
    WHERE j.owner = $1 AND NOT EXISTS (
        SELECT 1
        FROM orders o
        WHERE o.uid = j.uid
    )
),
trade_components AS (
    SELECT * FROM regular_order_trades
    UNION ALL
    SELECT * FROM onchain_order_trades
    UNION ALL
    SELECT * FROM jit_order_trades
),
trade_surplus AS (
    SELECT
        CASE kind
            -- amounts refer to tokens bought; more is better
            WHEN 'sell' THEN (trade_amount - limit_amount) * surplus_token_native_price
            -- amounts refer to tokens sold; less is better
            WHEN 'buy' THEN (limit_amount - trade_amount) * surplus_token_native_price
        END / POWER(10, 18) AS surplus_in_wei,
        uid
    FROM trade_components
)
SELECT
    -- use uid to order to avoid floating point rounding issues
    COALESCE(SUM(surplus_in_wei order by uid), 0) AS total_surplus_in_wei
FROM trade_surplus
"#;

    sqlx::query_scalar(TOTAL_SURPLUS_QUERY)
        .bind(user)
        .fetch_one(ex)
        .await
}

impl super::Postgres {
    pub async fn total_surplus(&self, user: &alloy::primitives::Address) -> Result<U256> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["get_total_surplus"])
            .start_timer();

        let mut ex = self.pool.acquire().await?;
        let surplus = fetch_total_surplus(&mut ex, &ByteArray(user.0.0)).await?;
        Ok(U256::from(surplus))
    }
}
