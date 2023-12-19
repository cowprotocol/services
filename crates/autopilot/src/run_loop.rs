use {
    crate::{
        arguments,
        database::{
            competition::{Competition, ExecutedFee, OrderExecution},
            Postgres,
        },
        driver_api::Driver,
        driver_model::{
            reveal::{self, Request},
            settle,
            solve::{self, fee_policy_to_dto, Class, TradedAmounts},
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
        order::{OrderClass, OrderUid},
        solver_competition::{
            CompetitionAuction,
            Order,
            Score,
            SolverCompetitionDB,
            SolverSettlement,
        },
    },
    number::nonzero::U256 as NonZeroU256,
    primitive_types::{H160, H256, U256},
    rand::seq::SliceRandom,
    shared::{remaining_amounts, token_list::AutoUpdatingTokenList},
    std::{
        collections::{BTreeMap, HashMap, HashSet},
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    },
    tracing::Instrument,
    web3::types::TransactionReceipt,
};

pub struct RunLoop {
    pub solvable_orders_cache: Arc<SolvableOrdersCache>,
    pub database: Arc<Postgres>,
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
    pub in_flight_orders: Arc<Mutex<InFlightOrders>>,
    pub fee_policy: arguments::FeePolicy,
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
                    self.single_run(id, auction)
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

    async fn single_run(&self, auction_id: AuctionId, auction: Auction) {
        tracing::info!(?auction, "solving");

        let auction = self.remove_in_flight_orders(auction).await;

        let solutions = {
            let mut solutions = self.competition(auction_id, &auction).await;
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

            let events = solution
                .order_ids()
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
            for order_id in solution.order_ids() {
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
                            false => ExecutedFee::Order(auction_order.metadata.solver_fee),
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
                            score: Score::Solver(participant.solution.score.get()),
                            ranking: solutions.len() - index,
                            orders: participant
                                .solution
                                .orders()
                                .iter()
                                .map(|(id, order)| Order::Colocated {
                                    id: *id,
                                    sell_amount: order.sell_amount,
                                    buy_amount: order.buy_amount,
                                })
                                .collect(),
                            clearing_prices: participant
                                .solution
                                .clearing_prices
                                .iter()
                                .map(|(token, price)| (*token, *price))
                                .collect(),
                            call_data: None,
                            uninternalized_call_data: None,
                        };
                        if is_winner {
                            settlement.call_data = Some(revealed.calldata.internalized.clone());
                            settlement.uninternalized_call_data =
                                Some(revealed.calldata.uninternalized.clone());
                        }
                        settlement
                    })
                    .collect(),
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
            let submission_start = Instant::now();
            match self.settle(driver, solution).await {
                Ok(()) => Metrics::settle_ok(driver, submission_start.elapsed()),
                Err(err) => {
                    Metrics::settle_err(driver, &err, submission_start.elapsed());
                    tracing::warn!(?err, driver = %driver.name, "settlement failed");
                }
            }
            let unsettled_orders: Vec<_> = solutions
                .iter()
                .flat_map(|p| p.solution.orders.keys())
                .filter(|uid| !solution.orders.contains_key(uid))
                .collect();
            Metrics::matched_unsettled(driver, unsettled_orders.as_slice());
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
            self.fee_policy.clone(),
        );
        let request = &request;

        let db = self.database.clone();
        let events = auction
            .orders
            .iter()
            .map(|o| (o.metadata.uid, OrderEventLabel::Ready))
            .collect_vec();
        // insert into `order_events` table operations takes a while and the result is
        // ignored, so we run it in the background
        tokio::spawn(
            async move {
                let start = Instant::now();
                db.store_order_events(&events).await;
                tracing::debug!(elapsed=?start.elapsed(), events_count=events.len(), "stored order events");
            }
            .instrument(tracing::Span::current()),
        );

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
                    orders: solution.orders,
                    clearing_prices: solution.clearing_prices,
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
    async fn settle(&self, driver: &Driver, solved: &Solution) -> Result<(), SettleError> {
        let events = solved
            .order_ids()
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

        *self.in_flight_orders.lock().unwrap() = InFlightOrders {
            tx_hash,
            orders: solved.orders.keys().copied().collect(),
        };

        let events = solved
            .orders
            .keys()
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

    /// Removes orders that are currently being settled to avoid solvers trying
    /// to fill an order a second time.
    async fn remove_in_flight_orders(&self, mut auction: Auction) -> Auction {
        let prev_settlement = self.in_flight_orders.lock().unwrap().tx_hash;
        let tx_receipt = self.web3.eth().transaction_receipt(prev_settlement).await;

        let prev_settlement_block = match tx_receipt {
            Ok(Some(TransactionReceipt {
                block_number: Some(number),
                ..
            })) => number.0[0],
            // Could not find the block of the previous settlement, let's be
            // conservative and assume all orders are still in-flight.
            _ => u64::MAX,
        };

        if auction.latest_settlement_block < prev_settlement_block {
            // Auction was built before the in-flight orders were processed.
            let in_flight_orders = self.in_flight_orders.lock().unwrap();
            auction
                .orders
                .retain(|o| !in_flight_orders.orders.contains(&o.metadata.uid));
            tracing::debug!(orders = ?in_flight_orders.orders, "filtered out in-flight orders");
        }

        auction
    }
}

pub fn solve_request(
    id: AuctionId,
    auction: &Auction,
    trusted_tokens: &HashSet<H160>,
    score_cap: U256,
    time_limit: Duration,
    fee_policy: arguments::FeePolicy,
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

                let fee_policies = match order.metadata.class {
                    OrderClass::Market => vec![],
                    OrderClass::Liquidity => vec![],
                    // todo https://github.com/cowprotocol/services/issues/2092
                    // skip protocol fee for limit orders with in-market price

                    // todo https://github.com/cowprotocol/services/issues/2115
                    // skip protocol fee for TWAP limit orders
                    OrderClass::Limit(_) => vec![fee_policy_to_dto(&fee_policy)],
                };
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
                    fee_policies,
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

/// Orders settled in the previous auction that might still be in-flight.
#[derive(Default)]
pub struct InFlightOrders {
    /// The transaction that these orders where settled in.
    tx_hash: H256,
    orders: HashSet<OrderUid>,
}

struct Participant<'a> {
    driver: &'a Driver,
    solution: Solution,
}

struct Solution {
    id: u64,
    account: H160,
    score: NonZeroU256,
    orders: HashMap<OrderUid, TradedAmounts>,
    clearing_prices: HashMap<H160, U256>,
}

impl Solution {
    pub fn order_ids(&self) -> impl Iterator<Item = &OrderUid> {
        self.orders.keys()
    }

    pub fn orders(&self) -> &HashMap<OrderUid, TradedAmounts> {
        &self.orders
    }
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
    #[metric(
        labels("driver", "result"),
        buckets(
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20
        )
    )]
    solve: prometheus::HistogramVec,

    /// Tracks driver solutions.
    #[metric(labels("driver", "result"))]
    solutions: prometheus::IntCounterVec,

    /// Tracks the result of driver `/reveal` requests.
    #[metric(labels("driver", "result"))]
    reveal: prometheus::IntCounterVec,

    /// Tracks the times and results of driver `/settle` requests.
    #[metric(labels("driver", "result"))]
    settle_time: prometheus::IntCounterVec,

    /// Tracks the number of orders that were part of some but not the winning
    /// solution together with the winning driver that did't include it.
    #[metric(labels("ignored_by"))]
    matched_unsettled: prometheus::IntCounterVec,
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

    fn settle_ok(driver: &Driver, time: Duration) {
        Self::get()
            .settle_time
            .with_label_values(&[&driver.name, "success"])
            .inc_by(time.as_millis().try_into().unwrap_or(u64::MAX));
    }

    fn settle_err(driver: &Driver, err: &SettleError, time: Duration) {
        let label = match err {
            SettleError::Failure(_) => "error",
        };
        Self::get()
            .settle_time
            .with_label_values(&[&driver.name, label])
            .inc_by(time.as_millis().try_into().unwrap_or(u64::MAX));
    }

    fn matched_unsettled(winning: &Driver, unsettled: &[&OrderUid]) {
        if !unsettled.is_empty() {
            tracing::debug!(?unsettled, "some orders were matched but not settled");
        }
        Self::get()
            .matched_unsettled
            .with_label_values(&[&winning.name])
            .inc_by(unsettled.len() as u64);
    }
}
