use {
    crate::{
        logic::{competition, eth},
        util,
    },
    primitive_types::U256,
};

/// An order in the auction.
#[derive(Debug)]
pub struct Order {
    pub uid: competition::OrderUid,
    pub from: eth::Address,
    pub receiver: Option<eth::Address>,
    pub valid_to: util::Timestamp,
    pub sell: eth::Asset,
    pub buy: eth::Asset,
    pub side: competition::Side,
    pub fee: Fee,
    pub kind: Kind,
    pub app_data: competition::AppData,
    /// A partial order doesn't require the full amount to be traded. E.g. only
    /// 10% of the requested amount may be traded, if this leads to the most
    /// optimal solution.
    pub partial: bool,
    /// The autopilot marks orders as mature after a certain time period. The
    /// solvers can use heuristics on this field to optimize solution sizes.
    pub mature: bool,
    pub executed: U256,
    /// The onchain calls necessary to fulfill this order. These are set by the
    /// user and included in the settlement transaction.
    pub interactions: Vec<eth::Interaction>,

    pub sell_source: SellSource,
    pub buy_destination: BuyDestination,

    // TODO This is a placeholder, there should probably be a proper submodule for signing stuff
    pub signature: i32,
}

#[derive(Debug)]
pub enum Kind {
    Market,
    Limit { surplus_fee: eth::Ether },
    Liquidity,
}

impl Kind {
    pub fn is_liquidity(&self) -> bool {
        matches!(self, Self::Liquidity)
    }
}

/// Order fee.
#[derive(Debug)]
pub struct Fee {
    /// The order fee that is actually paid by the user.
    pub user: eth::Ether,
    /// The fee used for scoring. The user fee is scaled by the autopilot to
    /// promote batching during auction solving.
    pub solver: eth::Ether,
}

// TODO Ask about this
#[derive(Debug)]
pub enum SellSource {
    /// Direct ERC20 allowances to the Vault relayer contract
    Erc20,
    /// ERC20 allowances to the Vault with GPv2 relayer approval
    Internal,
    /// Internal balances to the Vault with GPv2 relayer approval
    External,
}

// TODO Ask about this
#[derive(Debug)]
pub enum BuyDestination {
    /// Direct ERC20 allowances to the Vault relayer contract
    Erc20,
    /// ERC20 allowances to the Vault with GPv2 relayer approval
    Internal,
}
