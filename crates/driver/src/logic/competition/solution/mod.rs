use {
    crate::{
        logic::{competition, eth},
        solver::{self, Solver},
        util,
        Ethereum,
        Simulator,
    },
    num::ToPrimitive,
    primitive_types::U256,
    std::collections::HashMap,
};

mod approvals;
pub mod settlement;

pub use {approvals::Approvals, settlement::Settlement};

/// A solution represents a set of orders which the solver has found an optimal
/// way to settle. A [`Solution`] is generated by a solver as a response to a
/// [`super::auction::Auction`].
#[derive(Debug)]
pub struct Solution {
    // TODO After #831 we hope to make this a vec and not a hashmap
    pub orders: HashMap<usize, Order>,
    pub jit_orders: Vec<JitOrder>,
    // TODO This will be changed in #831:
    // 1. It should not be a HashMap, it should be a list.
    // 2. It should be part of the orders field, as an enum
    // But I'll make it work like this for now
    pub amms: HashMap<eth::Address, Vec<Amm>>,
    pub prices: HashMap<eth::Token, eth::Ether>,
    pub approvals: Approvals,
    pub interactions: Vec<SafeInteraction>,
}

/// An interaction with inputs and outputs which can be used to ensure that the
/// solver isn't cheating.
#[derive(Debug)]
pub struct SafeInteraction {
    pub inner: eth::Interaction,
    pub inputs: Vec<eth::Asset>,
    pub outputs: Vec<eth::Asset>,
}

#[derive(Debug)]
pub struct Amm {
    pub sell: eth::Asset,
    pub buy: eth::Asset,
    pub sequence: u32,
    pub position: u32,
    pub internal: bool,
}

// TODO Why is this on JIT orders in the SOLUTION and also on all orders in the
// auction?
#[derive(Debug)]
pub enum SellSource {
    /// Direct ERC20 allowances to the Vault relayer contract
    Erc20,
    /// ERC20 allowances to the Vault with GPv2 relayer approval
    Internal,
    /// Internal balances to the Vault with GPv2 relayer approval
    External,
}

#[derive(Debug)]
pub enum BuyDestination {
    /// Direct ERC20 allowances to the Vault relayer contract
    Erc20,
    /// ERC20 allowances to the Vault with GPv2 relayer approval
    Internal,
}

/// An order which was executed as part of this solution.
#[derive(Debug)]
pub struct Order {
    pub sell: U256,
    pub buy: U256,
    // TODO Revisit after #831
    pub plan: Option<ExecutionPlan>,
    pub fee: Option<eth::Asset>,

    // TODO This dictates where tokens are moved from, I guess I should document this and talk to
    // Nick about whether we should change it
    pub sell_source: SellSource,
    pub buy_destination: BuyDestination,
}

/// A just-in-time order. JIT orders are added at solving time by the solver to
/// generate a more optimal solution for the auction.
#[derive(Debug)]
pub struct JitOrder {
    pub from: eth::Address,
    pub sell: eth::Asset,
    pub buy: eth::Asset,
    pub fee: eth::Ether,
    pub receiver: Option<eth::Address>,
    pub valid_to: util::Timestamp,
    pub app_data: competition::AppData,
    pub side: competition::Side,
    pub partial: bool,

    // TODO There needs to be some formal structure for "partial orders" and not just random fields
    // scattered around, think about this more
    pub executed_buy_amount: U256,
    pub executed_sell_amount: U256,

    // TODO I still don't know what this is
    pub sell_source: SellSource,
    pub buy_destination: BuyDestination,

    // TODO This is a placeholder
    pub signature: i32,
}

#[derive(Debug)]
pub struct ExecutionPlan {
    pub sequence: u32,
    pub position: u32,
    pub internal: bool,
}

/// The solution score. This is often referred to as the "objective value".
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct Score(num::BigRational);

impl From<Score> for f64 {
    fn from(score: Score) -> Self {
        score.0.to_f64().expect("value can be represented as f64")
    }
}

/// Solve an auction and return the [`Score`] of the solution.
pub async fn solve(
    solver: &Solver,
    eth: &Ethereum,
    simulator: &Simulator,
    auction: &competition::Auction,
) -> Result<Score, solver::Error> {
    let solution = solver.solve(auction).await?;
    let settlement = Settlement::encode(solver, eth, auction, solution).await?;
    Ok(settlement.score(simulator))
}

/// A unique solution ID. TODO Once this is finally decided, document what this
/// ID is used for.
#[derive(Debug, Clone, Copy)]
pub struct Id(pub u64);
