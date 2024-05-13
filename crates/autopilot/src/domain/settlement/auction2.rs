//! Auction data related to the specific settlement.

use {
    crate::{
        domain::{self, eth, settlement},
        infra,
    },
    std::collections::{HashMap, HashSet},
};

/// Offchain data related to a specific settlement and the auction this
/// settlement belongs to.
pub struct Auction {
    /// Onchain observed settlement.
    pub settlement: settlement::Tx,
    /// Auction external prices
    pub prices: domain::auction::Prices,
    /// Competition winner (solver submission address).
    pub winner: eth::Address,
    /// Winning score promised during competition (based on the promised `competition::Solution`)
    pub winner_score: eth::U256,
    /// Winning solution promised during competition.
    pub winner_solution: competition::Solution,
    /// Settlement should appear onchain before this block.
    pub deadline: eth::BlockNo,
    /// Settlement orders that are missing from the orderbook (JIT orders).
    pub missing_orders: Vec<domain::OrderUid>,
    /// Fee policies for all settled orders
    pub fee_policies: HashMap<domain::OrderUid, Vec<domain::fee::Policy>>,
}

impl Auction {
    /// Returns a list of violated rules.
    pub fn violations(&self, eth: &infra::Ethereum) -> Vec<Violation> {
        let mut violations = Vec::new();

        // Rule 1: The winner settled the settlement.
        if self.settlement.solver() != self.winner {
            violations.push(Violation::WinnerSettled);
        }

        // Rule 2: Settlement settled before deadline.
        if self.settlement.block() > self.deadline {
            violations.push(Violation::Deadline);
        }

        // Settlement settled onchain
        let delivered_settlement = self.settlement.settlement();
        // Settlement promised during competition, as an answer to the /reveal endpoint.
        let promised_settlement = domain::settlement::Settlement::new(
            &self.winner_calldata,
            eth.contracts().settlement_domain_separator(),
        );

        // Rule 3: Promised settlement must be recoverable.
        let promised_settlement = match promised_settlement {
            Ok(settlement) => settlement,
            Err(err) => {
                violations.push(Violation::RecoverablePromisedSettlement(err));
                return violations;
            }
        };

        // Quick check if delivered settlement is the same as promised.
        if delivered_settlement == &promised_settlement {
            // No further checks needed if this passed.
            return violations;
        }

        // Rule 4: Equal auction ids
        if delivered_settlement.auction_id() != promised_settlement.auction_id() {
            violations.push(Violation::EqualAuctionIds);
        }

        // Rule 5: Delivered same orders as promised.
        let delivered_orders = delivered_settlement.order_uids().collect::<HashSet<_>>();
        let promised_orders = promised_settlement.order_uids().collect::<HashSet<_>>();
        if delivered_orders != promised_orders {
            violations.push(Violation::EqualOrders);
        }

        // Rule 6: Delivered score must be recoverable and equal.
        let delivered_score = delivered_settlement.score(&self.prices, &self.fee_policies);
        let promised_score = promised_settlement.score(&self.prices, &self.fee_policies);
        match (delivered_score, promised_score) {
            (Ok(delivered_score), Ok(promised_score)) => {
                if delivered_score != promised_score {
                    violations.push(Violation::EqualScores);
                }
            }
            (Err(err), _) => {
                violations.push(Violation::RecoverableDeliveredScore(err));
            }
            (_, Err(err)) => {
                violations.push(Violation::RecoverablePromisedScore(err));
            }
        }

        // Rule 7: Equal traded amounts
        todo!("check traded amounts");

        // Rule 8: Equal clearing prices for all trades
        todo!("check clearing prices");

        violations
    }
}

pub enum Violation {
    WinnerSettled,
    Deadline,
    RecoverablePromisedSettlement(domain::settlement::Error),
    EqualAuctionIds,
    EqualOrders,
    EqualScores,
    RecoverableDeliveredScore(domain::settlement::trade::Error),
    RecoverablePromisedScore(domain::settlement::trade::Error),
}
