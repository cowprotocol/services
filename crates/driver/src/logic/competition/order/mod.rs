use {
    crate::{logic::eth, util},
    primitive_types::H160,
};

pub mod signature;

pub use signature::Signature;

use crate::{blockchain, Ethereum};

/// Address used in place of an actual buy token address in an order which buys
/// ETH.
const BUY_ETH_ADDRESS: eth::TokenAddress = eth::TokenAddress(H160([0xee; 20]));

/// An order in the auction.
#[derive(Debug)]
pub struct Order {
    pub uid: Uid,
    /// The user specified a custom address to receive the output of this order.
    pub receiver: Option<eth::Address>,
    /// The address used to place this order.
    pub owner: eth::Address,
    pub valid_to: util::Timestamp,
    pub sell: eth::Asset,
    pub buy: eth::Asset,
    pub side: Side,
    pub fee: Fee,
    pub kind: Kind,
    pub app_data: AppData,
    pub partial: Partial,
    /// The autopilot marks orders as mature after a certain time period. The
    /// solvers can use heuristics on this field to optimize solution sizes.
    pub mature: bool,
    /// The onchain calls necessary to fulfill this order. These are set by the
    /// user and included in the settlement transaction.
    pub interactions: Vec<eth::Interaction>,
    pub sell_source: SellSource,
    pub buy_destination: BuyDestination,
    pub signature: Signature,
}

/// Order fee denominated in the sell token.
// TODO The token should be validated, probably use newtypes, e.g. UserFee and SolverFee
#[derive(Debug)]
pub struct Fee {
    /// The order fee that is actually paid by the user.
    pub user: eth::Asset,
    /// The fee used for scoring. This is a scaled version of the user fee to
    /// incentivize solvers to solve orders in batches.
    pub solver: eth::Asset,
}

impl Order {
    pub fn is_partial(&self) -> bool {
        matches!(self.partial, Partial::Yes { .. })
    }

    /// Does this order pay to a smart contract?
    pub async fn pays_to_contract(&self, eth: &Ethereum) -> Result<bool, blockchain::Error> {
        eth.is_contract(self.receiver()).await
    }

    /// Does this order buy ETH?
    pub fn buys_eth(&self) -> bool {
        self.buy.token == BUY_ETH_ADDRESS
    }

    /// The address which will receive the output of this order. If a custom
    /// receiver address was specified by the user explicitly, return that
    /// address. Otherwise, return the address which was used to place the
    /// order.
    pub fn receiver(&self) -> eth::Address {
        self.receiver.unwrap_or(self.owner)
    }
}

#[derive(Debug)]
pub enum Partial {
    /// A partially order doesn't require the full amount to be traded.
    /// E.g. only 10% of the requested amount may be traded, if this leads
    /// to the most optimal solution.
    Yes {
        /// The already-executed amount for the partial order. For sell
        /// orders this will be denominated in the sell token, for buy
        /// orders in the buy token.
        executed: eth::Asset,
    },
    No,
}

impl Order {
    pub fn is_liquidity(&self) -> bool {
        matches!(self.kind, Kind::Liquidity)
    }
}

/// UID of an order.
#[derive(Debug, Clone, Copy)]
pub struct Uid(pub [u8; 56]);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
}

impl From<[u8; 56]> for Uid {
    fn from(inner: [u8; 56]) -> Self {
        Self(inner)
    }
}

impl From<Uid> for [u8; 56] {
    fn from(uid: Uid) -> Self {
        uid.0
    }
}

/// This is a hash allowing arbitrary user data to be associated with an order.
/// While this type holds the hash, the data itself is uploaded to IPFS. This
/// hash is signed along with the order.
#[derive(Debug, Clone, Copy)]
pub struct AppData(pub [u8; 32]);

impl From<[u8; 32]> for AppData {
    fn from(inner: [u8; 32]) -> Self {
        Self(inner)
    }
}

impl From<AppData> for [u8; 32] {
    fn from(app_data: AppData) -> Self {
        app_data.0
    }
}

#[derive(Debug)]
pub enum Kind {
    /// Order intended to be immediately executed. This is the "regular" type of
    /// order.
    Market,
    /// Order intended to be executed possibly far into the future, when the
    /// price is such that the order can be executed.
    Limit {
        /// The fee to be taken from the order surplus. This is denominated in
        /// the sell token of the order.
        // TODO The token should be validated, possibly with a newtype
        surplus_fee: eth::Asset,
    },
    /// An order submitted by a privileged user, which provides liquidity for
    /// our settlement contract.
    Liquidity,
}

// TODO Ask about this
#[derive(Debug)]
pub enum SellSource {
    Erc20,
    Internal,
    External,
}

// TODO Ask about this
#[derive(Debug)]
pub enum BuyDestination {
    Erc20,
    Internal,
}

/// A just-in-time order. JIT orders are added at solving time by the solver to
/// generate a more optimal solution for the auction. Very similar to a regular
/// [`Order`].
#[derive(Debug)]
pub struct Jit {
    pub from: eth::Address,
    pub sell: eth::Asset,
    pub buy: eth::Asset,
    ///  Fee denominated in the sell token.
    // TODO The fee token should be validated, possibly with a newtype
    pub fee: eth::Asset,
    pub receiver: Option<eth::Address>,
    pub valid_to: util::Timestamp,
    pub app_data: AppData,
    pub side: Side,
    pub partially_fillable: bool,
    pub sell_source: SellSource,
    pub buy_destination: BuyDestination,
    pub signature: Signature,
}
