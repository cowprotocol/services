use {
    crate::{
        database::{
            competition::{Competition, ExecutedFee, OrderExecution},
            Postgres,
        },
        driver_api::Driver,
        driver_model::{
            reveal::{self, Request},
            settle,
            solve::{self, Class},
        },
        solvable_orders::SolvableOrdersCache,
    },
    anyhow::Result,
    chrono::Utc,
    database::order_events::OrderEventLabel,
    ethrpc::{current_block::CurrentBlockStream, Web3},
    itertools::Itertools,
    model::{
        auction::{Auction, AuctionId, AuctionWithId},
        interaction::InteractionData,
        order::OrderClass,
        solver_competition::{
            CompetitionAuction,
            Order,
            Score,
            SolverCompetitionDB,
            SolverSettlement,
        },
    },
    number::nonzero::U256 as NonZeroU256,
    primitive_types::{H160, U256},
    rand::seq::SliceRandom,
    shared::{remaining_amounts, token_list::AutoUpdatingTokenList},
    std::{
        collections::{BTreeMap, HashSet},
        sync::Arc,
        time::{Duration, Instant},
    },
    tracing::Instrument,
};

pub struct RunLoop {
    pub solvable_orders_cache: Arc<SolvableOrdersCache>,
    pub database: Postgres,
    pub drivers: Vec<Driver>,
    pub current_block: CurrentBlockStream,
    pub web3: Web3,
    pub network_block_interval: Duration,
    pub market_makable_token_list: AutoUpdatingTokenList,
    pub submission_deadline: u64,
    pub additional_deadline_for_rewards: u64,
    pub score_cap: U256,
    pub max_settlement_transaction_wait: Duration,
    pub solve_deadline: Duration,
}

impl RunLoop {
    pub async fn run_forever(self) -> ! {
        let mut last_auction_id = None;
        let mut last_block = None;
        loop {
            if let Some(AuctionWithId { id, auction }) = self.next_auction().await {
                let current_block = self.current_block.borrow().hash;
                // Only run the solvers if the auction or block has changed.
                if last_auction_id.replace(id) != Some(id)
                    || last_block.replace(current_block) != Some(current_block)
                {
                    self.single_run(id, &auction)
                        .instrument(tracing::info_span!("auction", id))
                        .await;
                }
            };
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    async fn next_auction(&self) -> Option<AuctionWithId> {
        let auction = match self.solvable_orders_cache.current_auction() {
            Some(auction) => auction,
            None => {
                tracing::debug!("no current auction");
                return None;
            }
        };

        let id = match self.database.replace_current_auction(&auction).await {
            Ok(id) => {
                Metrics::auction(id);
                id
            }
            Err(err) => {
                tracing::error!(?err, "failed to replace current auction");
                return None;
            }
        };

        if auction
            .orders
            .iter()
            .all(|order| match order.metadata.class {
                OrderClass::Market => false,
                OrderClass::Liquidity => true,
                OrderClass::Limit(_) => false,
            })
        {
            tracing::debug!("skipping empty auction");
            return None;
        }

        Some(AuctionWithId { id, auction })
    }

    async fn single_run(&self, auction_id: AuctionId, auction: &Auction) {
        tracing::info!(?auction, "solving");

        let solutions = {
            let mut solutions = self.competition(auction_id, auction).await;
            if solutions.is_empty() {
                tracing::info!("no solutions for auction");
                return;
            }

            // Shuffle so that sorting randomly splits ties.
            solutions.shuffle(&mut rand::thread_rng());
            solutions.sort_unstable_by_key(|participant| participant.solution.score);
            solutions
        };
        let competition_simulation_block = self.current_block.borrow().number;

        // TODO: Keep going with other solutions until some deadline.
        if let Some(Participant { driver, solution }) = solutions.last() {
            tracing::info!(driver = %driver.name, solution = %solution.id, "winner");

            let revealed = match self.reveal(driver, auction_id, solution.id).await {
                Ok(result) => {
                    Metrics::reveal_ok(driver);
                    result
                }
                Err(err) => {
                    Metrics::reveal_err(driver, &err);
                    tracing::warn!(driver = %driver.name, ?err, "failed to reveal winning solution");
                    return;
                }
            };

            let events = revealed
                .orders
                .iter()
                .map(|o| (*o, OrderEventLabel::Considered))
                .collect::<Vec<_>>();
            self.database.store_order_events(&events).await;

            let winner = solution.account;
            let winning_score = solution.score.get();
            let reference_score = solutions
                .iter()
                .nth_back(1)
                .map(|participant| participant.solution.score.get())
                .unwrap_or_default();
            let participants = solutions
                .iter()
                .map(|participant| participant.solution.account)
                .collect::<HashSet<_>>();

            let mut prices = BTreeMap::new();
            let block_deadline = competition_simulation_block
                + self.submission_deadline
                + self.additional_deadline_for_rewards;
            let call_data = revealed.calldata.internalized.clone();
            let uninternalized_call_data = revealed.calldata.uninternalized.clone();

            // Save order executions for all orders in the solution. Surplus fees for
            // limit orders will be saved after settling the order onchain.
            let mut order_executions = vec![];
            for order_id in &revealed.orders {
                let auction_order = auction
                    .orders
                    .iter()
                    .find(|auction_order| &auction_order.metadata.uid == order_id);
                match auction_order {
                    Some(auction_order) => {
                        let executed_fee = match auction_order.solver_determines_fee() {
                            // we don't know the surplus fee in advance. will be populated
                            // after the transaction containing the order is mined
                            true => ExecutedFee::Surplus,
                            false => ExecutedFee::Solver(auction_order.metadata.solver_fee),
                        };
                        order_executions.push(OrderExecution {
                            order_id: *order_id,
                            executed_fee,
                        });
                        if let Some(price) = auction.prices.get(&auction_order.data.sell_token) {
                            prices.insert(auction_order.data.sell_token, *price);
                        } else {
                            tracing::error!(
                                sell_token = ?auction_order.data.sell_token,
                                "sell token price is missing in auction"
                            );
                        }
                        if let Some(price) = auction.prices.get(&auction_order.data.buy_token) {
                            prices.insert(auction_order.data.buy_token, *price);
                        } else {
                            tracing::error!(
                                buy_token = ?auction_order.data.buy_token,
                                "buy token price is missing in auction"
                            );
                        }
                    }
                    None => {
                        tracing::debug!(?order_id, "order not found in auction");
                    }
                }
            }

            let competition_table = SolverCompetitionDB {
                auction_start_block: auction.block,
                competition_simulation_block,
                auction: CompetitionAuction {
                    orders: auction
                        .orders
                        .iter()
                        .map(|order| order.metadata.uid)
                        .collect(),
                    prices: auction.prices.clone(),
                },
                solutions: solutions
                    .iter()
                    .enumerate()
                    .map(|(index, participant)| {
                        let is_winner = solutions.len() - index == 1;
                        let mut settlement = SolverSettlement {
                            solver: participant.driver.name.clone(),
                            solver_address: participant.solution.account,
                            score: Some(Score::Solver(participant.solution.score.get())),
                            ranking: Some(solutions.len() - index),
                            // TODO: revisit once colocation is enabled (remove not populated
                            // fields) Not all fields can be populated in the colocated world
                            ..Default::default()
                        };
                        if is_winner {
                            settlement.orders = revealed
                                .orders
                                .iter()
                                .map(|o| Order {
                                    id: *o,
                                    // TODO: revisit once colocation is enabled (remove not
                                    // populated fields) Not all
                                    // fields can be populated in the colocated world
                                    ..Default::default()
                                })
                                .collect();
                            settlement.call_data = revealed.calldata.internalized.clone();
                            settlement.uninternalized_call_data =
                                Some(revealed.calldata.uninternalized.clone());
                        }
                        settlement
                    })
                    .collect(),
                // TODO: revisit once colocation is enabled (remove not populated fields)
                // Not all fields can be populated in the colocated world
                ..Default::default()
            };
            let competition = Competition {
                auction_id,
                winner,
                winning_score,
                reference_score,
                participants,
                prices,
                block_deadline,
                order_executions,
                competition_simulation_block,
                call_data,
                uninternalized_call_data,
                competition_table,
            };

            tracing::info!(?competition, "saving competition");
            if let Err(err) = self.save_competition(&competition).await {
                tracing::error!(?err, "failed to save competition");
                return;
            }

            tracing::info!(driver = %driver.name, "settling");
            match self.settle(driver, solution, &revealed).await {
                Ok(()) => Metrics::settle_ok(driver),
                Err(err) => {
                    Metrics::settle_err(driver, &err);
                    tracing::warn!(?err, driver = %driver.name, "settlement failed");
                }
            }
        }
    }

    /// Runs the solver competition, making all configured drivers participate.
    async fn competition(&self, id: AuctionId, auction: &Auction) -> Vec<Participant<'_>> {
        let request = solve_request(
            id,
            auction,
            &self.market_makable_token_list.all(),
            self.score_cap,
            self.solve_deadline,
        );
        let request = &request;

        let start = Instant::now();
        self.database
            .store_order_events(
                &auction
                    .orders
                    .iter()
                    .map(|o| (o.metadata.uid, OrderEventLabel::Ready))
                    .collect_vec(),
            )
            .await;
        tracing::debug!(elapsed=?start.elapsed(), aution_id=%id, "storing order events took");

        let start = Instant::now();
        futures::future::join_all(self.drivers.iter().map(|driver| async move {
            let result = self.solve(driver, request).await;
            let solutions = match result {
                Ok(solutions) => {
                    Metrics::solve_ok(driver, start.elapsed());
                    solutions
                }
                Err(err) => {
                    Metrics::solve_err(driver, start.elapsed(), &err);
                    if matches!(err, SolveError::NoSolutions) {
                        tracing::debug!(driver = %driver.name, "solver found no solution");
                    } else {
                        tracing::warn!(?err, driver = %driver.name, "solve error");
                    }
                    vec![]
                }
            };

            solutions.into_iter().filter_map(|solution| match solution {
                Ok(solution) => {
                    Metrics::solution_ok(driver);
                    Some(Participant { driver, solution })
                }
                Err(err) => {
                    Metrics::solution_err(driver, &err);
                    tracing::debug!(?err, driver = %driver.name, "invalid proposed solution");
                    None
                }
            })
        }))
        .await
        .into_iter()
        .flatten()
        .collect()
    }

    /// Computes a driver's solutions for the solver competition.
    async fn solve(
        &self,
        driver: &Driver,
        request: &solve::Request,
    ) -> Result<Vec<Result<Solution, ZeroScoreError>>, SolveError> {
        let response = tokio::time::timeout(self.solve_deadline, driver.solve(request))
            .await
            .map_err(|_| SolveError::Timeout)?
            .map_err(SolveError::Failure)?;
        if response.solutions.is_empty() {
            return Err(SolveError::NoSolutions);
        }

        Ok(response
            .solutions
            .into_iter()
            .map(|solution| {
                Ok(Solution {
                    id: solution.solution_id,
                    account: solution.submission_address,
                    score: NonZeroU256::new(solution.score).ok_or(ZeroScoreError)?,
                })
            })
            .collect())
    }

    /// Ask the winning solver to reveal their solution.
    async fn reveal(
        &self,
        driver: &Driver,
        auction: AuctionId,
        solution_id: u64,
    ) -> Result<reveal::Response, RevealError> {
        let response = driver
            .reveal(&Request { solution_id })
            .await
            .map_err(RevealError::Failure)?;
        if !response
            .calldata
            .internalized
            .ends_with(&auction.to_be_bytes())
        {
            return Err(RevealError::AuctionMismatch);
        }

        Ok(response)
    }

    /// Execute the solver's solution. Returns Ok when the corresponding
    /// transaction has been mined.
    async fn settle(
        &self,
        driver: &Driver,
        solved: &Solution,
        revealed: &reveal::Response,
    ) -> Result<(), SettleError> {
        let events = revealed
            .orders
            .iter()
            .map(|uid| (*uid, OrderEventLabel::Executing))
            .collect_vec();
        self.database.store_order_events(&events).await;

        let request = settle::Request {
            solution_id: solved.id,
        };

        let tx_hash = driver
            .settle(&request, self.max_settlement_transaction_wait)
            .await
            .map_err(SettleError::Failure)?
            .tx_hash;

        let events = revealed
            .orders
            .iter()
            .map(|uid| (*uid, OrderEventLabel::Traded))
            .collect_vec();
        self.database.store_order_events(&events).await;
        tracing::debug!(?tx_hash, "solution settled");

        Ok(())
    }

    /// Saves the competition data to the database
    async fn save_competition(&self, competition: &Competition) -> Result<()> {
        self.database.save_competition(competition).await
    }
}

pub fn solve_request(
    id: AuctionId,
    auction: &Auction,
    trusted_tokens: &HashSet<H160>,
    score_cap: U256,
    time_limit: Duration,
) -> solve::Request {
    solve::Request {
        id,
        orders: auction
            .orders
            .iter()
            .map(|order| {
                let class = match order.metadata.class {
                    OrderClass::Market => Class::Market,
                    OrderClass::Liquidity => Class::Liquidity,
                    OrderClass::Limit(_) => Class::Limit,
                };
                let remaining_order = remaining_amounts::Order::from(order);
                let map_interactions =
                    |interactions: &[InteractionData]| -> Vec<solve::Interaction> {
                        interactions
                            .iter()
                            .map(|interaction| solve::Interaction {
                                target: interaction.target,
                                value: interaction.value,
                                call_data: interaction.call_data.clone(),
                            })
                            .collect()
                    };
                let order_is_untouched = remaining_order.executed_amount.is_zero();
                solve::Order {
                    uid: order.metadata.uid,
                    sell_token: order.data.sell_token,
                    buy_token: order.data.buy_token,
                    sell_amount: order.data.sell_amount,
                    buy_amount: order.data.buy_amount,
                    solver_fee: order.metadata.full_fee_amount,
                    user_fee: order.data.fee_amount,
                    valid_to: order.data.valid_to,
                    kind: order.data.kind,
                    receiver: order.data.receiver,
                    owner: order.metadata.owner,
                    partially_fillable: order.data.partially_fillable,
                    executed: remaining_order.executed_amount,
                    // Partially fillable orders should have their pre-interactions only executed
                    // on the first fill.
                    pre_interactions: order_is_untouched
                        .then(|| map_interactions(&order.interactions.pre))
                        .unwrap_or_default(),
                    post_interactions: map_interactions(&order.interactions.post),
                    sell_token_balance: order.data.sell_token_balance,
                    buy_token_balance: order.data.buy_token_balance,
                    class,
                    app_data: order.data.app_data,
                    signature: order.signature.clone(),
                }
            })
            .collect(),
        tokens: auction
            .prices
            .iter()
            .map(|(address, price)| solve::Token {
                address: address.to_owned(),
                price: Some(price.to_owned()),
                trusted: trusted_tokens.contains(address),
            })
            .chain(trusted_tokens.iter().map(|&address| solve::Token {
                address,
                price: None,
                trusted: true,
            }))
            .unique_by(|token| token.address)
            .collect(),
        deadline: Utc::now() + chrono::Duration::from_std(time_limit).unwrap(),
        score_cap,
    }
}

struct Participant<'a> {
    driver: &'a Driver,
    solution: Solution,
}

struct Solution {
    id: u64,
    account: H160,
    score: NonZeroU256,
}

#[derive(Debug, thiserror::Error)]
enum SolveError {
    #[error("the solver timed out")]
    Timeout,
    #[error("driver did not propose any solutions")]
    NoSolutions,
    #[error(transparent)]
    Failure(anyhow::Error),
}

#[derive(Debug, thiserror::Error)]
#[error("the solver proposed a 0-score solution")]
struct ZeroScoreError;

#[derive(Debug, thiserror::Error)]
enum RevealError {
    #[error("revealed calldata does not match auction")]
    AuctionMismatch,
    #[error(transparent)]
    Failure(anyhow::Error),
}

#[derive(Debug, thiserror::Error)]
enum SettleError {
    #[error(transparent)]
    Failure(anyhow::Error),
}

#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "runloop")]
struct Metrics {
    /// Tracks the last executed auction.
    auction: prometheus::IntGauge,

    /// Tracks the duration of successful driver `/solve` requests.
    #[metric(labels("driver", "result"))]
    solve: prometheus::HistogramVec,

    /// Tracks driver solutions.
    #[metric(labels("driver", "result"))]
    solutions: prometheus::IntCounterVec,

    /// Tracks the result of driver `/reveal` requests.
    #[metric(labels("driver", "result"))]
    reveal: prometheus::IntCounterVec,

    /// Tracks the result of driver `/settle` requests.
    #[metric(labels("driver", "result"))]
    settle: prometheus::IntCounterVec,
}

impl Metrics {
    fn get() -> &'static Self {
        Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
    }

    fn auction(auction_id: AuctionId) {
        Self::get().auction.set(auction_id)
    }

    fn solve_ok(driver: &Driver, elapsed: Duration) {
        Self::get()
            .solve
            .with_label_values(&[&driver.name, "success"])
            .observe(elapsed.as_secs_f64())
    }

    fn solve_err(driver: &Driver, elapsed: Duration, err: &SolveError) {
        let label = match err {
            SolveError::Timeout => "timeout",
            SolveError::NoSolutions => "no_solutions",
            SolveError::Failure(_) => "error",
        };
        Self::get()
            .solve
            .with_label_values(&[&driver.name, label])
            .observe(elapsed.as_secs_f64())
    }

    fn solution_ok(driver: &Driver) {
        Self::get()
            .solutions
            .with_label_values(&[&driver.name, "success"])
            .inc();
    }

    fn solution_err(driver: &Driver, _: &ZeroScoreError) {
        Self::get()
            .solutions
            .with_label_values(&[&driver.name, "zero_score"])
            .inc();
    }

    fn reveal_ok(driver: &Driver) {
        Self::get()
            .reveal
            .with_label_values(&[&driver.name, "success"])
            .inc();
    }

    fn reveal_err(driver: &Driver, err: &RevealError) {
        let label = match err {
            RevealError::AuctionMismatch => "mismatch",
            RevealError::Failure(_) => "error",
        };
        Self::get()
            .reveal
            .with_label_values(&[&driver.name, label])
            .inc();
    }

    fn settle_ok(driver: &Driver) {
        Self::get()
            .settle
            .with_label_values(&[&driver.name, "success"])
            .inc();
    }

    fn settle_err(driver: &Driver, err: &SettleError) {
        let label = match err {
            SettleError::Failure(_) => "error",
        };
        Self::get()
            .settle
            .with_label_values(&[&driver.name, label])
            .inc();
    }
}
