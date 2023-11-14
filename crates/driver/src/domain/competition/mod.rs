use {
    self::solution::settlement,
    super::Mempools,
    crate::{
        domain::{competition::solution::Settlement, eth},
        infra::{
            self,
            blockchain::Ethereum,
            notify,
            observe,
            solver::{self, Solver},
            Simulator,
        },
        util::Bytes,
    },
    futures::StreamExt,
    itertools::Itertools,
    rand::seq::SliceRandom,
    std::{collections::HashSet, sync::Mutex},
    tap::TapFallible,
};

pub mod auction;
pub mod order;
pub mod score;
pub mod solution;

pub use {
    auction::{Auction, AuctionProcessor},
    order::Order,
    score::{
        risk::{ObjectiveValue, SuccessProbability},
        Score,
    },
    solution::{Solution, SolverScore, SolverTimeout},
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
            .solve(auction, &liquidity, auction.deadline().timeout()?)
            .await
            .tap_err(|err| {
                if err.is_timeout() {
                    notify::solver_timeout(&self.solver, auction.id());
                }
            })?;

        // Discard solutions that don't have unique ID.
        let mut ids = HashSet::new();
        let solutions = solutions.into_iter().filter(|solution| {
            if !ids.insert(solution.id()) {
                observe::duplicated_solution_id(self.solver.name(), solution.id());
                notify::duplicated_solution_id(&self.solver, auction.id(), solution.id());
                false
            } else {
                true
            }
        });

        // Empty solutions aren't useful, so discard them.
        let solutions = solutions
            .filter(|solution| {
                if solution.is_empty() {
                    observe::empty_solution(self.solver.name(), solution.id());
                    notify::empty_solution(&self.solver, auction.id(), solution.id());
                    false
                } else {
                    true
                }
            })
            .collect();

        // Encode the solutions into settlements.
        let settlements = self
            .encode(
                auction.clone(),
                self.eth.clone(),
                self.simulator.clone(),
                solutions,
            )
            .await;
        // let settlements = join_all(solutions.map(|solution| async move {
        //     observe::encoding(solution.id());
        //     (
        //         solution.id(),
        //         solution.encode(auction, &self.eth, &self.simulator).await,
        //     )
        // }))
        // .await;

        // Filter out solutions that failed to encode.
        let mut settlements = settlements
            .into_iter()
            .filter_map(|(id, result)| {
                result
                    .tap_err(|err| {
                        observe::encoding_failed(self.solver.name(), id, err);
                        notify::encoding_failed(&self.solver, auction.id(), id, err);
                    })
                    .ok()
            })
            .collect_vec();

        // TODO(#1483): parallelize this
        // TODO(#1480): more optimal approach for settlement merging

        // Merge the settlements in random order.
        settlements.shuffle(&mut rand::thread_rng());

        // The merging algorithm works as follows: the [`settlements`] vector keeps the
        // "most merged" settlements until they can't be merged anymore, at
        // which point they are moved into the [`results`] vector.

        // The merged settlements in their final form.
        let mut results = Vec::new();
        while let Some(settlement) = settlements.pop() {
            // Has [`settlement`] been merged into another settlement?
            let mut merged = false;
            // Try to merge [`settlement`] into some other settlement.
            for other in settlements.iter_mut() {
                match other.merge(&settlement, &self.eth, &self.simulator).await {
                    Ok(m) => {
                        *other = m;
                        merged = true;
                        observe::merged(&settlement, other);
                        break;
                    }
                    Err(err) => {
                        observe::not_merged(&settlement, other, err);
                    }
                }
            }
            // If [`settlement`] can't be merged into any other settlement, this is its
            // final, most optimal form. Push it to the results.
            if !merged {
                results.push(settlement);
            }
        }

        let settlements = results;

        // Score the settlements.
        let scores = settlements
            .into_iter()
            .map(|settlement| {
                observe::scoring(&settlement);
                (
                    settlement.score(&self.eth, auction, &self.mempools.revert_protection()),
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
                            settlement.notify_id(),
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
            .map(|(score, settlement)| (Solved { score }, settlement))
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
        if let Ok(deadline) = auction.deadline().timeout() {
            let score_ref = &mut score;
            let simulate_on_new_blocks = async move {
                let mut stream =
                    ethrpc::current_block::into_stream(self.eth.current_block().clone());
                while let Some(block) = stream.next().await {
                    if let Err(err) = self.simulate_settlement(&settlement).await {
                        tracing::warn!(block = block.number, ?err, "solution reverts on new block");
                        *score_ref = None;
                        *self.settlement.lock().unwrap() = None;
                        return;
                    }
                }
            };
            let timeout = deadline.duration().to_std().unwrap_or_default();
            let _ = tokio::time::timeout(timeout, simulate_on_new_blocks).await;
        }

        Ok(score)
    }

    /// Encode the solutions into settlements.
    async fn encode(
        &self,
        auction: Auction,
        eth: Ethereum,
        simulator: Simulator,
        solutions: Vec<Solution>,
    ) -> Vec<(solution::Id, Result<Settlement, solution::Error>)> {
        let deadline = match auction.deadline().timeout().and_then(|timeout| {
            timeout
                .duration()
                .to_std()
                .map_err(|_| solution::DeadlineExceeded)
        }) {
            Ok(deadline) => deadline,
            Err(_) => {
                tracing::debug!("deadline exceeded");
                return Vec::new();
            }
        };

        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let background_work = async move {
            for solution in solutions {
                let id = solution.id();
                observe::encoding(id);
                let settlement = solution.encode(&auction, &eth, &simulator).await;
                let _ = sender.send((id, settlement));
            }
        };

        if tokio::time::timeout(deadline, tokio::spawn(background_work))
            .await
            .is_err()
        {
            tracing::debug!("reached timeout while encoding");
        }

        let mut solutions = vec![];
        while let Ok(solution) = receiver.try_recv() {
            solutions.push(solution);
        }
        solutions
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
            orders: settlement.orders(),
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
            settlement.notify_id(),
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

/// Solution information sent to the protocol by the driver before the solution
/// ranking happens.
#[derive(Debug)]
pub struct Solved {
    pub score: Score,
}

/// Winning solution information revealed to the protocol by the driver before
/// the onchain settlement happens. Calldata is first time revealed at this
/// point.
#[derive(Debug)]
pub struct Revealed {
    /// The orders solved by this solution.
    pub orders: HashSet<order::Uid>,
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
    DeadlineExceeded(#[from] solution::DeadlineExceeded),
    #[error("solver error: {0:?}")]
    Solver(#[from] solver::Error),
    #[error("failed to submit the solution")]
    SubmissionError,
}
