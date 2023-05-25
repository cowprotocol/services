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
            },
            setup::new::blockchain::Blockchain,
        },
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Order {
    pub name: &'static str,

    /// The amount being bought or sold by this order.
    pub amount: eth::U256,
    pub sell_token: &'static str,
    pub buy_token: &'static str,

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

    /// Set a value to be used to divide the order buy or sell amount before
    /// the order gets placed and thereby generate surplus. Whether the sell or
    /// buy amount is divided depends on the order side. This is necessary to
    /// keep the solution scores positive.
    pub surplus_factor: eth::U256,
}

impl Order {
    /// Rename the order.
    pub fn rename(self, name: &'static str) -> Self {
        Self { name, ..self }
    }

    /// Reduce the amount of this order by the given amount.
    pub fn reduce_amount(self, diff: eth::U256) -> Self {
        Self {
            amount: self.amount - diff,
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
}

impl Default for Order {
    fn default() -> Self {
        Self {
            amount: Default::default(),
            sell_token: Default::default(),
            buy_token: Default::default(),
            internalize: Default::default(),
            side: order::Side::Sell,
            partial: order::Partial::No,
            valid_for: 100.into(),
            kind: order::Kind::Market,
            executed: Default::default(),
            user_fee: Default::default(),
            solver_fee: Default::default(),
            name: Default::default(),
            surplus_factor: DEFAULT_SURPLUS_FACTOR.into(),
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
    solutions: Vec<Solution>,
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
    /// Divide the surplus factor by the specified amount.
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

/// An example order which sells token "A" for token "B".
pub fn ab_order() -> Order {
    Order {
        name: "A-B order",
        amount: AB_ORDER_AMOUNT.into(),
        sell_token: "A",
        buy_token: "B",
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

/// An example order which sells token "C" for token "D".
pub fn cd_order() -> Order {
    Order {
        name: "C-D order",
        amount: CD_ORDER_AMOUNT.into(),
        sell_token: "C",
        buy_token: "D",
        ..Default::default()
    }
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

    /// Create a pool of tokens "A" and "B".
    pub fn ab_pool(self) -> Self {
        self.pool(
            "A",
            DEFAULT_POOL_AMOUNT_A.into(),
            "B",
            DEFAULT_POOL_AMOUNT_B.into(),
        )
    }

    /// Create a pool of tokens "C" and "D".
    pub fn cd_pool(self) -> Self {
        self.pool(
            "C",
            DEFAULT_POOL_AMOUNT_C.into(),
            "D",
            DEFAULT_POOL_AMOUNT_D.into(),
        )
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
    pub fn solution(mut self, solution: Solution) -> Self {
        self.solutions.push(solution);
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
        let solver = Solver::new(
            &blockchain,
            &solutions,
            &trusted,
            &quotes,
            deadline,
            self.now,
        )
        .await;
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
            fulfillments: solutions.into_iter().flat_map(|s| s.fulfillments).collect(),
            trusted,
            deadline,
            now,
            quotes,
        }
    }

    fn deadline(&self) -> chrono::DateTime<chrono::Utc> {
        self.now.now() + chrono::Duration::days(30)
    }
}

pub struct Test {
    quotes: Vec<blockchain::Quote>,
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
        Solve {
            status,
            body,
            fulfillments: &self.fulfillments,
            blockchain: &self.blockchain,
            now: self.now,
        }
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
pub struct Solve<'a> {
    status: StatusCode,
    body: String,
    fulfillments: &'a [Fulfillment],
    blockchain: &'a Blockchain,
    now: infra::time::Now,
}

impl<'a> Solve<'a> {
    /// Expect the /solve endpoint to have returned a 200 OK response.
    pub fn ok(self) -> SolveOk<'a> {
        assert_eq!(self.status, hyper::StatusCode::OK);
        SolveOk {
            body: self.body,
            fulfillments: self.fulfillments,
            blockchain: self.blockchain,
            now: self.now,
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
    now: infra::time::Now,
}

impl SolveOk<'_> {
    /// Ensure that the score in the response is within a certain range. The
    /// reason why this is a range is because small timing differences in
    /// the test can lead to the settlement using slightly different amounts
    /// of gas, which in turn leads to different scores.
    pub fn score(self, min: eth::U256, max: eth::U256) -> Self {
        let result: serde_json::Value = serde_json::from_str(&self.body).unwrap();
        assert!(result.is_object());
        assert_eq!(result.as_object().unwrap().len(), 4);
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
        let expected_order_uids: HashSet<_> = order_names
            .iter()
            .map(|name| {
                self.fulfillments
                    .iter()
                    .find(|f| f.quote.order.name == *name)
                    .unwrap_or_else(|| {
                        panic!(
                            "unexpected orders {order_names:?}: fulfillment not found in {:?}",
                            self.fulfillments,
                        )
                    })
                    .quote
                    .order_uid(self.blockchain, self.now)
                    .to_string()
            })
            .collect();
        let result: serde_json::Value = serde_json::from_str(&self.body).unwrap();
        assert!(result.is_object());
        assert_eq!(result.as_object().unwrap().len(), 4);
        assert!(result.get("orders").is_some());
        let order_uids: HashSet<_> = result
            .get("orders")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|order| order.as_str().unwrap().to_owned())
            .collect();
        assert_eq!(order_uids, expected_order_uids);
        self
    }

    /// Get the solution ID from the response.
    pub fn solution_id(self) -> String {
        let result: serde_json::Value = serde_json::from_str(&self.body).unwrap();
        assert!(result.is_object());
        assert_eq!(result.as_object().unwrap().len(), 4);
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
    pub fn score(self, expected_score: &str) {
        let result: serde_json::Value = serde_json::from_str(&self.body).unwrap();
        assert!(result.is_object());
        assert_eq!(result.as_object().unwrap().len(), 2);
        assert!(result.get("score").is_some());
        let score = result.get("score").unwrap().as_str().unwrap();
        assert_eq!(score, expected_score);
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
}
