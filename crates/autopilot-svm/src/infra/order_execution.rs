//! The fees each order paid in a settlement, per protocol fee policy and in
//! total, recorded in `solana.order_execution` by a sweep over the final
//! trades.

use {
    crate::{
        domain::fee::{self, FeePolicy, Quote},
        infra::db,
    },
    anyhow::{Context, Result},
    bigdecimal::BigDecimal,
    chain_types::solana::{IntentHash, Pubkey, Solana},
    database::{
        byte_array::ByteArray,
        solana::{OrderKind, PolicyKind},
    },
    protocol_fees::Side,
    sqlx::{PgPool, Postgres, QueryBuilder},
    std::{collections::HashMap, time::Duration},
};

/// Pause between sweeps.
const SWEEP_INTERVAL: Duration = Duration::from_secs(10);

/// Trades one sweep records at most, which keeps its insert far below the
/// bind parameter limit.
const SWEEP_LIMIT: i64 = 1_000;

/// A final trade without an `order_execution` row, with its order's limit
/// and linked quote.
#[derive(Debug, sqlx::FromRow)]
struct TradeRow {
    order_uid: ByteArray<32>,
    /// The auction the settlement instruction claims.
    auction_id: i64,
    kind: OrderKind,
    sell_token: ByteArray<32>,
    buy_token: ByteArray<32>,
    limit_sell: BigDecimal,
    limit_buy: BigDecimal,
    /// Sell tokens the settlement pulled, fees included.
    executed_sell: BigDecimal,
    /// Buy tokens the settlement paid out, fees taken.
    executed_buy: BigDecimal,
    quote_sell_amount: Option<BigDecimal>,
    quote_buy_amount: Option<BigDecimal>,
    quote_execution_cost: Option<BigDecimal>,
    quote_solver: Option<ByteArray<32>>,
    /// What the winning proposal with the trade's amounts took from the fill,
    /// in the surplus token.
    proposed_fee: Option<BigDecimal>,
}

/// A `solana.fee_policies` row.
#[derive(Debug, sqlx::FromRow)]
struct PolicyRow {
    auction_id: i64,
    order_uid: ByteArray<32>,
    kind: PolicyKind,
    surplus_factor: Option<f64>,
    surplus_max_volume_factor: Option<f64>,
    price_improvement_factor: Option<f64>,
    price_improvement_max_volume_factor: Option<f64>,
    volume_factor: Option<f64>,
}

/// Sweeps every [`SWEEP_INTERVAL`], starting right away. A failed sweep is
/// logged and retried on the next tick.
pub fn spawn(pool: PgPool) {
    init_metrics();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(SWEEP_INTERVAL);
        loop {
            interval.tick().await;
            if let Err(err) = record(&pool).await {
                tracing::warn!(?err, "failed to record order executions");
            }
        }
    });
}

/// Record the fees of up to [`SWEEP_LIMIT`] final trades without a row,
/// oldest first. Only final trades, so no reorg can take back a trade with
/// a recorded fee. A trade whose fees do not compute is recorded without
/// fees, so every trade is swept once.
async fn record(pool: &PgPool) -> Result<()> {
    let _timer = db::query_timer("record_order_executions");
    const TRADES: &str = r#"
SELECT t.order_uid, s.auction_id, o.kind, o.sell_token, o.buy_token,
       o.sell_amount AS limit_sell, o.buy_amount AS limit_buy,
       t.sell_amount AS executed_sell, t.buy_amount AS executed_buy,
       q.sell_amount AS quote_sell_amount, q.buy_amount AS quote_buy_amount,
       q.execution_cost AS quote_execution_cost, q.solver AS quote_solver,
       (SELECT pte.fee
        FROM solana.proposed_solutions ps
        JOIN solana.proposed_trade_executions pte
          ON pte.auction_id = ps.auction_id AND pte.solution_uid = ps.uid
        WHERE ps.auction_id = s.auction_id AND ps.solver = s.solver AND ps.is_winner
          AND pte.order_uid = t.order_uid
          AND pte.executed_sell = t.sell_amount AND pte.executed_buy = t.buy_amount
        LIMIT 1) AS proposed_fee
FROM solana.trades t
JOIN solana.settlements s
  ON s.tx_signature = t.tx_signature AND s.instruction_index = t.instruction_index
JOIN solana.orders o ON o.uid = t.order_uid
LEFT JOIN solana.order_quotes q ON q.order_uid = t.order_uid
WHERE t.slot <= (SELECT finalized_slot FROM solana.indexer_state)
  AND NOT EXISTS (
      SELECT 1 FROM solana.order_execution e
      WHERE e.order_uid = t.order_uid AND e.auction_id = s.auction_id
  )
ORDER BY t.slot
LIMIT $1
    "#;
    let trades: Vec<TradeRow> = sqlx::query_as(TRADES)
        .bind(SWEEP_LIMIT)
        .fetch_all(pool)
        .await
        .context("read the unrecorded trades")?;
    if trades.is_empty() {
        return Ok(());
    }

    const POLICIES: &str = r#"
SELECT auction_id, order_uid, kind, surplus_factor, surplus_max_volume_factor,
       price_improvement_factor, price_improvement_max_volume_factor, volume_factor
FROM solana.fee_policies
WHERE (auction_id, order_uid) IN (SELECT * FROM UNNEST($1::bigint[], $2::bytea[]))
ORDER BY application_order
    "#;
    let (auction_ids, uids): (Vec<_>, Vec<_>) = trades
        .iter()
        .map(|trade| (trade.auction_id, trade.order_uid))
        .unzip();
    let mut policies: HashMap<_, Vec<PolicyRow>> = HashMap::new();
    for row in sqlx::query_as::<_, PolicyRow>(POLICIES)
        .bind(auction_ids)
        .bind(uids)
        .fetch_all(pool)
        .await
        .context("read the fee policies")?
    {
        policies
            .entry((row.auction_id, row.order_uid))
            .or_default()
            .push(row);
    }

    let executions: Vec<_> = trades
        .iter()
        .map(|trade| {
            let policies = policies
                .get(&(trade.auction_id, trade.order_uid))
                .map_or(&[][..], Vec::as_slice);
            let breakdown = fees(trade, policies).and_then(|fees| Ok((fees, executed_fee(trade)?)));
            let (fees, executed_fee) = breakdown.unwrap_or_else(|err| {
                tracing::warn!(
                    ?err,
                    auction_id = trade.auction_id,
                    trade = %IntentHash(trade.order_uid.0),
                    "possible incomplete fee breakdown calculation",
                );
                metrics()
                    .settlement_math_errors
                    .with_label_values(&["fee_breakdown"])
                    .inc();
                (Vec::new(), None)
            });
            let (tokens, amounts): (Vec<_>, Vec<_>) = fees
                .into_iter()
                .map(|(token, amount)| (token.0.to_vec(), BigDecimal::from(amount)))
                .unzip();
            (trade, tokens, amounts, executed_fee.map(BigDecimal::from))
        })
        .collect();
    let mut insert = QueryBuilder::<Postgres>::new(
        "INSERT INTO solana.order_execution (order_uid, auction_id, protocol_fee_tokens, \
         protocol_fee_amounts, executed_fee) ",
    );
    insert.push_values(
        executions,
        |mut row, (trade, tokens, amounts, executed_fee)| {
            row.push_bind(trade.order_uid)
                .push_bind(trade.auction_id)
                .push_bind(tokens)
                .push_bind(amounts)
                .push_bind(executed_fee);
        },
    );
    // A concurrent sweep may have recorded the same trade already.
    insert.push(" ON CONFLICT DO NOTHING");
    insert
        .build()
        .execute(pool)
        .await
        .context("insert the order executions")?;
    Ok(())
}

/// The fee each policy took from the trade, in its order's surplus token,
/// recovered from the settled legs.
fn fees(trade: &TradeRow, policies: &[PolicyRow]) -> Result<Vec<(Pubkey, u64)>> {
    let fill = protocol_fees::Order::<Solana> {
        uid: IntentHash(trade.order_uid.0),
        sell_token: Pubkey(trade.sell_token.0),
        buy_token: Pubkey(trade.buy_token.0),
        sell_amount: db::to_amount(&trade.limit_sell).context("limit_sell")?,
        buy_amount: db::to_amount(&trade.limit_buy).context("limit_buy")?,
        executed_sell: db::to_amount(&trade.executed_sell).context("executed_sell")?,
        executed_buy: db::to_amount(&trade.executed_buy).context("executed_buy")?,
        side: match trade.kind {
            OrderKind::Sell => Side::Sell,
            OrderKind::Buy => Side::Buy,
        },
    };
    let quote = fee::reference_quote(
        fill.sell_amount,
        db::linked_quote(
            trade.kind,
            trade.quote_sell_amount.as_ref(),
            trade.quote_buy_amount.as_ref(),
            trade.quote_execution_cost.as_ref(),
            trade.quote_solver,
        ),
    );
    let policies = policies
        .iter()
        .map(|row| row.policy(quote))
        .collect::<Result<Vec<_>>>()?;
    let surplus_token = match fill.side {
        Side::Sell => fill.buy_token,
        Side::Buy => fill.sell_token,
    };
    Ok(protocol_fees::protocol_fees_by_policy(&fill, &policies)?
        .into_iter()
        .map(|amount| (surplus_token, amount))
        .collect())
}

/// The fee the winning proposal took, in the sell token. A sell order's
/// converts at the rate before the fee, rounded down. A buy order's is in the
/// sell token already.
fn executed_fee(trade: &TradeRow) -> Result<Option<u64>> {
    let Some(fee) = &trade.proposed_fee else {
        return Ok(None);
    };
    let fee = db::to_amount(fee).context("proposed_fee")?;
    if trade.kind == OrderKind::Buy {
        return Ok(Some(fee));
    }
    let sell = u128::from(db::to_amount(&trade.executed_sell).context("executed_sell")?);
    let buy = u128::from(db::to_amount(&trade.executed_buy).context("executed_buy")?);
    let fee = u128::from(fee);
    let fee = (fee * sell).checked_div(buy + fee).unwrap_or(0);
    Ok(Some(u64::try_from(fee).context("executed fee")?))
}

impl PolicyRow {
    /// The policy as the auction attached it. A price improvement policy
    /// measures against `quote`.
    fn policy(&self, quote: Quote) -> Result<FeePolicy> {
        Ok(match self.kind {
            PolicyKind::Surplus => FeePolicy::Surplus {
                factor: self.surplus_factor.context("missing surplus_factor")?,
                max_volume_factor: self
                    .surplus_max_volume_factor
                    .context("missing surplus_max_volume_factor")?,
            },
            PolicyKind::PriceImprovement => FeePolicy::PriceImprovement {
                factor: self
                    .price_improvement_factor
                    .context("missing price_improvement_factor")?,
                max_volume_factor: self
                    .price_improvement_max_volume_factor
                    .context("missing price_improvement_max_volume_factor")?,
                quote,
            },
            PolicyKind::Volume => FeePolicy::Volume {
                factor: self.volume_factor.context("missing volume_factor")?,
            },
        })
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "domain")]
struct Metrics {
    /// Trades whose fee breakdown failed and were recorded without fees.
    #[metric(labels("kind"))]
    settlement_math_errors: prometheus::IntCounterVec,
}

fn metrics() -> &'static Metrics {
    Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
}

/// Publishes the `settlement_math_errors` series at zero, so an alert on its
/// `increase()` catches the first failure too.
fn init_metrics() {
    metrics()
        .settlement_math_errors
        .with_label_values(&["fee_breakdown"])
        .reset();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sell order of 100 for at least 100 that settled 100 for 150 after
    /// fees, linked to a route quote `(sell, buy, execution cost)`.
    fn trade(quote: Option<(u64, u64, u64)>) -> TradeRow {
        TradeRow {
            order_uid: ByteArray([1; 32]),
            auction_id: 1,
            kind: OrderKind::Sell,
            sell_token: ByteArray([2; 32]),
            buy_token: ByteArray([3; 32]),
            limit_sell: 100.into(),
            limit_buy: 100.into(),
            executed_sell: 100.into(),
            executed_buy: 150.into(),
            quote_sell_amount: quote.map(|(sell, _, _)| sell.into()),
            quote_buy_amount: quote.map(|(_, buy, _)| buy.into()),
            quote_execution_cost: quote.map(|(_, _, cost)| cost.into()),
            quote_solver: quote.map(|_| ByteArray([9; 32])),
            proposed_fee: None,
        }
    }

    /// A sell order's fee, taken in the buy token, converts at the rate
    /// before it and rounds down. A buy order's fee is in the sell token
    /// already, and a trade without a reported fee has none.
    #[test]
    fn the_executed_fee_is_in_the_sell_token() {
        // 100 sold for 190 after a fee of 10, so for 200 before it: the fee
        // is worth 5 sold.
        let sell = TradeRow {
            executed_sell: 100.into(),
            executed_buy: 190.into(),
            proposed_fee: Some(10.into()),
            ..trade(None)
        };
        assert_eq!(executed_fee(&sell).unwrap(), Some(5));
        // 1 of the 300 bought before the fee is a third of a sold atom.
        let dust = TradeRow {
            executed_sell: 100.into(),
            executed_buy: 299.into(),
            proposed_fee: Some(1.into()),
            ..trade(None)
        };
        assert_eq!(executed_fee(&dust).unwrap(), Some(0));
        // A fee of everything bought is everything sold, at the u64 ceiling.
        let all = TradeRow {
            executed_sell: u64::MAX.into(),
            executed_buy: 0.into(),
            proposed_fee: Some(u64::MAX.into()),
            ..trade(None)
        };
        assert_eq!(executed_fee(&all).unwrap(), Some(u64::MAX));
        let nothing = TradeRow {
            executed_buy: 0.into(),
            proposed_fee: Some(0.into()),
            ..trade(None)
        };
        assert_eq!(executed_fee(&nothing).unwrap(), Some(0));
        let buy = TradeRow {
            kind: OrderKind::Buy,
            proposed_fee: Some(7.into()),
            ..trade(None)
        };
        assert_eq!(executed_fee(&buy).unwrap(), Some(7));
        assert_eq!(executed_fee(&trade(None)).unwrap(), None);
    }

    /// The driver takes a surplus and then a volume fee from a sell fill of
    /// 1M for 1M, limit 900k. The fees recovered from the settled legs come
    /// back in policy order, each within one atom of the driver's.
    #[test]
    fn the_fees_recover_what_the_driver_took_in_policy_order() {
        let policies = [
            FeePolicy::Surplus {
                factor: 0.5,
                max_volume_factor: 0.9,
            },
            FeePolicy::Volume { factor: 0.0002 },
        ];
        let mut fill = protocol_fees::Order::<Solana> {
            uid: IntentHash([1; 32]),
            sell_token: Pubkey([2; 32]),
            buy_token: Pubkey([3; 32]),
            sell_amount: 1_000_000,
            buy_amount: 900_000,
            executed_sell: 1_000_000,
            executed_buy: 1_000_000,
            side: Side::Sell,
        };
        let taken: Vec<u64> = policies
            .iter()
            .map(|policy| {
                let fee = protocol_fees::fee_before_settlement(&fill, policy).unwrap();
                fill.executed_buy -= fee;
                fee
            })
            .collect();
        let policy_rows = [
            PolicyRow {
                auction_id: 1,
                order_uid: ByteArray([1; 32]),
                kind: PolicyKind::Surplus,
                surplus_factor: Some(0.5),
                surplus_max_volume_factor: Some(0.9),
                price_improvement_factor: None,
                price_improvement_max_volume_factor: None,
                volume_factor: None,
            },
            PolicyRow {
                auction_id: 1,
                order_uid: ByteArray([1; 32]),
                kind: PolicyKind::Volume,
                surplus_factor: None,
                surplus_max_volume_factor: None,
                price_improvement_factor: None,
                price_improvement_max_volume_factor: None,
                volume_factor: Some(0.0002),
            },
        ];
        let settled = TradeRow {
            limit_buy: 900_000.into(),
            limit_sell: 1_000_000.into(),
            executed_sell: fill.executed_sell.into(),
            executed_buy: fill.executed_buy.into(),
            ..trade(None)
        };

        let recovered = fees(&settled, &policy_rows).unwrap();
        assert_eq!(taken.len(), 2);
        assert_eq!(recovered.len(), 2);
        for ((token, recovered), taken) in recovered.into_iter().zip(taken) {
            assert_eq!(token, Pubkey([3; 32]));
            assert!(recovered.abs_diff(taken) <= 1, "{recovered} vs {taken}");
        }
    }

    /// A price improvement fee measures against the quote the auction
    /// classed the order with: the linked quote net of its cost, or the
    /// order's own limit through the zero-buy reference quote.
    #[test]
    fn a_price_improvement_fee_measures_against_the_auctions_quote() {
        let policies = [PolicyRow {
            auction_id: 1,
            order_uid: ByteArray([1; 32]),
            kind: PolicyKind::PriceImprovement,
            surplus_factor: None,
            surplus_max_volume_factor: None,
            price_improvement_factor: Some(0.5),
            price_improvement_max_volume_factor: Some(0.9),
            volume_factor: None,
        }];
        // Half of the 50 bought over the limit, before the fee took it.
        assert_eq!(
            fees(&trade(None), &policies).unwrap(),
            vec![(Pubkey([3; 32]), 50)]
        );
        // Half of the 20 bought over the quote's 140 less its cost of 10.
        assert_eq!(
            fees(&trade(Some((100, 140, 10))), &policies).unwrap(),
            vec![(Pubkey([3; 32]), 20)]
        );
    }

    /// An order `[n; 32]` of `kind` selling 10 of `[n + 0x10; 32]` for at
    /// least 10 of `[n + 0x20; 32]`.
    async fn insert_order(pool: &PgPool, n: u8, kind: &str) {
        sqlx::query(
            "INSERT INTO solana.orders (uid, owner, sell_token, buy_token, sell_token_account, \
             buy_token_account, sell_amount, buy_amount, valid_to, kind, partially_fillable, \
             app_data, creation_timestamp, order_pda) VALUES ($1, $1, $2, $3, $1, $1, 10, 10, 0, \
             $4::solana.OrderKind, false, $1, now(), $1)",
        )
        .bind([n; 32])
        .bind([n + 0x10; 32])
        .bind([n + 0x20; 32])
        .bind(kind)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn insert_volume_policy(pool: &PgPool, auction_id: i64, order: u8, factor: f64) {
        sqlx::query(
            "INSERT INTO solana.fee_policies (auction_id, order_uid, kind, volume_factor) VALUES \
             ($1, $2, 'volume', $3)",
        )
        .bind(auction_id)
        .bind([order; 32])
        .bind(factor)
        .execute(pool)
        .await
        .unwrap();
    }

    /// A settlement of the auction under signature `[signature; 64]` at the
    /// slot.
    async fn insert_settlement(pool: &PgPool, signature: u8, slot: i64, auction_id: i64) {
        sqlx::query(
            "INSERT INTO solana.settlements (slot, tx_signature, instruction_index, solver, \
             auction_id) VALUES ($1, $2, 0, $3, $4)",
        )
        .bind(slot)
        .bind([signature; 64])
        .bind([7u8; 32])
        .bind(auction_id)
        .execute(pool)
        .await
        .unwrap();
    }

    /// A trade of order `[order; 32]` in the settlement under
    /// `[signature; 64]`, selling 10 for 20 after fees.
    async fn insert_trade(pool: &PgPool, signature: u8, slot: i64, order: u8) {
        sqlx::query(
            "INSERT INTO solana.trades (tx_signature, instruction_index, order_uid, sell_amount, \
             buy_amount, fee_amount, slot) VALUES ($1, 0, $2, 10, 20, 0, $3)",
        )
        .bind([signature; 64])
        .bind([order; 32])
        .bind(slot)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn set_finalized_slot(pool: &PgPool, slot: i64) {
        sqlx::query("DELETE FROM solana.indexer_state")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO solana.indexer_state (slot, finalized_slot) VALUES ($1, $1)")
            .bind(slot)
            .execute(pool)
            .await
            .unwrap();
    }

    /// Every `solana.order_execution` row: order, auction, fee tokens and fee
    /// amounts.
    async fn executions(pool: &PgPool) -> Vec<(Vec<u8>, i64, Vec<Vec<u8>>, Vec<BigDecimal>)> {
        sqlx::query_as(
            "SELECT order_uid, auction_id, protocol_fee_tokens, protocol_fee_amounts FROM \
             solana.order_execution ORDER BY order_uid, auction_id",
        )
        .fetch_all(pool)
        .await
        .unwrap()
    }

    /// Each trade's fees per policy, in the order's surplus token: the buy
    /// token of a sell order, the sell token of a buy order. An order without
    /// policies, or with a policy row the fee math cannot read, gets an empty
    /// record. An order traded in two auctions pays each auction's own
    /// policies.
    #[tokio::test]
    #[ignore = "needs the solana.* schema applied locally, run with --test-threads 1"]
    async fn solana_db_a_final_trade_records_the_fee_each_policy_took() {
        let pool = crate::test_db::pool().await;
        crate::test_db::wipe(&pool).await;

        for (order, kind) in [(1, "sell"), (2, "buy"), (3, "sell"), (4, "sell")] {
            insert_order(&pool, order, kind).await;
        }
        insert_volume_policy(&pool, 1, 1, 0.5).await;
        insert_volume_policy(&pool, 1, 2, 0.5).await;
        sqlx::query(
            "INSERT INTO solana.fee_policies (auction_id, order_uid, kind) VALUES (1, $1, \
             'surplus')",
        )
        .bind([4u8; 32])
        .execute(&pool)
        .await
        .unwrap();
        // Every trade sells 10 for 20 after fees. Half the volume before
        // fees is 20 of a sell order's 40 bought and 3 of a buy order's 6.67
        // sold, rounded down.
        insert_settlement(&pool, 9, 10, 1).await;
        for order in [1, 2, 3, 4] {
            insert_trade(&pool, 9, 10, order).await;
        }
        insert_settlement(&pool, 8, 10, 2).await;
        insert_trade(&pool, 8, 10, 1).await;
        set_finalized_slot(&pool, 10).await;

        record(&pool).await.unwrap();

        assert_eq!(
            executions(&pool).await,
            vec![
                (
                    vec![1; 32],
                    1,
                    vec![vec![0x21; 32]],
                    vec![BigDecimal::from(20)]
                ),
                (vec![1; 32], 2, vec![], vec![]),
                (
                    vec![2; 32],
                    1,
                    vec![vec![0x12; 32]],
                    vec![BigDecimal::from(3)]
                ),
                (vec![3; 32], 1, vec![], vec![]),
                (vec![4; 32], 1, vec![], vec![]),
            ]
        );
    }

    /// Solution `uid` of auction 1, proposed by the settling solver.
    async fn insert_proposal(pool: &PgPool, uid: i64, is_winner: bool) {
        sqlx::query(
            "INSERT INTO solana.proposed_solutions (auction_id, uid, id, solver, is_winner, \
             filtered_out, score) VALUES (1, $1, $1, $2, $3, false, 0)",
        )
        .bind(uid)
        .bind([7u8; 32])
        .bind(is_winner)
        .execute(pool)
        .await
        .unwrap();
    }

    /// Order `[order; 32]` in solution `uid` of auction 1, selling 10 for
    /// `buy` after the reported `fee`.
    async fn insert_proposed_trade(pool: &PgPool, uid: i64, order: u8, buy: u64, fee: Option<u64>) {
        sqlx::query(
            "INSERT INTO solana.proposed_trade_executions (auction_id, solution_uid, order_uid, \
             executed_sell, executed_buy, fee) VALUES (1, $1, $2, 10, $3, $4)",
        )
        .bind(uid)
        .bind([order; 32])
        .bind(BigDecimal::from(buy))
        .bind(fee.map(BigDecimal::from))
        .execute(pool)
        .await
        .unwrap();
    }

    /// Each trade sells 10 for 20. A trade takes the fee of the settling
    /// solver's winning solution with the same amounts, in the sell token:
    /// a sell order's 20 bought are worth 5 sold at the rate before the fee.
    /// A trade with no such solution, or one reporting no fee, has none.
    #[tokio::test]
    #[ignore = "needs the solana.* schema applied locally, run with --test-threads 1"]
    async fn solana_db_a_final_trade_records_its_winning_proposals_fee() {
        let pool = crate::test_db::pool().await;
        crate::test_db::wipe(&pool).await;

        for (order, kind) in [
            (1, "sell"),
            (2, "buy"),
            (3, "sell"),
            (4, "sell"),
            (5, "sell"),
        ] {
            insert_order(&pool, order, kind).await;
        }
        insert_proposal(&pool, 0, true).await;
        insert_proposed_trade(&pool, 0, 1, 20, Some(20)).await;
        insert_proposed_trade(&pool, 0, 2, 20, Some(3)).await;
        insert_proposed_trade(&pool, 0, 3, 21, Some(4)).await;
        insert_proposed_trade(&pool, 0, 4, 20, None).await;
        insert_proposal(&pool, 1, false).await;
        insert_proposed_trade(&pool, 1, 5, 20, Some(6)).await;
        insert_settlement(&pool, 9, 10, 1).await;
        for order in [1, 2, 3, 4, 5] {
            insert_trade(&pool, 9, 10, order).await;
        }
        set_finalized_slot(&pool, 10).await;

        record(&pool).await.unwrap();

        let fees: Vec<(Vec<u8>, Option<String>)> = sqlx::query_as(
            "SELECT order_uid, executed_fee::text FROM solana.order_execution ORDER BY order_uid",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(
            fees,
            vec![
                (vec![1; 32], Some("5".to_string())),
                (vec![2; 32], Some("3".to_string())),
                (vec![3; 32], None),
                (vec![4; 32], None),
                (vec![5; 32], None),
            ]
        );
    }

    /// A trade past the finalized slot waits for finality, and a trade
    /// indexed into an already recorded settlement is picked up by the next
    /// sweep. Recorded rows stay as they are.
    #[tokio::test]
    #[ignore = "needs the solana.* schema applied locally, run with --test-threads 1"]
    async fn solana_db_the_sweep_records_each_final_trade_once() {
        let pool = crate::test_db::pool().await;
        crate::test_db::wipe(&pool).await;

        for order in [1, 2, 3] {
            insert_order(&pool, order, "sell").await;
        }
        insert_volume_policy(&pool, 1, 1, 0.5).await;
        insert_volume_policy(&pool, 2, 2, 0.5).await;
        insert_settlement(&pool, 9, 10, 1).await;
        insert_trade(&pool, 9, 10, 1).await;
        insert_settlement(&pool, 8, 11, 2).await;
        insert_trade(&pool, 8, 11, 2).await;
        set_finalized_slot(&pool, 10).await;

        record(&pool).await.unwrap();
        let first = (
            vec![1; 32],
            1,
            vec![vec![0x21; 32]],
            vec![BigDecimal::from(20)],
        );
        assert_eq!(executions(&pool).await, vec![first.clone()]);

        insert_trade(&pool, 9, 10, 3).await;
        set_finalized_slot(&pool, 11).await;
        record(&pool).await.unwrap();
        assert_eq!(
            executions(&pool).await,
            vec![
                first,
                (
                    vec![2; 32],
                    2,
                    vec![vec![0x22; 32]],
                    vec![BigDecimal::from(20)]
                ),
                (vec![3; 32], 1, vec![], vec![]),
            ]
        );
    }
}
