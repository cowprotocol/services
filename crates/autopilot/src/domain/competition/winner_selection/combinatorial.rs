//! Winner Selection:
//! Implements a winner selction algorithm which picks the **set** of solutions
//! which maximize surplus while enforcing uniform **directional** clearing
//! prices. That means all orders selling the same token must get executed at
//! the same price for that token. But orders buying that same token may all be
//! settled at a different (but still uniform) price. So effectively instead of
//! allowing only 1 price for each token (uniform clearing price) each token may
//! have 2 prices (one for selling it and another for buying it).
//!
//! Fairness Guarantees:
//! A solution is only valid if it does not settle any order at a worse uniform
//! directional clearing price than the best solution which only contains this
//! uniform directional clearing price. In other words an order may only be
//! batched with other orders if each order gets a better deal than executing
//! it individually.
//! Because these guarantees rely heavily on all relevant scores of each
//! solution being computed, we'll discard solutions where that computation
//! fails.
//!
//! Reference Score:
//! Each solver S with a winning solution gets one reference score. The
//! reference score is the total score of all winning solutions if the solver S
//! had not participated in the competition.
//! That is effectively a measurement of how much better each order got executed
//! because solver S participated in the competition.
use {
    super::Arbitrator,
    crate::domain::{
        self, auction::{
            order::{self, TargetAmount}, Prices
        }, competition::{participant, Participant, Score, Solution, Unranked}, eth::{self, WrappedNativeToken}, fee, settlement::{
            math,
            transaction::{self, ClearingPrices},
        }, OrderUid
    },
    anyhow::{Context, Result},
    itertools::Itertools,
    std::{
        collections::{HashMap, HashSet},
        ops::Add,
    },
};

impl Arbitrator for Config {
    fn filter_unfair_solutions(
        &self,
        mut participants: Vec<Participant<Unranked>>,
        auction: &domain::Auction,
    ) -> Vec<Participant<Unranked>> {
        // Discard all solutions where we can't compute the aggregate scores
        // accurately because the fairness guarantees heavily rely on them.
        let scores_by_solution = compute_scores_by_solution(&mut participants, auction);
        participants.sort_unstable_by_key(|participant| {
            std::cmp::Reverse(participant.solution().score().get().0)
        });
        eprintln!("    after sorting by score:");
        for participant in &participants {
            eprintln!("        - {} - {}", participant.driver().submission_address, participant.solution().score);
        }
        let baseline_scores = compute_baseline_scores(&scores_by_solution);
        eprintln!("    baseline_scores:");
        for (pair, score) in &baseline_scores {
            eprintln!("        - (buy={}, sell={}) - score: {}", pair.buy.0, pair.sell.0, score);
        }
        eprintln!("    participant filtering:");
        participants.retain(|p| {
            let aggregated_scores = scores_by_solution
                .get(&SolutionKey {
                    driver: p.driver().submission_address,
                    solution_id: p.solution().id(),
                })
                .expect("every remaining participant has an entry");
            eprintln!("        - {}", p.driver().submission_address);
            eprintln!("            aggregated scores");
            for (pair, score) in aggregated_scores {
                eprintln!("             - (buy={}, sell={}) - score: {}", pair.buy.0, pair.sell.0, score);
            }
            eprintln!("            is_fair = {}", aggregated_scores.len() == 1
            || aggregated_scores.iter().all(|(pair, score)| {
                baseline_scores
                    .get(pair)
                    .is_none_or(|baseline| score >= baseline)
            }));
            // only keep solutions where each order execution is at least as good as
            // the baseline solution (or when there is only one baseline solution)
            aggregated_scores.len() == 1
                || aggregated_scores.iter().all(|(pair, score)| {
                    baseline_scores
                        .get(pair)
                        .is_none_or(|baseline| score >= baseline)
                })
        });
        participants
    }

    fn mark_winners(&self, participants: Vec<Participant<Unranked>>) -> Vec<Participant> {
        let winner_indexes = self.pick_winners(participants.iter().map(|p| p.solution()));
        participants
            .into_iter()
            .enumerate()
            .map(|(index, participant)| participant.rank(winner_indexes.contains(&index)))
            .collect()
    }

    fn compute_reference_scores(
        &self,
        participants: &[Participant],
    ) -> HashMap<eth::Address, Score> {
        let mut reference_scores = HashMap::default();

        for participant in participants {
            let solver = participant.driver().submission_address;
            if reference_scores.len() >= self.max_winners {
                // all winners have been processed
                return reference_scores;
            }
            if reference_scores.contains_key(&solver) {
                // we already computed this solver's reference score
                continue;
            }

            let solutions_without_solver = participants
                .iter()
                .filter(|p| p.driver().submission_address != solver)
                .map(|p| p.solution());
            let winner_indices = self.pick_winners(solutions_without_solver.clone());

            let score = solutions_without_solver
                .enumerate()
                .filter(|(index, _)| winner_indices.contains(index))
                .filter_map(|(_, solution)| solution.computed_score)
                .reduce(Score::add)
                .unwrap_or_default();
            reference_scores.insert(solver, score);
        }

        reference_scores
    }
}

impl Config {
    /// Returns indices of winning solutions.
    /// Assumes that `solutions` is sorted by score descendingly.
    /// This logic was moved into a helper function to avoid a ton of `.clone()`
    /// operations in `compute_reference_scores()`.
    fn pick_winners<'a>(&self, solutions: impl Iterator<Item = &'a Solution>) -> HashSet<usize> {
        // Winners are selected one by one, starting from the best solution,
        // until `max_winners` are selected. A solution can only
        // win if none of the (sell_token, buy_token) pairs of the executed
        // orders have been covered by any previously selected winning solution.
        // In other words this enforces a uniform **directional** clearing price.
        let mut already_swapped_tokens_pairs = HashSet::new();
        let mut winners = HashSet::default();
        for (index, solution) in solutions.enumerate() {
            if winners.len() >= self.max_winners {
                return winners;
            }

            let swapped_token_pairs = solution
                .orders()
                .values()
                .map(|order| DirectedTokenPair {
                    sell: order.sell.token.as_erc20(self.weth),
                    buy: order.buy.token.as_erc20(self.weth),
                })
                .collect::<HashSet<_>>();

            if swapped_token_pairs.is_disjoint(&already_swapped_tokens_pairs) {
                winners.insert(index);
                already_swapped_tokens_pairs.extend(swapped_token_pairs);
            }
        }
        winners
    }
}

/// Let's call a solution that only trades 1 directed token pair a baseline
/// solution. Returns the best baseline solution (highest score) for
/// each token pair if one exists.
fn compute_baseline_scores(scores_by_solution: &ScoresBySolution) -> ScoreByDirection {
    let mut baseline_directional_scores = ScoreByDirection::default();
    eprintln!("    compute_baseline_scores");
    for scores in scores_by_solution.values() {
        let Ok((token_pair, score)) = scores.iter().exactly_one() else {
            // base solutions must contain exactly 1 directed token pair
            eprintln!("        - discarded because does not contain exactly 1 directed token pair" );
            continue;
        };
        eprintln!("        - (buy={}, sell={}), score: {}", token_pair.buy.0, token_pair.sell.0, score);
        let current_best_score = baseline_directional_scores
            .entry(token_pair.clone())
            .or_default();
        if score > current_best_score {
            *current_best_score = *score;
        }
    }
    baseline_directional_scores
}

/// Computes the `DirectionalScores` for all solutions and discards
/// solutions as invalid whenever that computation is not possible.
/// Solutions get discarded because fairness guarantees heavily
/// depend on these scores being accurate.
fn compute_scores_by_solution(
    participants: &mut Vec<Participant<Unranked>>,
    auction: &domain::Auction,
) -> ScoresBySolution {
    eprintln!("    compute_scores_by_solution:");
    let auction = Auction::from(auction);
    let mut scores = HashMap::default();

    participants.retain_mut(|p| match score_by_token_pair(p.solution(), &auction) {
        Ok(score) => {
            let total_score = score
                .values()
                .fold(Default::default(), |acc, score| acc + *score);
            eprintln!("        - {} - score: {}", p.driver().submission_address, total_score);
            scores.insert(
                SolutionKey {
                    driver: p.driver().submission_address,
                    solution_id: p.solution().id,
                },
                score,
            );
            p.set_computed_score(total_score);
            true
        }
        Err(err) => {
            tracing::warn!(
                driver = p.driver().name,
                ?err,
                solution = ?p.solution(),
                "discarding solution where scores could not be computed"
            );
            false
        }
    });

    scores
}

/// Returns the total scores for each directed token pair of the solution.
/// E.g. if a solution contains 3 orders like:
///     sell A for B with a score of 10
///     sell A for B with a score of 5
///     sell B for C with a score of 5
/// it will return a map like:
///     (A, B) => 15
///     (B, C) => 5
fn score_by_token_pair(solution: &Solution, auction: &Auction) -> Result<ScoreByDirection> {
    let mut scores = HashMap::default();
    
    for (uid, trade) in solution.orders() {
        if !auction.contributes_to_score(uid) {
            continue;
        }

        let uniform_sell_price = solution
            .prices()
            .get(&trade.sell.token)
            .context("no uniform clearing price for sell token")?;
        let uniform_buy_price = solution
            .prices()
            .get(&trade.buy.token)
            .context("no uniform clearing price for buy token")?;

        let trade = math::Trade {
            uid: *uid,
            sell: trade.sell,
            buy: trade.buy,
            side: trade.side,
            executed: match trade.side {
                order::Side::Buy => TargetAmount(trade.executed_buy.into()),
                order::Side::Sell => TargetAmount(trade.executed_sell.into()),
            },
            prices: transaction::Prices {
                // clearing prices are denominated in the same underlying
                // unit so we assign sell to sell and buy to buy
                uniform: ClearingPrices {
                    sell: uniform_sell_price.get().into(),
                    buy: uniform_buy_price.get().into(),
                },
                // for custom clearing prices we only need to know how
                // much the traded tokens are worth relative to each
                // other so we can simply use the swapped executed
                // amounts here
                custom: ClearingPrices {
                    sell: trade.executed_buy.into(),
                    buy: trade.executed_sell.into(),
                },
            },
        };
        let score = trade
            .score(&auction.fee_policies, auction.native_prices)
            .context("failed to compute score")?;

        let token_pair = DirectedTokenPair {
            sell: trade.sell.token,
            buy: trade.buy.token,
        };
        eprintln!("         - score_by_token_pair {}: (buy={}, sell={}) - score: {}", solution.solver.0, token_pair.buy.0, token_pair.sell.0, score);
        *scores.entry(token_pair).or_default() += Score(score);
    }
    Ok(scores)
}

pub struct Config {
    pub max_winners: usize,
    pub weth: WrappedNativeToken,
}

/// Relevant data from `domain::Auction` but with data structures
/// optimized for the winner selection logic.
/// Avoids clones whenever possible.
struct Auction<'a> {
    /// Fee policies for **all** orders that were in the original auction.
    fee_policies: HashMap<OrderUid, &'a Vec<fee::Policy>>,
    surplus_capturing_jit_order_owners: HashSet<eth::Address>,
    native_prices: &'a Prices,
}

impl Auction<'_> {
    /// Returns whether an order is allowed to capture surplus and
    /// therefore contributes to the total score of a solution.
    fn contributes_to_score(&self, uid: &OrderUid) -> bool {
        self.fee_policies.contains_key(uid)
            || self
                .surplus_capturing_jit_order_owners
                .contains(&uid.owner())
    }
}

impl<'a> From<&'a domain::Auction> for Auction<'a> {
    fn from(original: &'a domain::Auction) -> Self {
        Self {
            fee_policies: original
                .orders
                .iter()
                .map(|o| (o.uid, &o.protocol_fees))
                .collect(),
            native_prices: &original.prices,
            surplus_capturing_jit_order_owners: original
                .surplus_capturing_jit_order_owners
                .iter()
                .cloned()
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct DirectedTokenPair {
    sell: eth::TokenAddress,
    buy: eth::TokenAddress,
}

/// Key to uniquely identify every solution.
#[derive(PartialEq, Eq, std::hash::Hash)]
struct SolutionKey {
    driver: eth::Address,
    solution_id: u64,
}

/// Scores of all trades in a solution aggregated by the directional
/// token pair. E.g. all trades (WETH -> USDC) are aggregated into
/// one value and all trades (USDC -> WETH) into another.
type ScoreByDirection = HashMap<DirectedTokenPair, Score>;

/// Mapping from solution to `DirectionalScores` for all solutions
/// of the auction.
type ScoresBySolution = HashMap<SolutionKey, ScoreByDirection>;

#[cfg(test)]
mod tests {
    use {
        crate::{
            domain::{
                auction::{
                    order::{self, AppDataHash}, Price
                }, competition::{
                    winner_selection::Arbitrator, Participant, Score, Solution, TradedOrder, Unranked
                }, eth::{self, TokenAddress}, Auction, Order, OrderUid
            },
            infra::Driver,
        },
        ethcontract::H160,
        hex_literal::hex,
        number::serialization::HexOrDecimalU256,
        serde::Deserialize,
        serde_json::json,
        serde_with::serde_as,
        std::{
            collections::{HashMap, HashSet}, hash::{DefaultHasher, Hash, Hasher}, panic::{catch_unwind, AssertUnwindSafe}, str::FromStr
        },
    };

    const DEFAULT_TOKEN_PRICE: u128 = 1_000;

    #[test]
    fn compare_python_rust_results() {
        use std::collections::HashMap;
        use std::fs::File;
        use std::io::BufReader;
        use csv::ReaderBuilder;
        use dotenv::dotenv;

        // Load environment variables from .env file
        dotenv().ok();

        // Hardcoded values
        let network = "mainnet-prod";
        let auction_start = 10606372;
        let auction_end = 10706372;

        // Get data folder path
        let data_folder = std::env::var("DATA_FOLDER").expect("DATA_FOLDER must be set in .env file");
        let data_path = std::path::Path::new(&data_folder);

        // Construct file paths
        let python_file = data_path.join(format!("python_results_{}_{}_{}.csv", network, auction_start, auction_end));
        let rust_file = data_path.join(format!("rust_results_{}_{}_{}.csv", network, auction_start, auction_end));

        eprintln!("Comparing Python results from: {}", python_file.display());
        eprintln!("With Rust results from: {}", rust_file.display());

        // Read Python results
        let mut python_results: HashMap<i64, Vec<(String, String)>> = HashMap::new();
        let file = File::open(python_file).expect("Failed to open Python result file");
        let reader = BufReader::new(file);
        let mut rdr = ReaderBuilder::new()
            .has_headers(true)
            .from_reader(reader);

        for result in rdr.records() {
            let record = result.expect("Failed to read Python CSV record");
            let auction_id: i64 = record[0].parse().expect("Failed to parse auction_id");
            let winner = record[1].to_string();
            let reference_score = record[2].to_string();
            python_results
                .entry(auction_id)
                .or_default()
                .push((winner, reference_score));
        }

        // Read Rust results
        let mut rust_results: HashMap<i64, Vec<(String, String)>> = HashMap::new();
        let mut multi_winners = 0;
        let mut db_mismatches = 0;
        let mut total_rust_auctions = 0;
        let file = File::open(rust_file).expect("Failed to open Rust result file");
        let reader = BufReader::new(file);
        let mut rdr = ReaderBuilder::new()
            .has_headers(true)
            .from_reader(reader);

        for result in rdr.records() {
            let record = result.expect("Failed to read Rust CSV record");
            let auction_id: i64 = record[0].parse().expect("Failed to parse auction_id");
            let winner = record[1].to_string();
            let same_as_db = record[2].parse::<bool>().expect("Failed to parse same_as_db");
            let reference_score = record[3].to_string();
            let num_winners: usize = record[4].parse().expect("Failed to parse num_winners");
            
            total_rust_auctions += 1;
            if num_winners > 1 {
                multi_winners += 1;
            }
            if !same_as_db {
                db_mismatches += 1;
            }
            
            rust_results
                .entry(auction_id)
                .or_default()
                .push((winner, reference_score));
        }

        // Compare results
        let mut matching = 0;
        let mut total_compared = 0;
        let mut differences = Vec::new();

        for (auction_id, py_winners) in &python_results {
            if let Some(rs_winners) = rust_results.get(auction_id) {
                total_compared += 1;
                
                // Sort both lists to ensure consistent comparison
                let mut py_winners = py_winners.clone();
                let mut rs_winners = rs_winners.clone();
                py_winners.sort();
                rs_winners.sort();

                if py_winners == rs_winners {
                    matching += 1;
                } else {
                    differences.push(format!(
                        "Auction {}: Python winners={:?} vs Rust winners={:?}",
                        auction_id, py_winners, rs_winners
                    ));
                }
            }
        }

        // Print summary
        eprintln!("\nComparison Summary:");
        eprintln!("Total auctions compared: {}", total_compared);
        eprintln!("Matching results: {} ({:.2}%)", matching, (matching as f64 / total_compared as f64) * 100.0);
        
        eprintln!("\nRust Implementation Statistics:");
        eprintln!("Total Rust auctions: {}", total_rust_auctions);
        eprintln!("Multi-winner auctions: {} ({:.2}%)", multi_winners, (multi_winners as f64 / total_rust_auctions as f64) * 100.0);
        eprintln!("DB winner mismatches: {} ({:.2}%)", db_mismatches, (db_mismatches as f64 / total_rust_auctions as f64) * 100.0);
        
        if !differences.is_empty() {
            eprintln!("\nDifferences found:");
            for diff in differences {
                eprintln!("{}", diff);
            }
        }

        // Print file coverage
        let python_only = python_results.keys().filter(|k| !rust_results.contains_key(k)).count();
        let rust_only = rust_results.keys().filter(|k| !python_results.contains_key(k)).count();
        
        eprintln!("\nFile Coverage:");
        eprintln!("Auctions only in Python file: {}", python_only);
        eprintln!("Auctions only in Rust file: {}", rust_only);
    }

    #[test]
    fn store_auction_results() {
        // Load environment variables from .env file
        dotenv::dotenv().ok();

        //let auction_start = 10606372;
        //let auction_end = 10706372;
        let auction_start = 10614525;
        let auction_end = 10614525;
        let network = "mainnet-prod";
        const BATCH_SIZE: i64 = 1000; // Process 1000 auctions at a time

        eprintln!("Starting test with auction range {} to {}", auction_start, auction_end);
        
        // Get data folder path
        let data_folder = std::env::var("DATA_FOLDER").expect("DATA_FOLDER must be set in .env file");
        let file_name = format!("rust_results_{}_{}_{}.csv", network, auction_start, auction_end);
        let file_path = std::path::Path::new(&data_folder).join(file_name);
        let mut writer = csv::Writer::from_path(file_path).expect("Failed to create CSV file");

        // Write header
        writer.write_record(&["auction_id", "winner", "same_as_db", "reference_score", "num_winners", "error"])
            .expect("Failed to write CSV header");

        // Process auctions in batches
        for batch_start in (auction_start..=auction_end).step_by(BATCH_SIZE as usize) {
            let batch_end = std::cmp::min(batch_start + BATCH_SIZE - 1, auction_end);
            eprintln!("Processing batch from {} to {}", batch_start, batch_end);

            // fetch db data for current batch
            eprintln!("fetching trade data for batch");
            let db_trade_data = fetch_trade_data(batch_start, batch_end);
            eprintln!("Trade data fetched, got {} auctions", db_trade_data.len());
            
            eprintln!("fetching auction data for batch");
            let db_auction_data = fetch_auction_orders_data(batch_start, batch_end);
            eprintln!("Auction data fetched, got {} auctions", db_auction_data.len());

            // Process each auction in the batch
            for auction_id in batch_start..=batch_end {
                if auction_id % 100 == 0 {
                    eprintln!("Processing auction {}", auction_id);
                }

                // Skip if we don't have both auction and trade data
                if !(db_trade_data.contains_key(&auction_id) && db_auction_data.contains_key(&auction_id)) {
                    continue;
                }

                let solutions = db_trade_data.get(&auction_id).unwrap();
                let auction = db_auction_data.get(&auction_id).unwrap();

                let mut test_case = TestCase {
                    tokens: vec![],
                    auction: auction.clone(),
                    solutions: solutions.clone(),
                    expected_fair_solutions: vec![],
                    expected_winners: vec![],
                    expected_reference_scores: HashMap::new(),
                };

                // Collect unique tokens from orders
                let mut unique_tokens = HashSet::new();
                for (_, order) in &test_case.auction.orders {
                    // sell token
                    unique_tokens.insert(order.1.clone());
                    // buy token
                    unique_tokens.insert(order.3.clone());
                }
                for token in unique_tokens {
                    test_case
                        .tokens
                        .push((token.clone(), H160::from_str(&token).unwrap()));
                }

                match test_case.calculate_results(auction_id) {
                    Ok(result) => {
                        let (auction_id, winners, same_winner, reference_scores, num_winners, error) = result;
                        // Write one line per winner with their reference score
                        for (winner, reference_score) in winners.iter().zip(reference_scores.iter()) {
                            writer.write_record(&[
                                auction_id.to_string(),
                                winner.clone(),
                                same_winner.to_string(),
                                reference_score.to_string(),
                                num_winners.to_string(),
                                error.clone().unwrap_or_default(),
                            ]).expect("Failed to write CSV record");
                        }
                    },
                    Err(e) => {
                        writer.write_record(&[
                            auction_id.to_string(),
                            String::new(),
                            false.to_string(),
                            "0".to_string(),
                            "0".to_string(),
                            e,
                        ]).expect("Failed to write CSV record");
                    }
                }
            }

            // Flush after each batch to ensure data is written
            writer.flush().expect("Failed to flush CSV writer");
            eprintln!("Completed batch from {} to {}", batch_start, batch_end);
        }

        eprintln!("Results written to rust_results_{}_{}_{}.csv", network, auction_start, auction_end);
    }

    fn fetch_trade_data(
        start_auction_id: i64,
        end_auction_id: i64,
    ) -> HashMap<i64, HashMap<String, TestSolution>> {
        use {
            dotenv::dotenv,
            sqlx::{PgPool, Row, types::BigDecimal},
            std::collections::HashMap,
        };

        // Load environment variables from .env file
        dotenv().ok();

        // Create a simple async block to run the database query
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // Get database URL from environment
            let database_url =
                std::env::var("DATABASE_URL").expect("DATABASE_URL must be set in .env file");

            // Create connection pool
            let pool = PgPool::connect(&database_url)
                .await
                .expect("Failed to create connection pool");

            eprintln!("Executing trade data query...");
            let query = format!(
                "WITH trade_data AS (
                    SELECT 
                        ps.*,
                        pte.order_uid,
                        COALESCE(o.sell_token, pjo.sell_token) AS sell_token,
                        COALESCE(o.buy_token, pjo.buy_token) AS buy_token,
                        pte.executed_sell AS executed_sell_amount,
                        pte.executed_buy AS executed_buy_amount,
                        COALESCE(o.sell_amount, pjo.limit_sell) AS limit_sell_amount,
                        COALESCE(o.buy_amount, pjo.limit_buy) AS limit_buy_amount,
                        COALESCE(o.kind, pjo.side) AS kind
                    FROM proposed_solutions ps
                    LEFT JOIN proposed_trade_executions pte
                        ON ps.auction_id = pte.auction_id 
                        AND ps.uid = pte.solution_uid
                    LEFT JOIN orders o
                        ON pte.order_uid = o.uid
                    LEFT JOIN proposed_jit_orders pjo
                        ON ps.auction_id = pjo.auction_id
                        AND ps.uid = pjo.solution_uid
                        AND pte.order_uid = pjo.order_uid
                    WHERE ps.auction_id BETWEEN {start_auction_id} AND {end_auction_id}
                )
                SELECT *
                FROM trade_data;"
            );

            // Execute the query with a timeout
            let start = std::time::Instant::now();
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(300),
                sqlx::query(&query).fetch_all(&pool)
            ).await;

            match result {
                Ok(Ok(rows)) => {
                    eprintln!("Query completed in {:?}", start.elapsed());
                    let mut auction_solutions: HashMap<i64, HashMap<String, TestSolution>> = HashMap::new();

                    for row in rows {
                        let auction_id = row.get::<i64, _>("auction_id");
                        let solver = hex::encode(row.get::<[u8; 20], _>("solver"));
                        let order_uid = hex::encode(row.get::<[u8; 56], _>("order_uid"));
                        let is_winner = row.get::<bool, _>("is_winner");

                        let mut trades = HashMap::new();
                        trades.insert(
                            order_uid,
                            TestTrade(
                                eth::U256::from_str_radix(
                                    &row.get::<BigDecimal, _>("executed_sell_amount").to_string(),
                                    10,
                                )
                                .unwrap(),
                                eth::U256::from_str_radix(
                                    &row.get::<BigDecimal, _>("executed_buy_amount").to_string(),
                                    10,
                                )
                                .unwrap(),
                            ),
                        );

                        let score = eth::U256::from_str_radix(
                            &row.get::<BigDecimal, _>("score").to_string(),
                            10,
                        )
                        .unwrap();

                        let solution = TestSolution {
                            solver: solver.clone(),
                            trades,
                            score,
                            is_winner,
                        };

                        // Get or create the solutions map for this auction
                        let solutions = auction_solutions
                            .entry(auction_id)
                            .or_insert_with(HashMap::new);

                        // Insert or update the solution for this solver
                        solutions.insert(solver, solution);
                    }
                    auction_solutions
                }
                Ok(Err(e)) => {
                    eprintln!("Query error: {}", e);
                    HashMap::new()
                }
                Err(e) => {
                    eprintln!("Query timeout: {}", e);
                    HashMap::new()
                }
            }
        })
    }

    fn fetch_auction_orders_data(start_auction_id: i64, end_auction_id: i64) -> HashMap<i64, TestAuction> {
        use {
            dotenv::dotenv,
            sqlx::{PgPool, Row, types::BigDecimal},
            std::collections::HashMap,
        };

        // Load environment variables from .env file
        dotenv().ok();

        // Create a simple async block to run the database query
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // Get database URL from environment
            let database_url =
                std::env::var("DATABASE_URL").expect("DATABASE_URL must be set in .env file");

            // Create connection pool
            let pool = PgPool::connect(&database_url)
                .await
                .expect("Failed to create connection pool");

            eprintln!("Executing auction data query...");
            let query = format!(
                "WITH auction_orders AS (
                    SELECT 
                        ao.auction_id,
                        unnest(ao.order_uids) as order_uid
                    FROM auction_orders ao
                    WHERE ao.auction_id BETWEEN {start_auction_id} AND {end_auction_id}
                )
                SELECT 
                    ao.auction_id,
                    ao.order_uid,
                    o.kind::text as side,
                    o.sell_token,
                    o.buy_token,
                    o.sell_amount,
                    o.buy_amount,
                    ap_sell.price as sell_token_price,
                    ap_buy.price as buy_token_price
                FROM auction_orders ao
                JOIN orders o ON ao.order_uid = o.uid
                JOIN auction_prices ap_sell ON ao.auction_id = ap_sell.auction_id AND o.sell_token = ap_sell.token
                JOIN auction_prices ap_buy ON ao.auction_id = ap_buy.auction_id AND o.buy_token = ap_buy.token
                ORDER BY ao.auction_id, ao.order_uid;"
            );

            // Execute the query with a timeout
            let start = std::time::Instant::now();
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(300),
                sqlx::query(&query).fetch_all(&pool)
            ).await;

            match result {
                Ok(Ok(rows)) => {
                    eprintln!("Query completed in {:?}", start.elapsed());
                    let mut auction_data: HashMap<i64, TestAuction> = HashMap::new();

                    for row in rows {
                        let auction_id = row.get::<i64, _>("auction_id");
                        let order_uid = hex::encode(row.get::<[u8; 56], _>("order_uid"));
                        let sell_token = hex::encode(row.get::<[u8; 20], _>("sell_token"));
                        let buy_token = hex::encode(row.get::<[u8; 20], _>("buy_token"));

                        // Get or create the TestAuction for this auction_id
                        let auction = auction_data
                            .entry(auction_id)
                            .or_insert_with(|| TestAuction {
                                orders: HashMap::new(),
                                prices: Some(HashMap::new()),
                            });

                        // Create order entry
                        auction.orders.insert(
                            order_uid.clone(),
                            TestOrder(
                                match row.get::<String, _>("side").to_lowercase().as_str() {
                                    "buy" => order::Side::Buy,
                                    "sell" => order::Side::Sell,
                                    _ => panic!(
                                        "Invalid side value: {}",
                                        row.get::<String, _>("side")
                                    ),
                                },
                                sell_token.clone(),
                                eth::U256::from_str_radix(
                                    &row.get::<BigDecimal, _>("sell_amount").to_string(),
                                    10,
                                )
                                .unwrap(),
                                buy_token.clone(),
                                eth::U256::from_str_radix(
                                    &row.get::<BigDecimal, _>("buy_amount").to_string(),
                                    10,
                                )
                                .unwrap(),
                            ),
                        );

                        // Add token prices if not already present
                        if let Some(prices) = &mut auction.prices {
                            if !prices.contains_key(&sell_token) {
                                prices.insert(
                                    sell_token,
                                    eth::U256::from_str_radix(
                                        &row.get::<BigDecimal, _>("sell_token_price").to_string(),
                                        10,
                                    )
                                    .unwrap(),
                                );
                            }
                            if !prices.contains_key(&buy_token) {
                                prices.insert(
                                    buy_token,
                                    eth::U256::from_str_radix(
                                        &row.get::<BigDecimal, _>("buy_token_price").to_string(),
                                        10,
                                    )
                                    .unwrap(),
                                );
                            }
                        }
                    }

                    auction_data
                }
                Ok(Err(e)) => {
                    eprintln!("Query error: {}", e);
                    HashMap::new()
                }
                Err(e) => {
                    eprintln!("Query timeout: {}", e);
                    HashMap::new()
                }
            }
        })
    }

    #[serde_as]
    #[derive(Deserialize, Debug)]
    struct TestCase {
        pub tokens: Vec<(String, H160)>,
        pub auction: TestAuction,
        pub solutions: HashMap<String, TestSolution>,
        pub expected_fair_solutions: Vec<String>,
        pub expected_winners: Vec<String>,
        #[serde_as(as = "HashMap<_, HexOrDecimalU256>")]
        pub expected_reference_scores: HashMap<String, eth::U256>,
    }

    impl TestCase {
        pub fn from_json(value: serde_json::Value) -> Self {
            serde_json::from_value(value).unwrap()
        }

        pub fn validate(&self) {
            let arbitrator = create_test_arbitrator();

            // map (token id -> token address) for later reference during the test
            let token_map: HashMap<String, H160> = self.tokens.iter().cloned().collect();

            // map (order id -> order) for later reference during the test
            let order_map: HashMap<String, Order> = self
                .auction
                .orders
                .iter()
                .map(
                    |(
                        order_id,
                        TestOrder(side, sell_token, sell_token_amount, buy_token, buy_token_amount),
                    )| {
                        let order_uid = hash(order_id);
                        let sell_token = token_map.get(sell_token).unwrap();
                        let buy_token = token_map.get(buy_token).unwrap();
                        let order = create_order(
                            order_uid,
                            *sell_token,
                            *sell_token_amount,
                            *buy_token,
                            *buy_token_amount,
                            *side,
                        );
                        (order_id.clone(), order)
                    },
                )
                .collect();

            let orders = order_map.values().cloned().collect();
            let prices = self.auction.prices.as_ref().map(|prices| {
                prices
                    .iter()
                    .map(|(token_id, price)| {
                        let token_address = TokenAddress(*token_map.get(token_id).unwrap());
                        let price = create_price(*price);
                        (token_address, price)
                    })
                    .collect()
            });

            let auction = create_auction(orders, prices);

            // map (solver id -> solver address) for later reference during the test
            let mut solver_map = HashMap::new();

            // map (solution id -> participant) for later reference during the test
            let mut solution_map = HashMap::new();
            for (solution_id, solution) in &self.solutions {
                let solver_address = H160::from_str(&solution.solver).unwrap();
                solver_map.insert(solution.solver.clone(), solver_address);

                let trades: Vec<(OrderUid, TradedOrder)> = solution
                    .trades
                    .iter()
                    .filter_map(|(order_id, trade)| {
                        order_map.get(order_id).map(|order| {
                            let sell_token_amount = trade.0;
                            let buy_token_amount = trade.1;
                            let trade = create_trade(order, sell_token_amount, buy_token_amount);
                            (order.uid, trade)
                        })
                    })
                    .collect();

                let solution_uid = hash(solution_id);
                solution_map.insert(
                    solution_id,
                    create_solution(solution_uid, solver_address, solution.score, trades, None),
                );
            }

            // filter solutions
            let participants = solution_map.values().cloned().collect();
            let solutions = arbitrator.filter_unfair_solutions(participants, &auction);

            // select the winners
            let solutions = arbitrator.mark_winners(solutions);
            let winners = filter_winners(&solutions);

            // Check that winners match the database
            let _expected_winners: Vec<H160> = self
                .solutions
                .iter()
                .filter(|(_, solution)| solution.is_winner)
                .map(|(_, solution)| solution.solver.clone())
                .map(|solver| H160::from_str(&solver).unwrap())
                .collect();

            // compute reference score
            let _reference_scores = arbitrator.compute_reference_scores(&solutions);
        }

        pub fn calculate_results(&self, auction_id: i64) -> Result<(i64, Vec<String>, bool, Vec<eth::U256>, usize, Option<String>), String> {
            use std::panic::{catch_unwind, AssertUnwindSafe};
            use anyhow::Context;

            let result = catch_unwind(AssertUnwindSafe(|| -> anyhow::Result<_> {
                let arbitrator = create_test_arbitrator();

                // map (token id -> token address) for later reference during the test
                let token_map: HashMap<String, H160> = self.tokens.iter().cloned().collect();

                // map (order id -> order) for later reference during the test
                let order_map: HashMap<String, Order> = self
                    .auction
                    .orders
                    .iter()
                    .map(
                        |(
                            order_id,
                            TestOrder(side, sell_token, sell_token_amount, buy_token, buy_token_amount),
                        )| {
                            let order_uid = hash(order_id);
                            let sell_token = token_map.get(sell_token).context("Missing sell_token in token_map").unwrap();
                            let buy_token = token_map.get(buy_token).context("Missing buy_token in token_map").unwrap();
                            let order = create_order(
                                order_uid,
                                *sell_token,
                                *sell_token_amount,
                                *buy_token,
                                *buy_token_amount,
                                *side,
                            );
                            (order_id.clone(), order)
                        },
                    )
                    .collect();

                let orders = order_map.values().cloned().collect();
                let prices = self.auction.prices.as_ref().map(|prices| {
                    prices
                        .iter()
                        .map(|(token_id, price)| {
                            let token_address = TokenAddress(*token_map.get(token_id).context("Missing token_id in token_map").unwrap());
                            let price = create_price(*price);
                            (token_address, price)
                        })
                        .collect()
                });

                let auction = create_auction(orders, prices);

                // map (solver id -> solver address) for later reference during the test
                let mut solver_map = HashMap::new();

                // map (solution id -> participant) for later reference during the test
                let mut solution_map = HashMap::new();
                for (solution_id, solution) in &self.solutions {
                    let solver_address = H160::from_str(&solution.solver).context("Invalid solver address").unwrap();
                    solver_map.insert(solution.solver.clone(), solver_address);

                    // Filter out trades with missing orders
                    let trades: Vec<(OrderUid, TradedOrder)> = solution
                        .trades
                        .iter()
                        .filter_map(|(order_id, trade)| {
                            order_map.get(order_id).map(|order| {
                                let sell_token_amount = trade.0;
                                let buy_token_amount = trade.1;
                                let trade = create_trade(order, sell_token_amount, buy_token_amount);
                                (order.uid, trade)
                            })
                        })
                        .collect();

                    // Skip solutions with no valid trades
                    if trades.is_empty() {
                        continue;
                    }

                    let solution_uid = hash(solution_id);
                    solution_map.insert(
                        solution_id,
                        create_solution(solution_uid, solver_address, solution.score, trades, None),
                    );
                    eprintln!("Solution id:{}, solver:{}, score:{}", solution_id, solver_address, solution.score);
                }

                // Skip if no valid solutions
                if solution_map.is_empty() {
                    return Ok((
                        auction_id,
                        vec![],
                        false,
                        vec![],
                        0,
                        Some("No valid solutions found".to_string()),
                    ));
                }

                // filter solutions
                let participants = solution_map.values().cloned().collect();
                let solutions = arbitrator.filter_unfair_solutions(participants, &auction);
                eprintln!("********** after filtering unfair solutions:");
                for solution in &solutions {
                    eprintln!("Solution id:{}, solver:{}, score:{}", solution.solution().id, solution.driver().submission_address, solution.solution().score);
                }
                
                // select the winners
                let solutions = arbitrator.mark_winners(solutions);
                let winners = filter_winners(&solutions);

                eprintln!("********** winners:");
                for solution in &winners {
                    eprintln!("Solution id:{}, solver:{}, score:{}", solution.solution().id, solution.driver().submission_address, solution.solution().score);
                }

                // Get the winners from our calculation
                let calculated_winners: Vec<String> = winners.iter()
                    .map(|w| hex::encode(w.driver().submission_address.0))
                    .collect();

                // Get the winner from the database
                let db_winner = self.solutions.iter()
                    .find(|(_, solution)| solution.is_winner)
                    .map(|(_, solution)| solution.solver.clone())
                    .unwrap_or_default();

                // compute reference scores
                let reference_scores = arbitrator.compute_reference_scores(&solutions);
                
                let reference_scores: Vec<eth::U256> = winners.iter()
                    .map(|winner| {
                        reference_scores.get(&winner.driver().submission_address)
                            .map(|score| score.get().0)
                            .unwrap_or_default()
                    })
                    .collect();

                Ok((
                    auction_id,
                    calculated_winners.clone(),
                    calculated_winners.contains(&db_winner),
                    reference_scores,
                    winners.len(),
                    None,
                ))
            }));

            match result {
                Ok(Ok(val)) => Ok(val),
                Ok(Err(e)) => Err(format!("Error: {e:?}")),
                Err(e) => Err(match e.downcast_ref::<&str>() {
                    Some(s) => s.to_string(),
                    None => match e.downcast_ref::<String>() {
                        Some(s) => s.clone(),
                        None => "Unknown panic".to_string(),
                    },
                }),
            }
        }
    }

    #[serde_as]
    #[derive(Deserialize, Debug, Clone)]
    struct TestAuction {
        pub orders: HashMap<String, TestOrder>,
        #[serde(default)]
        #[serde_as(as = "Option<HashMap<_, HexOrDecimalU256>>")]
        pub prices: Option<HashMap<String, eth::U256>>,
    }

    #[serde_as]
    #[derive(Deserialize, Debug, Clone)]
    struct TestOrder(
        // side,
        #[serde(deserialize_with = "deserialize_side")] pub order::Side,
        // sell_token
        pub String,
        // sell_amount
        #[serde_as(as = "HexOrDecimalU256")] pub eth::U256,
        // buy_token
        pub String,
        // buy_amount
        #[serde_as(as = "HexOrDecimalU256")] pub eth::U256,
    );

    #[derive(Deserialize, Debug, Clone)]
    struct TestSolution {
        pub solver: String,
        pub trades: HashMap<String, TestTrade>,
        pub score: eth::U256,
        #[serde(default)]
        pub is_winner: bool,
    }

    #[serde_as]
    #[derive(Deserialize, Debug, Clone)]
    struct TestTrade(
        // sell_amount
        #[serde_as(as = "HexOrDecimalU256")] pub eth::U256,
        // buy_amount
        #[serde_as(as = "HexOrDecimalU256")] pub eth::U256,
    );

    fn create_test_arbitrator() -> super::Config {
        super::Config {
            max_winners: 10,
            weth: H160::from_slice(&hex!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")).into(),
        }
    }

    fn address(id: u64) -> H160 {
        H160::from_low_u64_le(id)
    }

    fn create_order(
        uid: u64,
        sell_token: H160,
        sell_amount: eth::U256,
        buy_token: H160,
        buy_amount: eth::U256,
        side: order::Side,
    ) -> Order {
        Order {
            uid: create_order_uid(uid),
            sell: eth::Asset {
                amount: sell_amount.into(),
                token: sell_token.into(),
            },
            buy: eth::Asset {
                amount: buy_amount.into(),
                token: buy_token.into(),
            },
            protocol_fees: vec![],
            side,
            receiver: None,
            owner: Default::default(),
            partially_fillable: false,
            executed: eth::U256::zero().into(),
            pre_interactions: vec![],
            post_interactions: vec![],
            sell_token_balance: order::SellTokenSource::Erc20,
            buy_token_balance: order::BuyTokenDestination::Erc20,
            app_data: AppDataHash(hex!(
                "6000000000000000000000000000000000000000000000000000000000000007"
            )),
            created: Default::default(),
            valid_to: Default::default(),
            signature: order::Signature::PreSign,
            quote: None,
        }
    }

    // Deterministically creates an OrderUid from a u64.
    fn create_order_uid(uid: u64) -> OrderUid {
        let mut encoded_uid = [0u8; 56];
        let uid_bytes = uid.to_le_bytes();
        encoded_uid[..uid_bytes.len()].copy_from_slice(&uid_bytes);
        OrderUid(encoded_uid)
    }

    fn create_price(value: eth::U256) -> Price {
        Price::try_new(eth::Ether(value)).unwrap()
    }

    fn create_auction(
        orders: Vec<Order>,
        prices: Option<HashMap<eth::TokenAddress, Price>>,
    ) -> Auction {
        // Initialize the prices of the tokens if they are not specified
        let prices = prices.unwrap_or({
            let default_price = create_price(DEFAULT_TOKEN_PRICE.into());
            let mut res = HashMap::new();
            for order in &orders {
                res.insert(order.buy.token, default_price);
                res.insert(order.sell.token, default_price);
            }
            res
        });

        Auction {
            id: 0,
            block: 0,
            orders,
            prices,
            surplus_capturing_jit_order_owners: vec![],
        }
    }

    fn create_trade(
        order: &Order,
        executed_sell: eth::U256,
        executed_buy: eth::U256,
    ) -> TradedOrder {
        TradedOrder {
            side: order.side,
            sell: order.sell,
            buy: order.buy,
            executed_sell: executed_sell.into(),
            executed_buy: executed_buy.into(),
        }
    }

    fn create_solution(
        solution_id: u64,
        solver_address: H160,
        score: eth::U256,
        trades: Vec<(OrderUid, TradedOrder)>,
        prices: Option<HashMap<TokenAddress, Price>>,
    ) -> Participant<Unranked> {
        // The prices of the tokens do not affect the result but they keys must exist
        // for every token of every trade
        let prices = prices.unwrap_or({
            let mut res = HashMap::new();
            for (_, trade) in &trades {
                res.insert(trade.buy.token, create_price(eth::U256::one()));
                res.insert(trade.sell.token, create_price(eth::U256::one()));
            }
            res
        });

        let trade_order_map: HashMap<OrderUid, TradedOrder> = trades.into_iter().collect();
        let solver_address = eth::Address(solver_address);

        let solution = Solution::new(
            solution_id,
            solver_address,
            Score(eth::Ether(score)),
            trade_order_map,
            prices,
        );

        Participant::new(
            solution,
            Driver::mock(solver_address.to_string(), solver_address).into(),
        )
    }

    fn amount(value: u128) -> String {
        // adding decimal units to avoid the math rounding it down to 0
        to_e15(value).to_string()
    }

    fn to_e15(value: u128) -> u128 {
        value * 10u128.pow(15)
    }

    fn score(score: u128) -> String {
        // adding decimal units to avoid the math rounding it down to 0
        let score: u128 = to_e15(score);
        eth::U256::from(score)
            // Scores must be denominated in buy token price
            .checked_mul(DEFAULT_TOKEN_PRICE.into()).unwrap()
            // and expresed in wei
            .checked_div(10_u128.pow(18).into()).unwrap()
            .to_string()
    }

    fn filter_winners(solutions: &[Participant]) -> Vec<&Participant> {
        solutions.iter().filter(|s| s.is_winner()).collect()
    }

    // Used to generate deterministic identifiers (e.g., UIDs, addresses) from
    // string descriptions.
    fn hash(s: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        hasher.finish()
    }

    // Needed to automatically deserialize order::Side in JSON test cases
    fn deserialize_side<'de, D>(deserializer: D) -> Result<order::Side, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        match s.to_lowercase().as_str() {
            "buy" => Ok(order::Side::Buy),
            "sell" => Ok(order::Side::Sell),
            _ => Err(serde::de::Error::custom(format!("Invalid side: {}", s))),
        }
    }

    /// Splits a solution into multiple solutions based on token pairs and applies efficiency loss.
    /// This is equivalent to the Python implementation's compute_split_solution function.
    fn compute_split_solution(
        solution: &Participant<Unranked>,
        efficiency_loss: f64,
    ) -> Vec<Participant<Unranked>> {
        let mut split_solutions = vec![solution.clone()];
        
        // Group trades by token pairs
        let mut trades_by_pair: HashMap<(eth::TokenAddress, eth::TokenAddress), Vec<(OrderUid, TradedOrder)>> = HashMap::new();
        for (uid, trade) in solution.solution().orders() {
            let pair = (trade.sell.token, trade.buy.token);
            trades_by_pair.entry(pair).or_default().push((*uid, trade.clone()));
        }

        // If we have multiple token pairs, create split solutions
        if trades_by_pair.len() > 1 {
            for (pair, trades) in trades_by_pair {
                // Calculate adjusted score for this token pair
                let score = trades.iter()
                    .map(|(_, trade)| {
                        let trade_score = trade.executed_buy.0
                            .checked_sub(trade.executed_sell.0)
                            .unwrap_or_default();
                        // Convert U256 to u128 for the calculation
                        let score_u128 = trade_score.as_u128();
                        (score_u128 as f64 * (1.0 - efficiency_loss)) as u128
                    })
                    .sum::<u128>();

                // Create new solution with just these trades
                let solution_uid = hash(&format!("{}-{:?}", solution.solution().id(), pair));
                let mut trade_map = HashMap::new();
                for (uid, trade) in trades {
                    trade_map.insert(uid, trade);
                }

                let split_solution = create_solution(
                    solution_uid,
                    solution.driver().submission_address.0,
                    eth::U256::from(score),
                    trade_map.into_iter().collect(),
                    None,
                );

                split_solutions.push(split_solution);
            }
        }

        split_solutions
    }
}
