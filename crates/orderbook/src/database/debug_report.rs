use {
    super::Postgres,
    crate::database::orders::OrderStoring,
    alloy::primitives::{Address, B256},
    anyhow::{Context, Result},
    database::{
        auction::{self, Auction as DbAuction, AuctionId},
        byte_array::ByteArray,
        fee_policies::{self, FeePolicy as DbFeePolicy, FeePolicyKind as DbFeePolicyKind},
        order_events,
        order_execution::{self, OrderExecution as DbOrderExecution},
        settlement_executions::{self, SettlementExecution as DbSettlementExecution},
        solver_competition_v2::{self, OrderProposedSolution as DbProposedSolution},
        trades::{self, TradesQueryRow as DbTradesQueryRow},
    },
    model::{
        debug_report::{
            Auction,
            DebugReport,
            Event,
            Execution,
            FeePolicy,
            FeePolicyKind,
            ProposedSolution,
            ProtocolFee,
            SettlementAttempt,
            Trade,
        },
        order::OrderUid,
    },
    std::collections::HashMap,
};

impl Postgres {
    pub async fn fetch_debug_report(&self, uid: &OrderUid) -> Result<Option<DebugReport>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["fetch_debug_report"])
            .start_timer();

        let db_uid = ByteArray(uid.0);
        let order = match self.single_order(uid).await? {
            Some(order) => order,
            None => return Ok(None),
        };

        let mut conn = self.pool.acquire().await?;

        let events: Vec<Event> = order_events::get_all(&mut conn, &db_uid)
            .await?
            .into_iter()
            .map(|e| Event {
                label: format!("{:?}", e.label).to_lowercase(),
                timestamp: e.timestamp,
                reason: e.reason.map(|r| r.to_string()),
            })
            .collect();

        let proposed_solutions =
            solver_competition_v2::find_solutions_for_order(&mut conn, &db_uid).await?;
        let executions = order_execution::read_by_order_uid(&mut conn, &db_uid).await?;
        let trades: Vec<DbTradesQueryRow> = trades::trades(&mut conn, None, Some(&db_uid), 0, 100)
            .into_inner()
            .await
            .context("failed to fetch trades")?;

        let auction_ids: Vec<AuctionId> =
            auction::fetch_auction_ids_by_order_uid(&mut conn, &db_uid).await?;

        let db_auctions = auction::fetch_multiple(&mut conn, &auction_ids).await?;
        let settlement_execs =
            settlement_executions::read_by_auction_ids(&mut conn, &auction_ids).await?;
        let fee_policies = fee_policies::fetch_by_order_uid(&mut conn, &db_uid).await?;

        let sell_token = order.data.sell_token;
        let buy_token = order.data.buy_token;
        let auctions = build_auctions(
            db_auctions,
            proposed_solutions,
            executions,
            settlement_execs,
            fee_policies,
            sell_token,
            buy_token,
        );

        Ok(Some(DebugReport {
            order_uid: *uid,
            order,
            events,
            auctions,
            trades: trades.into_iter().map(convert_trade).collect(),
        }))
    }
}

fn build_auctions(
    db_auctions: Vec<DbAuction>,
    db_solutions: Vec<DbProposedSolution>,
    db_executions: Vec<DbOrderExecution>,
    db_settlements: Vec<DbSettlementExecution>,
    db_fees: Vec<DbFeePolicy>,
    sell_token: Address,
    buy_token: Address,
) -> Vec<Auction> {
    let sell = ByteArray(sell_token.0.0);
    let buy = ByteArray(buy_token.0.0);

    let mut solutions_by_auction: HashMap<i64, Vec<ProposedSolution>> = HashMap::new();
    for s in db_solutions {
        solutions_by_auction
            .entry(s.auction_id)
            .or_default()
            .push(convert_solution(s));
    }

    let mut executions_by_auction: HashMap<i64, Vec<Execution>> = HashMap::new();
    for e in db_executions {
        executions_by_auction
            .entry(e.auction_id)
            .or_default()
            .push(convert_execution(e));
    }

    let mut settlements_by_auction: HashMap<i64, Vec<SettlementAttempt>> = HashMap::new();
    for s in db_settlements {
        settlements_by_auction
            .entry(s.auction_id)
            .or_default()
            .push(convert_settlement(s));
    }

    let mut fees_by_auction: HashMap<i64, Vec<FeePolicy>> = HashMap::new();
    for f in db_fees {
        fees_by_auction
            .entry(f.auction_id)
            .or_default()
            .push(convert_fee_policy(f));
    }

    let mut auctions: Vec<Auction> = db_auctions
        .into_iter()
        .map(|a| {
            let native_prices = a
                .price_tokens
                .iter()
                .zip(&a.price_values)
                .filter(|(token, _)| **token == sell || **token == buy)
                .map(|(token, price)| (Address::from(token.0), price.clone()))
                .collect();
            let id = a.id;
            Auction {
                id,
                block: a.block,
                deadline: a.deadline,
                native_prices,
                proposed_solutions: solutions_by_auction.remove(&id).unwrap_or_default(),
                executions: executions_by_auction.remove(&id).unwrap_or_default(),
                settlement_attempts: settlements_by_auction.remove(&id).unwrap_or_default(),
                fee_policies: fees_by_auction.remove(&id).unwrap_or_default(),
            }
        })
        .collect();

    auctions.sort_by_key(|a| a.id);
    auctions
}

fn convert_solution(s: DbProposedSolution) -> ProposedSolution {
    ProposedSolution {
        solution_uid: s.solution_uid,
        ranking: s.solution_uid + 1,
        solver: Address::from(s.solver.0),
        is_winner: s.is_winner,
        filtered_out: s.filtered_out,
        score: s.score,
        executed_sell: s.executed_sell,
        executed_buy: s.executed_buy,
    }
}

fn convert_execution(e: DbOrderExecution) -> Execution {
    let protocol_fees = e
        .protocol_fee_tokens
        .iter()
        .zip(e.protocol_fee_amounts.iter())
        .map(|(token, amount)| ProtocolFee {
            token: Address::from(token.0),
            amount: amount.clone(),
        })
        .collect();
    Execution {
        executed_fee: e.executed_fee,
        executed_fee_token: Address::from(e.executed_fee_token.0),
        block_number: e.block_number,
        protocol_fees,
    }
}

fn convert_trade(t: DbTradesQueryRow) -> Trade {
    Trade {
        block_number: t.block_number,
        log_index: t.log_index,
        buy_amount: t.buy_amount,
        sell_amount: t.sell_amount,
        sell_amount_before_fees: t.sell_amount_before_fees,
        tx_hash: t.tx_hash.map(|h| B256::from(h.0)),
        auction_id: t.auction_id,
    }
}

fn convert_settlement(s: DbSettlementExecution) -> SettlementAttempt {
    SettlementAttempt {
        solver: Address::from(s.solver.0),
        solution_uid: s.solution_uid,
        start_timestamp: s.start_timestamp,
        end_timestamp: s.end_timestamp,
        start_block: s.start_block,
        end_block: s.end_block,
        deadline_block: s.deadline_block,
        outcome: s.outcome,
    }
}

fn convert_fee_policy(f: DbFeePolicy) -> FeePolicy {
    FeePolicy {
        kind: match f.kind {
            DbFeePolicyKind::Surplus => FeePolicyKind::Surplus,
            DbFeePolicyKind::Volume => FeePolicyKind::Volume,
            DbFeePolicyKind::PriceImprovement => FeePolicyKind::PriceImprovement,
        },
        surplus_factor: f.surplus_factor,
        surplus_max_volume_factor: f.surplus_max_volume_factor,
        volume_factor: f.volume_factor,
        price_improvement_factor: f.price_improvement_factor,
        price_improvement_max_volume_factor: f.price_improvement_max_volume_factor,
    }
}
