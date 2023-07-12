//! Framework for setting up tests.

use {
    self::{blockchain::Fulfillment, driver::Driver, solver::Solver},
    crate::{
        domain::{competition::order, eth},
        infra::time,
        tests::{
            cases::{
                AB_ORDER_AMOUNT,
                CD_ORDER_AMOUNT,
                DEFAULT_POOL_AMOUNT_A,
                DEFAULT_POOL_AMOUNT_B,
                DEFAULT_POOL_AMOUNT_C,
                DEFAULT_POOL_AMOUNT_D,
                DEFAULT_SCORE_MAX,
                DEFAULT_SCORE_MIN,
                DEFAULT_SURPLUS_FACTOR,
                DEFAULT_SURPLUS_FEE,
                ETH_ORDER_AMOUNT,
            },
            setup::blockchain::Blockchain,
        },
        util,
    },
    ethcontract::BlockId,
    hyper::StatusCode,
    itertools::Itertools,
    secp256k1::SecretKey,
    std::{
        collections::{HashMap, HashSet},
        path::PathBuf,
        str::FromStr,
    },
    web3::types::TransactionId,
};

mod blockchain;
mod driver;
mod solver;

#[derive(Debug, Clone, Copy)]
pub struct Asset {
    token: &'static str,
    amount: eth::U256,
}

/// Set up a difference between the placed order amounts and the amounts
/// executed by the solver. This is useful for testing e.g. asset flow
/// verification. See [`crate::domain::competition::solution::Settlement`].
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionDiff {
    // TODO I think only increase_sell and decrease_buy make sense
    /// Increase the sell amount executed by the solver by the specified amount.
    pub increase_sell: eth::U256,
    /// Decrease the sell amount executed by the solver by the specified amount.
    pub decrease_sell: eth::U256,
    /// Increase the buy amount executed by the solver by the specified amount.
    pub increase_buy: eth::U256,
    /// Decrease the buy amount executed by the solver by the specified amount.
    pub decrease_buy: eth::U256,
}

impl ExecutionDiff {
    pub fn increase_sell() -> Self {
        Self {
            increase_sell: 300.into(),
            ..Default::default()
        }
    }

    pub fn decrease_buy() -> Self {
        Self {
            decrease_buy: 300.into(),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Order {
    pub name: &'static str,

    pub sell_amount: eth::U256,
    pub sell_token: &'static str,
    pub buy_token: &'static str,

    pub internalize: bool,
    pub side: order::Side,
    pub partial: order::Partial,
    pub valid_for: util::Timestamp,
    pub kind: order::Kind,

    // TODO For now I'll always set these to zero. But I think they should be tested as well.
    // Figure out what (if anything) would constitute meaningful tests for these values.
    pub user_fee: eth::U256,
    pub solver_fee: eth::U256,

    /// Set a value to be used to divide the order buy or sell amount before
    /// the order gets placed and thereby generate surplus. Whether the sell or
    /// buy amount is divided depends on the order side. This is necessary to
    /// keep the solution scores positive.
    pub surplus_factor: eth::U256,
    pub execution_diff: ExecutionDiff,
    /// Override the executed amount of the order. Useful for testing liquidity
    /// orders. Otherwise [`execution_diff`] is probably more suitable.
    pub executed: Option<eth::U256>,
}

impl Order {
    /// Rename the order.
    pub fn rename(self, name: &'static str) -> Self {
        Self { name, ..self }
    }

    /// Reduce the amount of this order by the given amount.
    pub fn reduce_amount(self, diff: eth::U256) -> Self {
        Self {
            sell_amount: self.sell_amount - diff,
            ..self
        }
    }

    /// Ensure that this order generates no surplus, and therefore most likely
    /// has a negative score.
    pub fn no_surplus(self) -> Self {
        Self {
            surplus_factor: 1.into(),
            ..self
        }
    }

    /// Mark this order as internalizable.
    pub fn internalize(self) -> Self {
        Self {
            internalize: true,
            ..self
        }
    }

    /// Set the order kind.
    pub fn kind(self, kind: order::Kind) -> Self {
        Self { kind, ..self }
    }

    /// Set the order side.
    pub fn side(self, side: order::Side) -> Self {
        Self { side, ..self }
    }

    /// Make this a limit order.
    pub fn limit(self) -> Self {
        Self {
            kind: order::Kind::Limit {
                surplus_fee: eth::U256::from(DEFAULT_SURPLUS_FEE).into(),
            },
            ..self
        }
    }

    /// Make the amounts executed by the solver less than the amounts placed as
    /// part of the order.
    pub fn execution_diff(self, diff: ExecutionDiff) -> Self {
        Self {
            execution_diff: diff,
            ..self
        }
    }

    /// Increase the order valid_to value by one. This is useful for changing
    /// the order UID during testing without changing any of the order
    /// semantics.
    pub fn increase_valid_to(self) -> Self {
        Self {
            valid_for: (self.valid_for.0 + 1).into(),
            ..self
        }
    }

    fn surplus_fee(&self) -> eth::U256 {
        match self.kind {
            order::Kind::Limit { surplus_fee } => surplus_fee.0,
            _ => 0.into(),
        }
    }
}

impl Default for Order {
    fn default() -> Self {
        Self {
            sell_amount: Default::default(),
            sell_token: Default::default(),
            buy_token: Default::default(),
            internalize: Default::default(),
            side: order::Side::Sell,
            partial: order::Partial::No,
            valid_for: 100.into(),
            kind: order::Kind::Market,
            user_fee: Default::default(),
            solver_fee: Default::default(),
            name: Default::default(),
            surplus_factor: DEFAULT_SURPLUS_FACTOR.into(),
            execution_diff: Default::default(),
            executed: Default::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pool {
    pub token_a: &'static str,
    pub token_b: &'static str,
    pub amount_a: eth::U256,
    pub amount_b: eth::U256,
}

/// Create a builder for the setup process.
pub fn setup() -> Setup {
    Setup {
        name: Default::default(),
        pools: Default::default(),
        orders: Default::default(),
        trusted: Default::default(),
        config_file: Default::default(),
        solutions: Default::default(),
        quote: Default::default(),
        fund_solver: true,
        enable_simulation: true,
    }
}

#[derive(Debug)]
pub struct Setup {
    name: Option<String>,
    pools: Vec<blockchain::Pool>,
    orders: Vec<Order>,
    trusted: HashSet<&'static str>,
    config_file: Option<PathBuf>,
    solutions: Vec<Solution>,
    /// Is this a test for the /quote endpoint?
    quote: bool,
    /// Should the solver be funded with ETH? True by default.
    fund_solver: bool,
    /// Should simulation be enabled? True by default.
    enable_simulation: bool,
}

/// The validity of a solution.
#[derive(Debug, Clone, Copy)]
pub enum Calldata {
    /// Set up the solver to return a solution with valid calldata.
    Valid {
        /// Include additional meaningless bytes appended to the calldata. This
        /// is useful for lowering the solution score in a controlled
        /// way.
        additional_bytes: usize,
    },
    /// Set up the solver to return a solution with bogus calldata.
    Invalid,
}

#[derive(Debug, Clone)]
pub struct Solution {
    pub calldata: Calldata,
    pub orders: Vec<&'static str>,
    pub risk: eth::U256,
}

impl Solution {
    /// Reduce the solution score by adding additional meaningless calldata.
    pub fn reduce_score(self) -> Self {
        Self {
            calldata: match self.calldata {
                Calldata::Valid { .. } => Calldata::Valid {
                    additional_bytes: 10,
                },
                Calldata::Invalid => Calldata::Invalid,
            },
            ..self
        }
    }

    /// Make the solution return invalid calldata.
    pub fn invalid(self) -> Self {
        Self {
            calldata: Calldata::Invalid,
            ..self
        }
    }

    /// Set the solution risk.
    pub fn risk(self, risk: eth::U256) -> Self {
        Self { risk, ..self }
    }
}

impl Default for Solution {
    fn default() -> Self {
        Self {
            calldata: Calldata::Valid {
                additional_bytes: 0,
            },
            orders: Default::default(),
            risk: Default::default(),
        }
    }
}

/// A pool between tokens "A" and "B".
pub fn ab_pool() -> Pool {
    Pool {
        token_a: "A",
        token_b: "B",
        amount_a: DEFAULT_POOL_AMOUNT_A.into(),
        amount_b: DEFAULT_POOL_AMOUNT_B.into(),
    }
}

/// An example order which sells token "A" for token "B".
pub fn ab_order() -> Order {
    Order {
        name: "A-B order",
        sell_amount: AB_ORDER_AMOUNT.into(),
        sell_token: "A",
        buy_token: "B",
        ..Default::default()
    }
}

/// A solution solving the [`ab_order`].
pub fn ab_solution() -> Solution {
    Solution {
        calldata: Calldata::Valid {
            additional_bytes: 0,
        },
        orders: vec!["A-B order"],
        risk: Default::default(),
    }
}

/// A pool between tokens "C" and "D".
pub fn cd_pool() -> Pool {
    Pool {
        token_a: "C",
        token_b: "D",
        amount_a: DEFAULT_POOL_AMOUNT_C.into(),
        amount_b: DEFAULT_POOL_AMOUNT_D.into(),
    }
}

/// An example order which sells token "C" for token "D".
pub fn cd_order() -> Order {
    Order {
        name: "C-D order",
        sell_amount: CD_ORDER_AMOUNT.into(),
        sell_token: "C",
        buy_token: "D",
        ..Default::default()
    }
}

/// A solution solving the [`cd_order`].
pub fn cd_solution() -> Solution {
    Solution {
        calldata: Calldata::Valid {
            additional_bytes: 0,
        },
        orders: vec!["C-D order"],
        risk: Default::default(),
    }
}

/// A pool between "A" and "WETH".
pub fn weth_pool() -> Pool {
    Pool {
        token_a: "A",
        token_b: "WETH",
        amount_a: DEFAULT_POOL_AMOUNT_A.into(),
        amount_b: DEFAULT_POOL_AMOUNT_B.into(),
    }
}

/// An order which buys ETH.
pub fn eth_order() -> Order {
    Order {
        name: "ETH order",
        sell_amount: ETH_ORDER_AMOUNT.into(),
        sell_token: "A",
        buy_token: "ETH",
        ..Default::default()
    }
}

pub fn eth_solution() -> Solution {
    Solution {
        calldata: Calldata::Valid {
            additional_bytes: 0,
        },
        orders: vec!["ETH order"],
        risk: Default::default(),
    }
}

impl Setup {
    /// Set an explicit name for this test. If a name is set, it will be logged
    /// before the test runs.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Add a uniswap pool with the specified reserves. Tokens are identified
    /// by their symbols. Every order will be solved through one of the pools.
    pub fn pool(mut self, pool: Pool) -> Self {
        self.pools.push(blockchain::Pool {
            reserve_a: Asset {
                token: pool.token_a,
                amount: pool.amount_a,
            },
            reserve_b: Asset {
                token: pool.token_b,
                amount: pool.amount_b,
            },
        });
        self
    }

    /// Add a new order to be solved as part of the test. This order will be
    /// passed to /solve when [`Test::solve`] is called and it will be
    /// anticipated by the mock solver.
    pub fn order(mut self, order: Order) -> Self {
        self.orders.push(order);
        self
    }

    /// Set up the protocol to consider the specified token as trusted. The
    /// token is identified by its symbol.
    pub fn trust(mut self, token: &'static str) -> Self {
        self.trusted.insert(token);
        self
    }

    /// Load the specified config file. Otherwise, a temporary config file will
    /// be created with reasonable values.
    pub fn config(mut self, path: PathBuf) -> Self {
        self.config_file = Some(path);
        self
    }

    /// Add a solution to be returned by the mock solver.
    pub fn solution(mut self, solution: Solution) -> Self {
        self.solutions.push(solution);
        self
    }

    /// Don't fund the solver account with any ETH.
    pub fn defund_solver(mut self) -> Self {
        self.fund_solver = false;
        self
    }

    /// Disable simulating solutions during solving. Used to make testing easier
    /// when checking the asset flow and similar rules that don't depend on
    /// the blockchain.
    pub fn disable_simulation(mut self) -> Self {
        self.enable_simulation = false;
        self
    }

    /// Create the test: set up onchain contracts and pools, start a mock HTTP
    /// server for the solver and start the HTTP server for the driver.
    pub async fn done(self) -> Test {
        crate::boundary::initialize_tracing("driver=trace");

        if let Some(name) = self.name.as_ref() {
            tracing::warn!("\n***** [RUNNING TEST CASE] *****\n{name}");
        }

        let deadline = self.deadline();
        let Self {
            pools,
            orders,
            trusted,
            config_file,
            ..
        } = self;

        // Hardcoded trader account. Don't use this account for anything else!!!
        let trader_address =
            eth::H160::from_str("d2525C68A663295BBE347B65C87c8e17De936a0a").unwrap();
        let trader_secret_key = SecretKey::from_slice(
            &hex::decode("f9f831cee763ef826b8d45557f0f8677b27045e0e011bcd78571a40acc8a6cc3")
                .unwrap(),
        )
        .unwrap();

        // Hardcoded solver account. Don't use this account for anything else!!!
        let solver_address =
            eth::H160::from_str("72b92Ee5F847FBB0D243047c263Acd40c34A63B9").unwrap();
        let solver_secret_key = SecretKey::from_slice(
            &hex::decode("a131a35fb8f614b31611f4fe68b6fc538b0febd2f75cd68e1282d8fd45b63326")
                .unwrap(),
        )
        .unwrap();

        let relative_slippage = 0.3;
        let absolute_slippage = 183.into();

        // Create the necessary components for testing.
        let blockchain = Blockchain::new(blockchain::Config {
            pools,
            trader_address,
            trader_secret_key,
            solver_address,
            solver_secret_key,
            fund_solver: self.fund_solver,
        })
        .await;
        let mut solutions = Vec::new();
        for solution in self.solutions {
            let orders = solution
                .orders
                .iter()
                .map(|solution_order| orders.iter().find(|o| o.name == *solution_order).unwrap());
            solutions.push(blockchain.fulfill(orders, &solution).await);
        }
        let mut quotes = Vec::new();
        for order in orders {
            let quote = blockchain.quote(&order).await;
            quotes.push(quote);
        }
        let solver = Solver::new(solver::Config {
            blockchain: &blockchain,
            solutions: &solutions,
            trusted: &trusted,
            quoted_orders: &quotes,
            deadline,
            quote: self.quote,
        })
        .await;
        let driver = Driver::new(
            &driver::Config {
                config_file,
                relative_slippage,
                absolute_slippage,
                solver_address,
                solver_secret_key,
                enable_simulation: self.enable_simulation,
            },
            &solver,
            &blockchain,
        )
        .await;

        Test {
            blockchain,
            driver,
            client: Default::default(),
            trader_address,
            fulfillments: solutions.into_iter().flat_map(|s| s.fulfillments).collect(),
            trusted,
            deadline,
            quoted_orders: quotes,
            quote: self.quote,
        }
    }

    /// This is a test for the /quote endpoint.
    pub fn quote(self) -> Self {
        Self {
            quote: true,
            ..self
        }
    }

    fn deadline(&self) -> chrono::DateTime<chrono::Utc> {
        time::now() + chrono::Duration::days(30)
    }
}

pub struct Test {
    quoted_orders: Vec<blockchain::QuotedOrder>,
    blockchain: Blockchain,
    driver: Driver,
    client: reqwest::Client,
    trader_address: eth::H160,
    fulfillments: Vec<Fulfillment>,
    trusted: HashSet<&'static str>,
    deadline: chrono::DateTime<chrono::Utc>,
    /// Is this testing the /quote endpoint?
    quote: bool,
}

impl Test {
    /// Call the /solve endpoint.
    pub async fn solve(&self) -> Solve {
        let res = self
            .client
            .post(format!(
                "http://{}/{}/solve",
                self.driver.addr,
                solver::NAME
            ))
            .json(&driver::solve_req(self))
            .send()
            .await
            .unwrap();
        let status = res.status();
        let body = res.text().await.unwrap();
        tracing::debug!(?status, ?body, "got a response from /solve");
        Solve {
            status,
            body,
            fulfillments: &self.fulfillments,
            blockchain: &self.blockchain,
        }
    }

    /// Call the /quote endpoint.
    pub async fn quote(&self) -> Quote {
        if !self.quote {
            panic!("called /quote on a test which wasn't configured to test the /quote endpoint");
        }

        let res = self
            .client
            .post(format!(
                "http://{}/{}/quote",
                self.driver.addr,
                solver::NAME
            ))
            .json(&driver::quote_req(self))
            .send()
            .await
            .unwrap();
        let status = res.status();
        let body = res.text().await.unwrap();
        tracing::debug!(?status, ?body, "got a response from /quote");
        Quote {
            fulfillments: &self.fulfillments,
            status,
            body,
        }
    }

    /// Call the /settle endpoint.
    pub async fn settle(&self) -> Settle {
        let old_balances = self.balances().await;
        let res = blockchain::wait_for(
            &self.blockchain.web3,
            self.client
                .post(format!(
                    "http://{}/{}/settle",
                    self.driver.addr,
                    solver::NAME
                ))
                .send(),
        )
        .await
        .unwrap();
        let status = res.status();
        let body = res.text().await.unwrap();
        tracing::debug!(?status, ?body, "got a response from /settle");
        Settle {
            old_balances,
            status,
            test: self,
            body,
        }
    }

    async fn balances(&self) -> HashMap<&'static str, eth::U256> {
        let mut balances = HashMap::new();
        for (token, contract) in self.blockchain.tokens.iter() {
            let balance = contract
                .balance_of(self.trader_address)
                .call()
                .await
                .unwrap();
            balances.insert(*token, balance);
        }
        balances.insert(
            "WETH",
            self.blockchain
                .weth
                .balance_of(self.trader_address)
                .call()
                .await
                .unwrap(),
        );
        balances.insert(
            "ETH",
            self.blockchain
                .web3
                .eth()
                .balance(self.trader_address, None)
                .await
                .unwrap(),
        );
        balances
    }
}

/// A /solve response.
pub struct Solve<'a> {
    status: StatusCode,
    body: String,
    fulfillments: &'a [Fulfillment],
    blockchain: &'a Blockchain,
}

impl<'a> Solve<'a> {
    /// Expect the /solve endpoint to have returned a 200 OK response.
    pub fn ok(self) -> SolveOk<'a> {
        assert_eq!(self.status, hyper::StatusCode::OK);
        SolveOk {
            body: self.body,
            fulfillments: self.fulfillments,
            blockchain: self.blockchain,
        }
    }

    /// Expect the /solve endpoint to return a 400 BAD REQUEST response.
    pub fn err(self) -> SolveErr {
        assert_eq!(self.status, hyper::StatusCode::BAD_REQUEST);
        SolveErr { body: self.body }
    }
}

pub struct SolveOk<'a> {
    body: String,
    fulfillments: &'a [Fulfillment],
    blockchain: &'a Blockchain,
}

impl SolveOk<'_> {
    /// Ensure that the score in the response is within a certain range. The
    /// reason why this is a range is because small timing differences in
    /// the test can lead to the settlement using slightly different amounts
    /// of gas, which in turn leads to different scores.
    pub fn score(self, min: eth::U256, max: eth::U256) -> Self {
        let result: serde_json::Value = serde_json::from_str(&self.body).unwrap();
        assert!(result.is_object());
        assert_eq!(result.as_object().unwrap().len(), 3);
        assert!(result.get("score").is_some());
        let score = result.get("score").unwrap().as_str().unwrap();
        let score = eth::U256::from_dec_str(score).unwrap();
        assert!(score >= min, "score less than min {score} < {min}");
        assert!(score <= max, "score more than max {score} > {max}");
        self
    }

    /// Ensure that the score is within the default expected range.
    pub fn default_score(self) -> Self {
        self.score(DEFAULT_SCORE_MIN.into(), DEFAULT_SCORE_MAX.into())
    }

    /// Check that the solution contains the expected orders.
    pub fn orders(self, order_names: &[&str]) -> Self {
        let expected_order_uids = order_names
            .iter()
            .map(|name| {
                self.fulfillments
                    .iter()
                    .find(|f| f.quoted_order.order.name == *name)
                    .unwrap_or_else(|| {
                        panic!(
                            "unexpected orders {order_names:?}: fulfillment not found in {:?}",
                            self.fulfillments,
                        )
                    })
                    .quoted_order
                    .order_uid(self.blockchain)
                    .to_string()
            })
            .sorted()
            .collect_vec();
        let result: serde_json::Value = serde_json::from_str(&self.body).unwrap();
        assert!(result.is_object());
        assert_eq!(result.as_object().unwrap().len(), 3);
        assert!(result.get("orders").is_some());
        let order_uids = result
            .get("orders")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|order| order.as_str().unwrap().to_owned())
            .sorted()
            .collect_vec();
        assert_eq!(order_uids, expected_order_uids);
        self
    }
}

pub struct SolveErr {
    body: String,
}

impl SolveErr {
    /// Check the kind field in the error response.
    pub fn kind(self, expected_kind: &str) {
        let result: serde_json::Value = serde_json::from_str(&self.body).unwrap();
        assert!(result.is_object());
        assert_eq!(result.as_object().unwrap().len(), 2);
        assert!(result.get("kind").is_some());
        assert!(result.get("description").is_some());
        let kind = result.get("kind").unwrap().as_str().unwrap();
        assert_eq!(kind, expected_kind);
    }
}

/// A /quote response.
pub struct Quote<'a> {
    fulfillments: &'a [Fulfillment],
    status: StatusCode,
    body: String,
}

impl<'a> Quote<'a> {
    /// Expect the /quote endpoint to have returned a 200 OK response.
    pub fn ok(self) -> QuoteOk<'a> {
        assert_eq!(self.status, hyper::StatusCode::OK);
        QuoteOk {
            fulfillments: self.fulfillments,
            body: self.body,
        }
    }
}

pub struct QuoteOk<'a> {
    fulfillments: &'a [Fulfillment],
    body: String,
}

impl QuoteOk<'_> {
    /// Check that the quote returns the expected amount of tokens. This is
    /// based on the state of the blockchain and the test setup.
    pub fn amount(self) -> Self {
        assert_eq!(self.fulfillments.len(), 1);
        let fulfillment = &self.fulfillments[0];
        let result: serde_json::Value = serde_json::from_str(&self.body).unwrap();
        let amount = result.get("amount").unwrap().as_str().unwrap().to_owned();
        let expected = match fulfillment.quoted_order.order.side {
            order::Side::Buy => (fulfillment.quoted_order.sell
                - fulfillment.quoted_order.order.surplus_fee())
            .to_string(),
            order::Side::Sell => fulfillment.quoted_order.buy.to_string(),
        };
        assert_eq!(amount, expected);
        self
    }

    /// Check that the quote returns the expected interactions. This is
    /// based on the state of the blockchain and the test setup.
    pub fn interactions(self) -> Self {
        assert_eq!(self.fulfillments.len(), 1);
        let fulfillment = &self.fulfillments[0];
        let result: serde_json::Value = serde_json::from_str(&self.body).unwrap();
        let interactions = result
            .get("interactions")
            .unwrap()
            .as_array()
            .unwrap()
            .to_owned();
        assert_eq!(interactions.len(), fulfillment.interactions.len());
        for (interaction, expected) in interactions.iter().zip(&fulfillment.interactions) {
            let target = interaction.get("target").unwrap().as_str().unwrap();
            let value = interaction.get("value").unwrap().as_str().unwrap();
            let calldata = interaction.get("callData").unwrap().as_str().unwrap();
            assert_eq!(target, format!("0x{}", hex::encode(expected.address)));
            assert_eq!(value, "0");
            assert_eq!(calldata, format!("0x{}", hex::encode(&expected.calldata)));
        }
        self
    }
}

/// The expected difference between a previous user balance for a certain token
/// and the balance after the settlement has been broadcast.
#[derive(Debug, Clone, Copy)]
pub enum Balance {
    /// The balance should be greater than before.
    Greater,
    /// The balance should be smaller than before by an exact amount.
    SmallerBy(eth::U256),
}

/// A /settle response.
pub struct Settle<'a> {
    old_balances: HashMap<&'static str, eth::U256>,
    status: StatusCode,
    test: &'a Test,
    body: String,
}

pub struct SettleOk<'a> {
    test: &'a Test,
    old_balances: HashMap<&'static str, eth::U256>,
}

impl<'a> Settle<'a> {
    /// Expect the /settle endpoint to have returned a 200 OK response.
    pub async fn ok(self) -> SettleOk<'a> {
        // Ensure that the response is OK.
        assert_eq!(self.status, hyper::StatusCode::OK);
        let result: serde_json::Value = serde_json::from_str(&self.body).unwrap();
        assert!(result.is_object());
        assert_eq!(result.as_object().unwrap().len(), 1);
        assert!(!result
            .get("calldata")
            .unwrap()
            .get("internalized")
            .unwrap()
            .as_str()
            .unwrap()
            .is_empty());
        assert!(!result
            .get("calldata")
            .unwrap()
            .get("uninternalized")
            .unwrap()
            .as_str()
            .unwrap()
            .is_empty());

        // Ensure that the solution ID is included in the settlement.
        let tx = self
            .test
            .blockchain
            .web3
            .eth()
            .transaction(TransactionId::Block(
                BlockId::Number(ethcontract::BlockNumber::Latest),
                0.into(),
            ))
            .await
            .unwrap()
            .unwrap();
        let input = tx.input.0;
        let len = input.len();
        let tx_auction_id = u64::from_be_bytes((&input[len - 8..]).try_into().unwrap());
        assert_eq!(tx_auction_id.to_string(), "1");

        // Ensure that the internalized calldata returned by the driver is equal to the
        // calldata published to the blockchain.
        let internalized = result
            .get("calldata")
            .unwrap()
            .get("internalized")
            .unwrap()
            .as_str()
            .unwrap();
        assert_eq!(
            internalized,
            format!("0x{}", hex::encode(&input).to_lowercase())
        );

        SettleOk {
            test: self.test,
            old_balances: self.old_balances,
        }
    }
}

impl<'a> SettleOk<'a> {
    /// Check that the user balance changed.
    pub async fn balance(self, token: &'static str, balance: Balance) -> SettleOk<'a> {
        let new_balances = self.test.balances().await;
        let new_balance = new_balances.get(token).unwrap();
        let old_balance = self.old_balances.get(token).unwrap();
        match balance {
            Balance::Greater => assert!(new_balance > old_balance),
            Balance::SmallerBy(diff) => assert_eq!(*new_balance, old_balance - diff),
        }
        self
    }

    /// Ensure that the onchain balances changed in accordance with the
    /// [`ab_order`].
    pub async fn ab_order_executed(self) -> SettleOk<'a> {
        self.balance("A", Balance::SmallerBy(AB_ORDER_AMOUNT.into()))
            .await
            .balance("B", Balance::Greater)
            .await
    }

    /// Ensure that the onchain balances changed in accordance with the
    /// [`cd_order`].
    pub async fn cd_order_executed(self) -> SettleOk<'a> {
        self.balance("C", Balance::SmallerBy(CD_ORDER_AMOUNT.into()))
            .await
            .balance("D", Balance::Greater)
            .await
    }

    /// Ensure that the onchain balances changed in accordance with the
    /// [`eth_order`].
    pub async fn eth_order_executed(self) -> SettleOk<'a> {
        self.balance("A", Balance::SmallerBy(ETH_ORDER_AMOUNT.into()))
            .await
            .balance("ETH", Balance::Greater)
            .await
    }
}
