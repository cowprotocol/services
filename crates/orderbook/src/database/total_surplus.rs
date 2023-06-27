use {
    anyhow::{Context, Result},
    database::{byte_array::ByteArray, Address},
    primitive_types::{H160, U256},
    sqlx::PgConnection,
};

/// Computes a user's total surplus received (price improvement over limit price
/// and **NOT** quoted price) since march 2023.
async fn fetch_total_surplus(ex: &mut PgConnection, user: &Address) -> Result<i64, sqlx::Error> {
    const TOTAL_SURPLUS_QUERY: &str = r#"
with trade_components as (
    select
       case kind
          -- so much was actually bought
          when 'sell' then t.buy_amount
          -- so much was actually converted to buy tokens
          when 'buy' then t.sell_amount - t.fee_amount
       end as trade_amount,
       case kind
          -- so much had to be bought at least (given exeucted amount and limit price)
          when 'sell' then (t.sell_amount - t.fee_amount) * o.buy_amount / o.sell_amount
          -- so much could be converted to buy_token at most (given executed amount and limit price)
          when 'buy' then t.buy_amount * o.sell_amount / o.buy_amount
       end as limit_amount,
       o.kind,
       (select price from auction_prices where ap.token = o.sell_token and ap.auction_id = oe.auction_id) as native_sell_price,
       (select price from auction_prices where ap.token = o.buy_token and ap.auction_id = oe.auction_id) as native_buy_price
    from orders o
    join trades t on o.uid = t.order_uid
    join order_quotes oq on o.uid = oq.order_uid
    left join onchain_placed_orders opo on opo.uid = o.uid
    join order_execution oe on o.uid = oe.order_uid
    where
        o.owner = $1
        or opo.sender = $1
),
trade_surplus as (
    select
        case kind
            -- amounts refer to tokens bought; more is better
            when 'sell' then (trade_amount - limit_amount) * native_buy_price
            -- amounts refer to tokens sold; less is better
            when 'buy' then (limit_amount - trade_amount) * native_sell_price
        end / power(10, 18) as surplus_in_wei,
        limit_amount,
        trade_amount
    from trade_components
)
select
   sum(surplus_in_wei) as total_surplus_in_wei
from trade_surplus
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
        U256::try_from(surplus).context("failed to convert surplus to U256")
    }
}
