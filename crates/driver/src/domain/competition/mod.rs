use {
    self::solution::settlement,
    super::{
        time::{self, Remaining},
        Mempools,
    },
    crate::{
        domain::{competition::solution::Settlement, eth},
        infra::{
            self,
            blockchain::Ethereum,
            notify, observe,
            simulator::{RevertError, SimulatorError},
            solver::{self, SolutionMerging, Solver},
            Simulator,
        },
        util::Bytes,
    },
    futures::{stream::FuturesUnordered, StreamExt},
    itertools::Itertools,
    std::{
        cmp::Reverse,
        collections::{HashMap, HashSet, VecDeque},
        sync::Mutex,
    },
    tap::TapFallible,
};

pub mod auction;
pub mod order;
pub mod solution;
mod sorting;

pub use {
    auction::{Auction, AuctionProcessor},
    order::Order,
    solution::Solution,
};

/// An ongoing competition. There is one competition going on per solver at any
/// time. The competition stores settlements to solutions generated by the
/// driver, and allows them to be executed onchain when requested later. The
/// solutions expire after a certain amount of time, at which point trying to
/// use them will return an `[Error::InvalidSolutionId]`.
#[derive(Debug)]
pub struct Competition {
    pub solver: Solver,
    pub eth: Ethereum,
    pub liquidity: infra::liquidity::Fetcher,
    pub simulator: Simulator,
    pub mempools: Mempools,
    /// Cached solutions with the most recent solutions at the front.
    pub settlements: Mutex<VecDeque<Settlement>>,
}

impl Competition {
    /// Solve an auction as part of this competition.
    pub async fn solve(&self, auction: &Auction) -> Result<Option<Solved>, Error> {
        let liquidity = match self.solver.liquidity() {
            solver::Liquidity::Fetch => {
                self.liquidity
                    .fetch(
                        &auction.liquidity_pairs(),
                        infra::liquidity::AtBlock::Latest,
                    )
                    .await
            }
            solver::Liquidity::Skip => Default::default(),
        };

        // Fetch the solutions from the solver.
        let solutions = self
            .solver
            .solve(auction, &liquidity)
            .await
            .tap_err(|err| {
                if err.is_timeout() {
                    notify::solver_timeout(&self.solver, auction.id());
                }
            })?;

        observe::postprocessing(&solutions, auction.deadline().driver());

        // Discard solutions that don't have unique ID.
        let mut ids = HashSet::new();
        let solutions = solutions.into_iter().filter(|solution| {
            if !ids.insert(solution.id().clone()) {
                observe::duplicated_solution_id(self.solver.name(), solution.id());
                notify::duplicated_solution_id(&self.solver, auction.id(), solution.id());
                false
            } else {
                true
            }
        });

        // Discard empty solutions.
        let solutions = solutions.filter(|solution| {
            if solution.is_empty(auction.surplus_capturing_jit_order_owners()) {
                observe::empty_solution(self.solver.name(), solution.id());
                notify::empty_solution(&self.solver, auction.id(), solution.id().clone());
                false
            } else {
                true
            }
        });

        let all_solutions = match self.solver.solution_merging() {
            SolutionMerging::Allowed => merge(solutions, auction),
            SolutionMerging::Forbidden => solutions.collect(),
        };

        // Encode solutions into settlements (streamed).
        let encoded = all_solutions
            .into_iter()
            .map(|solution| async move {
                let id = solution.id().clone();
                observe::encoding(&id);
                let settlement = solution
                    .encode(
                        auction,
                        &self.eth,
                        &self.simulator,
                        self.solver.solver_native_token(),
                    )
                    .await;
                (id, settlement)
            })
            .collect::<FuturesUnordered<_>>()
            .filter_map(|(id, result)| async move {
                match result {
                    Ok(solution) => Some(solution),
                    // don't report on errors coming from solution merging
                    Err(_err) if id.solutions().len() > 1 => None,
                    Err(err) => {
                        observe::encoding_failed(self.solver.name(), &id, &err);
                        notify::encoding_failed(&self.solver, auction.id(), &id, &err);
                        None
                    }
                }
            });

        // Encode settlements as they arrive until there are no more new settlements or
        // timeout is reached.
        let mut settlements = Vec::new();
        let future = async {
            let mut encoded = std::pin::pin!(encoded);
            while let Some(settlement) = encoded.next().await {
                settlements.push(settlement);
            }
        };
        if tokio::time::timeout(
            auction.deadline().driver().remaining().unwrap_or_default(),
            future,
        )
        .await
        .is_err()
        {
            observe::postprocessing_timed_out(&settlements);
            notify::postprocessing_timed_out(&self.solver, auction.id())
        }

        // Score the settlements.
        let scores = settlements
            .into_iter()
            .map(|settlement| {
                observe::scoring(&settlement);
                (
                    settlement.score(
                        &auction.prices(),
                        auction.surplus_capturing_jit_order_owners(),
                    ),
                    settlement,
                )
            })
            .collect_vec();

        // Filter out settlements which failed scoring.
        let scores = scores
            .into_iter()
            .filter_map(|(result, settlement)| {
                result
                    .tap_err(|err| {
                        observe::scoring_failed(self.solver.name(), err);
                        notify::scoring_failed(
                            &self.solver,
                            auction.id(),
                            settlement.solution(),
                            err,
                        );
                    })
                    .ok()
                    .map(|score| (score, settlement))
            })
            .collect_vec();

        // Observe the scores.
        for (score, settlement) in scores.iter() {
            observe::score(settlement, score);
        }

        // Pick the best-scoring settlement.
        let (mut score, settlement) = scores
            .into_iter()
            .max_by_key(|(score, _)| score.to_owned())
            .map(|(score, settlement)| {
                (
                    Solved {
                        id: settlement.solution().clone(),
                        score,
                        trades: settlement.orders(),
                        prices: settlement.prices(),
                        gas: Some(settlement.gas.estimate),
                    },
                    settlement,
                )
            })
            .unzip();

        let Some(settlement) = settlement else {
            // Don't wait for the deadline because we can't produce a solution anyway.
            return Ok(score);
        };
        let solution_id = settlement.solution().get();

        {
            let mut lock = self.settlements.lock().unwrap();
            lock.push_front(settlement.clone());

            /// Number of solutions that may be cached at most.
            const MAX_SOLUTION_STORAGE: usize = 5;
            lock.truncate(MAX_SOLUTION_STORAGE);
        }

        // Re-simulate the solution on every new block until the deadline ends to make
        // sure we actually submit a working solution close to when the winner
        // gets picked by the procotol.
        if let Ok(remaining) = auction.deadline().driver().remaining() {
            let score_ref = &mut score;
            let simulate_on_new_blocks = async move {
                let mut stream =
                    ethrpc::block_stream::into_stream(self.eth.current_block().clone());
                while let Some(block) = stream.next().await {
                    if let Err(infra::simulator::Error::Revert(err)) =
                        self.simulate_settlement(&settlement).await
                    {
                        observe::winner_voided(block, &err);
                        *score_ref = None;
                        self.settlements
                            .lock()
                            .unwrap()
                            .retain(|s| s.solution().get() != solution_id);
                        notify::simulation_failed(
                            &self.solver,
                            auction.id(),
                            settlement.solution(),
                            &infra::simulator::Error::Revert(err),
                            true,
                        );
                        return;
                    }
                }
            };
            let _ = tokio::time::timeout(remaining, simulate_on_new_blocks).await;
        }

        Ok(score)
    }

    pub async fn reveal(&self, solution_id: u64) -> Result<Revealed, Error> {
        let settlement = self
            .settlements
            .lock()
            .unwrap()
            .iter()
            .find(|s| s.solution().get() == solution_id)
            .cloned()
            .ok_or(Error::SolutionNotAvailable)?;
        Ok(Revealed {
            internalized_calldata: settlement
                .transaction(settlement::Internalization::Enable)
                .input
                .clone(),
            uninternalized_calldata: settlement
                .transaction(settlement::Internalization::Disable)
                .input
                .clone(),
        })
    }

    /// Execute the solution generated as part of this competition. Use
    /// [`Competition::solve`] to generate the solution.
    pub async fn settle(
        &self,
        auction_id: i64,
        solution_id: u64,
        submission_deadline: u64,
    ) -> Result<Settled, Error> {
        let settlement = {
            let mut lock = self.settlements.lock().unwrap();
            let index = lock
                .iter()
                .position(|s| s.solution().get() == solution_id)
                .ok_or(Error::SolutionNotAvailable)?;
            // remove settlement to ensure we can't settle it twice by accident
            lock.swap_remove_front(index)
                .ok_or(Error::SolutionNotAvailable)?
        };

        if auction_id != settlement.auction_id.0 {
            return Err(Error::SolutionIdMismatchedAuctionId);
        }

        let executed = self
            .mempools
            .execute(&self.solver, &settlement, submission_deadline)
            .await;
        notify::executed(
            &self.solver,
            settlement.auction_id,
            settlement.solution(),
            &executed,
        );

        match executed {
            Err(_) => Err(Error::SubmissionError),
            Ok(tx_hash) => Ok(Settled {
                internalized_calldata: settlement
                    .transaction(settlement::Internalization::Enable)
                    .input
                    .clone(),
                uninternalized_calldata: settlement
                    .transaction(settlement::Internalization::Disable)
                    .input
                    .clone(),
                tx_hash,
            }),
        }
    }

    /// The ID of the auction being competed on.
    pub fn auction_id(&self, solution_id: u64) -> Option<auction::Id> {
        self.settlements
            .lock()
            .unwrap()
            .iter()
            .find(|s| s.solution().get() == solution_id)
            .map(|s| s.auction_id)
    }

    /// Returns whether the settlement can be executed or would revert.
    async fn simulate_settlement(
        &self,
        settlement: &Settlement,
    ) -> Result<(), infra::simulator::Error> {
        let tx = settlement.transaction(settlement::Internalization::Enable);
        let gas_needed_for_tx = self.simulator.gas(tx).await?;
        if gas_needed_for_tx > settlement.gas.limit {
            return Err(infra::simulator::Error::Revert(RevertError {
                err: SimulatorError::GasExceeded(gas_needed_for_tx, settlement.gas.limit),
                tx: tx.clone(),
                block: self.eth.current_block().borrow().number.into(),
            }));
        }
        Ok(())
    }
}

const MAX_SOLUTIONS_TO_MERGE: usize = 10;

/// Creates a vector with all possible combinations of the given solutions.
/// The result is sorted descending by score.
fn merge(solutions: impl Iterator<Item = Solution>, auction: &Auction) -> Vec<Solution> {
    let mut merged: Vec<Solution> = Vec::new();
    // Limit the number of solutions to merge to avoid combinatorial explosion
    // (2^MAX_SOLUTIONS).
    for solution in solutions.take(MAX_SOLUTIONS_TO_MERGE) {
        let mut extension = vec![];
        for already_merged in merged.iter() {
            match solution.merge(already_merged) {
                Ok(merged) => {
                    observe::merged(&solution, already_merged, &merged);
                    extension.push(merged);
                }
                Err(err) => {
                    observe::not_merged(&solution, already_merged, err);
                }
            }
        }
        // At least insert the current solution
        extension.push(solution);
        merged.extend(extension);
    }

    // Sort merged solutions descending by score.
    merged.sort_by_key(|solution| {
        Reverse(
            solution
                .scoring(
                    &auction.prices(),
                    auction.surplus_capturing_jit_order_owners(),
                )
                .map(|score| score.0)
                .unwrap_or_default(),
        )
    });
    merged
}

/// Solution information sent to the protocol by the driver before the solution
/// ranking happens.
#[derive(Debug)]
pub struct Solved {
    pub id: solution::Id,
    pub score: eth::Ether,
    pub trades: HashMap<order::Uid, Amounts>,
    pub prices: HashMap<eth::TokenAddress, eth::TokenAmount>,
    pub gas: Option<eth::Gas>,
}

#[derive(Debug)]
pub struct Amounts {
    pub side: order::Side,
    /// The sell token and limit sell amount of sell token.
    pub sell: eth::Asset,
    /// The buy token and limit buy amount of buy token.
    pub buy: eth::Asset,
    /// The effective amount that left the user's wallet including all fees.
    pub executed_sell: eth::TokenAmount,
    /// The effective amount the user received after all fees.
    pub executed_buy: eth::TokenAmount,
}

#[derive(Clone, Debug)]
pub struct PriceLimits {
    pub sell: eth::TokenAmount,
    pub buy: eth::TokenAmount,
}

/// Winning solution information revealed to the protocol by the driver before
/// the onchain settlement happens. Calldata is first time revealed at this
/// point.
#[derive(Debug)]
pub struct Revealed {
    /// The internalized calldata is the final calldata that appears onchain.
    pub internalized_calldata: Bytes<Vec<u8>>,
    /// The uninternalized calldata must be known so that the CoW solver team
    /// can manually enforce certain rules which can not be enforced
    /// automatically.
    pub uninternalized_calldata: Bytes<Vec<u8>>,
}

#[derive(Debug)]
pub struct Settled {
    /// The transaction hash in which the solution was submitted.
    pub tx_hash: eth::TxId,
    pub internalized_calldata: Bytes<Vec<u8>>,
    /// The uninternalized calldata must be known so that the CoW solver team
    /// can manually enforce certain rules which can not be enforced
    /// automatically.
    pub uninternalized_calldata: Bytes<Vec<u8>>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(
        "no solution is available yet, this might mean that /settle was called before /solve \
         returned"
    )]
    SolutionNotAvailable,
    #[error("{0:?}")]
    DeadlineExceeded(#[from] time::DeadlineExceeded),
    #[error("solver error: {0:?}")]
    Solver(#[from] solver::Error),
    #[error("failed to submit the solution")]
    SubmissionError,
    #[error("solution ID cannot be found for provided auction ID")]
    SolutionIdMismatchedAuctionId,
}
