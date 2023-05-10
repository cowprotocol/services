//! Improved test framework. Currently does not implement quoting, but this is
//! about the last thing I expect to have missed. Once that is implemented, the
//! dead_code macros should be removed and this `new` module should become the
//! super module.

use {
    self::{
        blockchain::{Fulfillment, Pool},
        driver::Driver,
        solver::Solver,
    },
    crate::{
        domain::{competition::order, eth},
        infra,
        tests::setup::new::blockchain::Blockchain,
        util,
    },
    ethcontract::BlockId,
    hyper::StatusCode,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Diff {
    Add(eth::U256),
    Sub(eth::U256),
}

impl Diff {
    fn apply(&self, amount: eth::U256) -> eth::U256 {
        match self {
            Self::Add(diff) => amount + diff,
            Self::Sub(diff) => amount - diff,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Order {
    pub name: &'static str,

    /// The amount being bought or sold by this order.
    pub amount: eth::U256,
    pub sell_token: &'static str,
    pub buy_token: &'static str,

    /// The difference between the order amount and the amount passed to the
    /// solver.
    pub solver_sell_diff: Diff,
    pub solver_buy_diff: Diff,

    pub internalize: bool,
    pub side: order::Side,
    pub partial: order::Partial,
    pub valid_for: util::Timestamp,
    pub kind: order::Kind,
    pub executed: eth::U256,

    // TODO For now I'll always set these to zero. But I think they should be tested as well.
    // Figure out what (if anything) would constitute meaningful tests for these values.
    pub user_fee: eth::U256,
    pub solver_fee: eth::U256,
}

impl Default for Order {
    fn default() -> Self {
        Self {
            amount: Default::default(),
            sell_token: Default::default(),
            buy_token: Default::default(),
            solver_sell_diff: Diff::Add(Default::default()),
            solver_buy_diff: Diff::Add(Default::default()),
            internalize: Default::default(),
            side: order::Side::Sell,
            partial: order::Partial::No,
            valid_for: 100.into(),
            kind: order::Kind::Market,
            executed: Default::default(),
            user_fee: Default::default(),
            solver_fee: Default::default(),
            name: Default::default(),
        }
    }
}

/// Create a builder for the setup process.
pub fn setup() -> Setup {
    Setup {
        pools: Default::default(),
        orders: Default::default(),
        trusted: Default::default(),
        internalize: Default::default(),
        now: infra::time::Now::Fake(chrono::Utc::now()),
        config_file: Default::default(),
        solutions: Default::default(),
    }
}

#[derive(Debug)]
pub struct Setup {
    pools: Vec<Pool>,
    orders: Vec<Order>,
    trusted: HashSet<&'static str>,
    internalize: bool,
    now: infra::time::Now,
    config_file: Option<PathBuf>,
    solutions: Vec<SolutionSetup>,
}

#[derive(Debug, Clone, Copy)]
pub enum Solution {
    /// Set up the solver to return a valid solution.
    Valid,
    /// Set up the solver to return a valid solution, with additional
    /// meaningless bytes appended to the calldata. This is useful for
    /// lowering the solution score in a controlled way.
    LowerScore { additional_calldata: usize },
    /// Set up the solver to return a solution with bogus calldata.
    InvalidCalldata,
}

#[derive(Debug)]
struct SolutionSetup {
    solution: Solution,
    order_names: Vec<&'static str>,
}

impl Setup {
    /// Add a uniswap pool with the specified reserves. Tokens are identified
    /// by their symbols. Every order will be solved through one of the pools.
    pub fn pool(
        mut self,
        token_a: &'static str,
        token_a_reserve: eth::U256,
        token_b: &'static str,
        token_b_reserve: eth::U256,
    ) -> Self {
        self.pools.push(Pool {
            reserve_a: Asset {
                token: token_a,
                amount: token_a_reserve,
            },
            reserve_b: Asset {
                token: token_b,
                amount: token_b_reserve,
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

    // TODO Idea for this: instead of just storing the orders, also store the
    // operation which is expected on each order. This way, there could be tests
    // which also exercise a mix of solving/quoting. Alternatively if you want
    // to do something simple, just store a single order and explicitly allow
    // only one order to be added if quote() was called.
    /// Add a new order to be quoted as part of the test. This order will be
    /// passed to /quote when [`Test::quote`] is called and it will be
    /// anticipated by the mock solver.
    pub fn quote(self, _order: Order) -> Self {
        todo!("not implemented yet")
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
    pub fn solution(mut self, solution: Solution, orders: &[&'static str]) -> Self {
        self.solutions.push(SolutionSetup {
            solution,
            order_names: orders.to_owned(),
        });
        self
    }

    /// Create the test: set up onchain contracts and pools, start a mock HTTP
    /// server for the solver and start the HTTP server for the driver.
    pub async fn done(self) -> Test {
        crate::boundary::initialize_tracing("driver=trace");

        let deadline = self.deadline();
        let Self {
            pools,
            orders,
            trusted,
            now,
            config_file,
            ..
        } = self;

        // Hardcoded trader account.
        let trader_address =
            eth::H160::from_str("d2525C68A663295BBE347B65C87c8e17De936a0a").unwrap();
        let trader_secret_key = SecretKey::from_slice(
            &hex::decode("f9f831cee763ef826b8d45557f0f8677b27045e0e011bcd78571a40acc8a6cc3")
                .unwrap(),
        )
        .unwrap();

        // Hardcoded solver account.
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
        })
        .await;
        let mut solutions = Vec::new();
        for SolutionSetup {
            solution,
            order_names,
        } in self.solutions
        {
            let orders = order_names
                .iter()
                .map(|name| orders.iter().find(|o| o.name == *name).unwrap());
            solutions.push(blockchain.fulfill(orders, solution).await);
        }
        let solver = Solver::new(&blockchain, &solutions, &trusted, deadline, self.now).await;
        let driver = Driver::new(
            &driver::Config {
                config_file,
                relative_slippage,
                absolute_slippage,
                solver_address,
                solver_secret_key,
                now: self.now,
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
            fulfillments: solutions.into_iter().flatten().collect(),
            trusted,
            deadline,
            now,
        }
    }

    fn deadline(&self) -> chrono::DateTime<chrono::Utc> {
        self.now.now() + chrono::Duration::days(30)
    }
}

pub struct Test {
    blockchain: Blockchain,
    driver: Driver,
    client: reqwest::Client,
    trader_address: eth::H160,
    fulfillments: Vec<Fulfillment>,
    trusted: HashSet<&'static str>,
    deadline: chrono::DateTime<chrono::Utc>,
    now: infra::time::Now,
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
        Solve { status, body }
    }

    /// Call the /quote endpoint.
    pub async fn quote(&self) -> Quote {
        todo!("not implemented yet")
    }

    /// Call the /settle endpoint.
    pub async fn settle(&self, solution_id: String) -> Settle {
        let old_balances = self.balances().await;
        let res = blockchain::wait_for(
            &self.blockchain.web3,
            self.client
                .post(format!(
                    "http://{}/{}/settle/{solution_id}",
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
            solution_id,
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
pub struct Solve {
    status: StatusCode,
    body: String,
}

impl Solve {
    /// Expect the /solve endpoint to have returned a 200 OK response.
    pub fn ok(self) -> SolveOk {
        assert_eq!(self.status, hyper::StatusCode::OK);
        SolveOk { body: self.body }
    }

    /// Expect the /solve endpoint to return a 400 BAD REQUEST response.
    pub fn err(self) -> SolveErr {
        assert_eq!(self.status, hyper::StatusCode::BAD_REQUEST);
        SolveErr { body: self.body }
    }
}

pub struct SolveOk {
    body: String,
}

impl SolveOk {
    /// Check the score in the response against the expected value.
    pub fn score(self, expected_score: f64) {
        let result: serde_json::Value = serde_json::from_str(&self.body).unwrap();
        assert!(result.is_object());
        assert_eq!(result.as_object().unwrap().len(), 2);
        assert!(result.get("score").is_some());
        let score = result.get("score").unwrap().as_f64().unwrap();
        approx::assert_relative_eq!(score, expected_score, max_relative = 0.01);
    }

    /// Get the solution ID from the response.
    pub fn solution_id(self) -> String {
        let result: serde_json::Value = serde_json::from_str(&self.body).unwrap();
        assert!(result.is_object());
        assert_eq!(result.as_object().unwrap().len(), 2);
        assert!(result.get("id").is_some());
        result.get("id").unwrap().as_str().unwrap().to_owned()
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
pub struct Quote {
    status: StatusCode,
    body: String,
}

impl Quote {
    /// Expect the /quote endpoint to have returned a 200 OK response.
    pub fn ok(self) -> QuoteOk {
        assert_eq!(self.status, hyper::StatusCode::OK);
        QuoteOk { body: self.body }
    }
}

pub struct QuoteOk {
    body: String,
}

impl QuoteOk {
    /// Get the solution ID from the response.
    pub fn solution_id(self) -> String {
        let result: serde_json::Value = serde_json::from_str(&self.body).unwrap();
        assert!(result.is_object());
        assert_eq!(result.as_object().unwrap().len(), 2);
        assert!(result.get("id").is_some());
        result.get("id").unwrap().as_str().unwrap().to_owned()
    }

    /// Check the score in the response against the expected value.
    pub fn score(self, expected_score: f64) {
        let result: serde_json::Value = serde_json::from_str(&self.body).unwrap();
        assert!(result.is_object());
        assert_eq!(result.as_object().unwrap().len(), 2);
        assert!(result.get("score").is_some());
        let score = result.get("score").unwrap().as_f64().unwrap();
        approx::assert_relative_eq!(score, expected_score, max_relative = 0.01);
    }
}

/// The expected difference between a previous user balance for a certain token
/// and the balance after the settlement has been broadcast.
#[derive(Debug, Clone, Copy)]
pub enum Balance {
    /// The balance should be greater than before.
    Greater,
    /// The balance should be smaller than before.
    Smaller,
    /// The balance should be greater than before by an exact amount.
    GreaterBy(eth::U256),
    /// The balance should be smaller than before by an exact amount.
    SmallerBy(eth::U256),
    /// The balance should remain the same.
    Same,
}

/// A /settle response.
pub struct Settle<'a> {
    old_balances: HashMap<&'static str, eth::U256>,
    status: StatusCode,
    test: &'a Test,
    solution_id: String,
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
        let tx_solution_id = u64::from_be_bytes((&input[len - 8..]).try_into().unwrap());
        assert_eq!(tx_solution_id.to_string(), self.solution_id);

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
            Balance::GreaterBy(diff) => assert_eq!(*new_balance, old_balance + diff),
            Balance::Smaller => assert!(new_balance < old_balance),
            Balance::SmallerBy(diff) => assert_eq!(*new_balance, old_balance - diff),
            Balance::Same => assert_eq!(new_balance, old_balance),
        }
        self
    }
}
