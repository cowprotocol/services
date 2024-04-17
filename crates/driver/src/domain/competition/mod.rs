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
            notify,
            observe,
            solver::{self, SolutionMerging, Solver},
            Simulator,
        },
        util::Bytes,
    },
    futures::{stream::FuturesUnordered, StreamExt},
    itertools::Itertools,
    std::{
        cmp::Reverse,
        collections::{HashMap, HashSet},
        sync::Mutex,
    },
    tap::TapFallible,
};

pub mod auction;
pub mod order;
pub mod solution;

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
    pub settlement: Mutex<Option<Settlement>>,
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
            if solution.is_empty() {
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
                let settlement = solution.encode(auction, &self.eth, &self.simulator).await;
                (id, settlement)
            })
            .collect::<FuturesUnordered<_>>()
            .filter_map(|(id, result)| async move {
                result
                    .tap_err(|err| {
                        observe::encoding_failed(self.solver.name(), &id, err);
                        notify::encoding_failed(&self.solver, auction.id(), &id, err);
                    })
                    .ok()
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
                (settlement.score(&auction.prices()), settlement)
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
                        score,
                        trades: settlement.orders(),
                        prices: settlement.prices(),
                        gas: Some(settlement.gas.estimate),
                    },
                    settlement,
                )
            })
            .unzip();

        *self.settlement.lock().unwrap() = settlement.clone();

        let settlement = match settlement {
            Some(settlement) => settlement,
            // Don't wait for the deadline because we can't produce a solution anyway.
            None => return Ok(score),
        };

        // Re-simulate the solution on every new block until the deadline ends to make
        // sure we actually submit a working solution close to when the winner
        // gets picked by the procotol.
        if let Ok(remaining) = auction.deadline().driver().remaining() {
            let score_ref = &mut score;
            let simulate_on_new_blocks = async move {
                let mut stream =
                    ethrpc::current_block::into_stream(self.eth.current_block().clone());
                while let Some(block) = stream.next().await {
                    if let Err(infra::simulator::Error::Revert(err)) =
                        self.simulate_settlement(&settlement).await
                    {
                        observe::winner_voided(block, &err);
                        *score_ref = None;
                        *self.settlement.lock().unwrap() = None;
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

    pub async fn reveal(&self) -> Result<Revealed, Error> {
        let settlement = self
            .settlement
            .lock()
            .unwrap()
            .as_ref()
            .cloned()
            .ok_or(Error::SolutionNotAvailable)?;
        Ok(Revealed {
            internalized_calldata: settlement
                .calldata(
                    self.eth.contracts().settlement(),
                    settlement::Internalization::Enable,
                )
                .into(),
            uninternalized_calldata: settlement
                .calldata(
                    self.eth.contracts().settlement(),
                    settlement::Internalization::Disable,
                )
                .into(),
        })
    }

    /// Execute the solution generated as part of this competition. Use
    /// [`Competition::solve`] to generate the solution.
    pub async fn settle(&self) -> Result<Settled, Error> {
        let settlement = self
            .settlement
            .lock()
            .unwrap()
            .take()
            .ok_or(Error::SolutionNotAvailable)?;

        let executed = self.mempools.execute(&self.solver, &settlement).await;
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
                    .calldata(
                        self.eth.contracts().settlement(),
                        settlement::Internalization::Enable,
                    )
                    .into(),
                uninternalized_calldata: settlement
                    .calldata(
                        self.eth.contracts().settlement(),
                        settlement::Internalization::Disable,
                    )
                    .into(),
                tx_hash,
            }),
        }
    }

    /// The ID of the auction being competed on.
    pub fn auction_id(&self) -> Option<auction::Id> {
        self.settlement
            .lock()
            .unwrap()
            .as_ref()
            .map(|s| s.auction_id)
    }

    /// Returns whether the settlement can be executed or would revert.
    async fn simulate_settlement(
        &self,
        settlement: &Settlement,
    ) -> Result<(), infra::simulator::Error> {
        self.simulator
            .gas(eth::Tx {
                from: self.solver.address(),
                to: settlement.solver(),
                value: eth::Ether(0.into()),
                input: crate::util::Bytes(settlement.calldata(
                    self.eth.contracts().settlement(),
                    settlement::Internalization::Enable,
                )),
                access_list: settlement.access_list.clone(),
            })
            .await
            .map(|_| ())
    }
}

/// Creates a vector with all possible combinations of the given solutions.
/// The result is sorted descending by score.
fn merge(solutions: impl Iterator<Item = Solution>, auction: &Auction) -> Vec<Solution> {
    let mut merged: Vec<Solution> = Vec::new();
    for solution in solutions {
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
                .scoring(&auction.prices())
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
    pub score: eth::Ether,
    pub trades: HashMap<order::Uid, Amounts>,
    pub prices: HashMap<eth::TokenAddress, eth::TokenAmount>,
    pub gas: Option<eth::Gas>,
}

#[derive(Debug, Default)]
pub struct Amounts {
    pub sell: eth::TokenAmount,
    pub buy: eth::TokenAmount,
}

#[derive(Debug)]
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
}
