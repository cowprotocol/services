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
WITH trade_components AS (
    SELECT
        o.uid,
        oe.auction_id,
        CASE o.kind
            WHEN 'sell' THEN t.buy_amount
            WHEN 'buy' THEN t.sell_amount - t.fee_amount
        END AS trade_amount,
        CASE o.kind
            WHEN 'sell' THEN (t.sell_amount - t.fee_amount) * o.buy_amount / o.sell_amount
            WHEN 'buy' THEN t.buy_amount * o.sell_amount / o.buy_amount
        END AS limit_amount,
        o.kind,
        CASE o.kind WHEN 'sell' THEN o.buy_token ELSE o.sell_token END AS surplus_token
    FROM orders o
    JOIN trades t ON t.order_uid = o.uid
    JOIN order_execution oe ON oe.order_uid = t.order_uid
    WHERE o.owner = $1

    UNION ALL

    SELECT
        o.uid,
        oe.auction_id,
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
        CASE o.kind WHEN 'sell' THEN o.buy_token ELSE o.sell_token END AS surplus_token
    FROM onchain_placed_orders opo
    JOIN orders o ON o.uid = opo.uid AND o.owner != $1
    JOIN trades t ON t.order_uid = o.uid
    JOIN order_execution oe ON oe.order_uid = t.order_uid
    WHERE opo.sender = $1

    UNION ALL

    -- Additional query for jit_orders
    SELECT
        j.uid,
        oe.auction_id,
        CASE j.kind
            WHEN 'sell' THEN t.buy_amount
            WHEN 'buy' THEN t.sell_amount - t.fee_amount
        END AS trade_amount,
        CASE j.kind
            WHEN 'sell' THEN (t.sell_amount - t.fee_amount) * j.buy_amount / j.sell_amount
            WHEN 'buy' THEN t.buy_amount * j.sell_amount / j.buy_amount
        END AS limit_amount,
        j.kind,
        CASE j.kind WHEN 'sell' THEN j.buy_token ELSE j.sell_token END AS surplus_token
    FROM jit_orders j
    JOIN trades t ON j.uid = t.order_uid
    JOIN order_execution oe ON t.order_uid = oe.order_uid
    WHERE j.owner = $1
      AND NOT EXISTS (
        SELECT 1
        FROM orders o
        WHERE o.uid = j.uid
    )
),
-- Price the distinct (auction, token) pairs only. `price_tokens`/`price_values`
-- are TOASTed, so every `array_position` call detoasts ~34 kB, and an order can
-- have many `order_execution` rows.
native_prices AS (
    SELECT
        p.auction_id,
        p.surplus_token,
        ca.price_values[array_position(ca.price_tokens, p.surplus_token)] AS price
    FROM (SELECT DISTINCT auction_id, surplus_token FROM trade_components) p
    LEFT JOIN competition_auctions ca ON ca.id = p.auction_id
)
SELECT
    COALESCE(SUM(surplus_in_wei ORDER BY uid), 0) AS total_surplus_in_wei
FROM (
    SELECT
        tc.uid,
        CASE tc.kind
            WHEN 'sell' THEN (tc.trade_amount - tc.limit_amount) * np.price
            WHEN 'buy' THEN (tc.limit_amount - tc.trade_amount) * np.price
        END / POWER(10, 18) AS surplus_in_wei
    FROM trade_components tc
    LEFT JOIN native_prices np
        ON np.auction_id = tc.auction_id
        AND np.surplus_token = tc.surplus_token
) ts;
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
