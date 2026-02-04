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
WITH user_order_uids AS (
    SELECT uid FROM orders WHERE owner = $1
    UNION
    SELECT uid FROM onchain_placed_orders WHERE sender = $1
),
trade_components AS (
    SELECT
        o.uid,
        CASE o.kind
            -- so much was actually bought
            WHEN 'sell' THEN t.buy_amount
            -- so much was actually converted to buy tokens
            WHEN 'buy' THEN t.sell_amount - t.fee_amount
        END AS trade_amount,
        CASE o.kind
            -- so much had to be bought at least (given executed amount and limit price)
            WHEN 'sell' THEN (t.sell_amount - t.fee_amount) * o.buy_amount / o.sell_amount
            -- so much could be converted to buy_token at most (given executed amount and limit price)
            WHEN 'buy' THEN t.buy_amount * o.sell_amount / o.buy_amount
        END AS limit_amount,
        o.kind,
        ap.price AS surplus_token_native_price
    FROM user_order_uids u
    JOIN orders o ON o.uid = u.uid
    JOIN trades t ON o.uid = t.order_uid
    JOIN order_execution oe ON o.uid = oe.order_uid
    LEFT JOIN auction_prices ap
        ON ap.auction_id = oe.auction_id
        AND ap.token = CASE o.kind WHEN 'sell' THEN o.buy_token ELSE o.sell_token END

    UNION ALL

    -- Additional query for jit_orders
    SELECT
        j.uid,
        CASE j.kind
            WHEN 'sell' THEN t.buy_amount
            WHEN 'buy' THEN t.sell_amount - t.fee_amount
        END AS trade_amount,
        CASE j.kind
            WHEN 'sell' THEN (t.sell_amount - t.fee_amount) * j.buy_amount / j.sell_amount
            WHEN 'buy' THEN t.buy_amount * j.sell_amount / j.buy_amount
        END AS limit_amount,
        j.kind,
        ap.price AS surplus_token_native_price
    FROM jit_orders j
    JOIN trades t ON j.uid = t.order_uid
    JOIN order_execution oe ON j.uid = oe.order_uid
    LEFT JOIN auction_prices ap
        ON ap.auction_id = oe.auction_id
        AND ap.token = CASE j.kind WHEN 'sell' THEN j.buy_token ELSE j.sell_token END
    WHERE j.owner = $1 AND NOT EXISTS (
        SELECT 1
        FROM orders o
        WHERE o.uid = j.uid
    )
),
trade_surplus AS (
    SELECT
        uid,
        CASE kind
            WHEN 'sell' THEN (trade_amount - limit_amount) * surplus_token_native_price
            WHEN 'buy' THEN (limit_amount - trade_amount) * surplus_token_native_price
        END / POWER(10, 18) AS surplus_in_wei
    FROM trade_components
)
SELECT
    COALESCE(SUM(surplus_in_wei ORDER BY uid), 0) AS total_surplus_in_wei
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
