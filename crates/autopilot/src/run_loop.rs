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
    anyhow::{anyhow, ensure, Context, Result},
    chrono::Utc,
    database::order_events::OrderEventLabel,
    ethrpc::{current_block::CurrentBlockStream, Web3},
    itertools::Itertools,
    model::{
        auction::{Auction, AuctionId},
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
    primitive_types::{H160, H256, U256},
    rand::seq::SliceRandom,
    shared::{
        event_handling::MAX_REORG_BLOCK_COUNT,
        remaining_amounts,
        token_list::AutoUpdatingTokenList,
    },
    std::{
        collections::{BTreeMap, HashSet},
        sync::Arc,
        time::Duration,
    },
    tracing::Instrument,
    web3::types::Transaction,
};

pub const SOLVE_TIME_LIMIT: Duration = Duration::from_secs(15);

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
}

impl RunLoop {
    pub async fn run_forever(&self) -> ! {
        loop {
            self.single_run().await;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    async fn single_run(&self) {
        let auction = match self.solvable_orders_cache.current_auction() {
            Some(auction) => auction,
            None => {
                tracing::debug!("no current auction");
                return;
            }
        };
        let id = match self.database.replace_current_auction(&auction).await {
            Ok(id) => id,
            Err(err) => {
                tracing::error!(?err, "failed to replace current auction");
                return;
            }
        };
        self.single_run_(id, &auction)
            .instrument(tracing::info_span!("auction", id))
            .await;
    }

    async fn single_run_(&self, id: AuctionId, auction: &Auction) {
        tracing::info!("solving");
        let solutions = self.solve(auction, id).await;

        // Validate solutions and filter out invalid ones.
        let mut solutions = solutions
            .into_iter()
            .filter_map(|(index, response)| {
                if response.solutions.is_empty() {
                    tracing::debug!(driver = ?self.drivers[index].url, "driver sent zero solutions");
                    return None;
                }

                Some(
                    response
                        .solutions
                        .into_iter()
                        .filter_map(|solution| {
                            if solution.score == U256::zero() {
                                tracing::debug!(
                                    id = ?solution.solution_id,
                                    driver = ?self.drivers[index].url,
                                    "driver sent solution with zero score",
                                );
                                None
                            } else {
                                Some((index, solution))
                            }
                        })
                        .collect_vec(),
                )
            })
            .flatten()
            .collect_vec();

        if solutions.is_empty() {
            tracing::info!(?id, "no solutions for auction");
            return;
        }

        // Shuffle so that sorting randomly splits ties.
        solutions.shuffle(&mut rand::thread_rng());
        solutions.sort_unstable_by_key(|solution| solution.1.score);
        let competition_simulation_block = self.current_block.borrow().number;

        // TODO: Keep going with other solutions until some deadline.
        if let Some((index, solution)) = solutions.last() {
            tracing::info!(url = %self.drivers[*index].url, "revealing with driver");
            let revealed = match self
                .reveal(id, solution.solution_id, &self.drivers[*index])
                .await
            {
                Ok(result) => result,
                Err(err) => {
                    tracing::warn!(?err, "driver {} failed to reveal", self.drivers[*index].url);
                    return;
                }
            };

            let events = revealed
                .orders
                .iter()
                .map(|o| (*o, OrderEventLabel::Considered))
                .collect::<Vec<_>>();
            self.database.store_order_events(&events).await;
            let auction_id = id;
            let winner = solution.submission_address;
            let winning_score = solution.score;
            let reference_score = solutions
                .iter()
                .nth_back(1)
                .map(|(_, response)| response.score)
                .unwrap_or_default();
            let participants = solutions
                .iter()
                .map(|(_, response)| response.submission_address)
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
                    .map(|(index, response)| {
                        let is_winner = solutions.len() - index == 1;
                        let mut settlement = SolverSettlement {
                            solver_address: response.submission_address,
                            score: Some(Score::Solver(response.score)),
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

            tracing::info!(url = %self.drivers[*index].url, "settling with solver");
            match self
                .settle(id, &self.drivers[*index], solution, &revealed)
                .await
            {
                Ok(()) => (),
                Err(err) => {
                    tracing::error!(?err, "solver {index} failed to settle");
                }
            }
        }
    }

    /// Returns the successful /solve responses and the index of the solver.
    async fn solve(&self, auction: &Auction, id: AuctionId) -> Vec<(usize, solve::Response)> {
        if auction
            .orders
            .iter()
            .all(|order| match order.metadata.class {
                OrderClass::Market => false,
                OrderClass::Liquidity => true,
                OrderClass::Limit(_) => false,
            })
        {
            return Default::default();
        }

        let request = solve_request(
            id,
            auction,
            &self.market_makable_token_list.all(),
            self.score_cap,
        );
        let request = &request;

        self.database
            .store_order_events(
                &auction
                    .orders
                    .iter()
                    .map(|o| (o.metadata.uid, OrderEventLabel::Ready))
                    .collect_vec(),
            )
            .await;

        let futures = self
            .drivers
            .iter()
            .enumerate()
            .map(|(index, driver)| async move {
                let result =
                    match tokio::time::timeout(SOLVE_TIME_LIMIT, driver.solve(request)).await {
                        Ok(inner) => inner,
                        Err(_) => Err(anyhow!("timeout")),
                    };
                (index, result)
            })
            .collect::<Vec<_>>();
        let results = futures::future::join_all(futures).await;
        results
            .into_iter()
            .filter_map(|(index, result)| match result {
                Ok(result) => Some((index, result)),
                Err(err) => {
                    tracing::warn!(?err, "driver solve error");
                    None
                }
            })
            .collect()
    }

    /// Ask the winning solver to reveal their solution.
    async fn reveal(
        &self,
        id: AuctionId,
        solution_id: u64,
        driver: &Driver,
    ) -> Result<reveal::Response> {
        let response = driver
            .reveal(&Request { solution_id })
            .await
            .context("reveal")?;
        ensure!(
            response.calldata.internalized.ends_with(&id.to_be_bytes()),
            "reveal auction id missmatch"
        );
        Ok(response)
    }

    /// Execute the solver's solution. Returns Ok when the corresponding
    /// transaction has been mined.
    async fn settle(
        &self,
        id: AuctionId,
        driver: &Driver,
        solved: &solve::Solution,
        revealed: &reveal::Response,
    ) -> Result<()> {
        let events = revealed
            .orders
            .iter()
            .map(|uid| (*uid, OrderEventLabel::Executing))
            .collect_vec();
        self.database.store_order_events(&events).await;

        driver
            .settle(&settle::Request {
                solution_id: solved.solution_id,
            })
            .await
            .context("settle")?;
        // TODO: React to deadline expiring.
        let transaction = self
            .wait_for_settlement_transaction(id, solved.submission_address)
            .await
            .context("wait for settlement transaction")?;
        if let Some(tx) = transaction {
            let events = revealed
                .orders
                .iter()
                .map(|uid| (*uid, OrderEventLabel::Traded))
                .collect_vec();
            self.database.store_order_events(&events).await;
            tracing::debug!("settled in tx {:?}", tx.hash);
        }
        Ok(())
    }

    /// Tries to find a `settle` contract call with calldata ending in `tag`.
    ///
    /// Returns None if no transaction was found within the deadline.
    pub async fn wait_for_settlement_transaction(
        &self,
        id: AuctionId,
        submission_address: H160,
    ) -> Result<Option<Transaction>> {
        const MAX_WAIT_TIME: Duration = Duration::from_secs(60);
        // Start earlier than current block because there might be a delay when
        // receiving the Solver's /execute response during which it already
        // started broadcasting the tx.
        let start_offset = MAX_REORG_BLOCK_COUNT;
        let max_wait_time_blocks =
            (MAX_WAIT_TIME.as_secs_f32() / self.network_block_interval.as_secs_f32()).ceil() as u64;
        let current = self.current_block.borrow().number;
        let start = current.saturating_sub(start_offset);
        let deadline = current.saturating_add(max_wait_time_blocks);
        tracing::debug!(%current, %start, %deadline, ?id, ?submission_address, "waiting for settlement");

        // Use the existing event indexing infrastructure to find the transaction. We
        // query all settlement events in the block range to get tx hashes and
        // query the node for the full calldata.
        //
        // If the block range was large, we would make the query more efficient by
        // moving the starting block up while taking reorgs into account. With
        // the current range of 30 blocks this isn't necessary.
        //
        // We do keep track of hashes we have already seen to reduce load from the node.

        let mut seen_transactions: HashSet<H256> = Default::default();
        loop {
            // This could be a while loop. It isn't, because some care must be taken to not
            // accidentally keep the borrow alive, which would block senders. Technically
            // this is fine with while conditions but this is clearer.
            if self.current_block.borrow().number > deadline {
                break;
            }
            let mut hashes = self
                .database
                .recent_settlement_tx_hashes(start..deadline + 1)
                .await?;
            hashes.retain(|hash| !seen_transactions.contains(hash));
            for hash in hashes {
                let tx: Option<Transaction> = self
                    .web3
                    .eth()
                    .transaction(web3::types::TransactionId::Hash(hash))
                    .await
                    .with_context(|| format!("web3 transaction {hash:?}"))?;
                let tx: Transaction = match tx {
                    Some(tx) => tx,
                    None => continue,
                };
                if tx.input.0.ends_with(&id.to_be_bytes()) && tx.from == Some(submission_address) {
                    return Ok(Some(tx));
                }
                seen_transactions.insert(hash);
            }
            // It would be more correct to wait until just after the last event update run,
            // but that is hard to synchronize.
            tokio::time::sleep(self.network_block_interval.div_f32(2.)).await;
        }
        Ok(None)
    }

    /// Saves the competition data to the database
    async fn save_competition(&self, competition: &Competition) -> Result<()> {
        self.database
            .save_competition(competition)
            .await
            .context("save competition data")
    }
}

pub fn solve_request(
    id: AuctionId,
    auction: &Auction,
    trusted_tokens: &HashSet<H160>,
    score_cap: U256,
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
        deadline: Utc::now() + chrono::Duration::from_std(SOLVE_TIME_LIMIT).unwrap(),
        score_cap,
    }
}
