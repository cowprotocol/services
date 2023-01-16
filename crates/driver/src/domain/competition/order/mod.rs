use crate::{domain::eth, util};

pub mod signature;

pub use signature::Signature;

use crate::infra::{blockchain, Ethereum};

/// Address used in place of an actual buy token address in an order which buys
/// ETH.
const BUY_ETH_ADDRESS: eth::TokenAddress =
    eth::TokenAddress(eth::ContractAddress(eth::H160([0xee; 20])));

/// An order in the auction.
#[derive(Debug, Clone)]
pub struct Order {
    pub uid: Uid,
    /// The user specified a custom address to receive the output of this order.
    pub receiver: Option<eth::Address>,
    pub valid_to: util::Timestamp,
    pub sell: eth::Asset,
    pub buy: eth::Asset,
    pub side: Side,
    pub fee: Fee,
    pub kind: Kind,
    pub app_data: AppData,
    pub partial: Partial,
    /// The onchain calls necessary to fulfill this order. These are set by the
    /// user and included in the settlement transaction.
    pub interactions: Vec<eth::Interaction>,
    pub sell_token_balance: SellTokenBalance,
    pub buy_token_balance: BuyTokenBalance,
    pub signature: Signature,
    /// The reward that will be received by the solver denominated in CoW
    /// tokens.
    pub reward: f64,
}

/// An amount denominated in the sell token of an [`Order`].
#[derive(Debug, Default, Clone, Copy)]
pub struct SellAmount(eth::U256);

impl From<eth::U256> for SellAmount {
    fn from(value: eth::U256) -> Self {
        Self(value)
    }
}

impl From<SellAmount> for eth::U256 {
    fn from(sell_amount: SellAmount) -> Self {
        sell_amount.0
    }
}

impl SellAmount {
    pub fn to_asset(self, order: &Order) -> eth::Asset {
        eth::Asset {
            amount: self.0,
            token: order.sell.token,
        }
    }
}

/// An amount denominated in the sell token for [`Side::Sell`] [`Order`]s, or in
/// the buy token for [`Side::Buy`] [`Order`]s.
#[derive(Debug, Default, Clone, Copy)]
pub struct TargetAmount(eth::U256);

impl From<eth::U256> for TargetAmount {
    fn from(value: eth::U256) -> Self {
        Self(value)
    }
}

impl From<TargetAmount> for eth::U256 {
    fn from(value: TargetAmount) -> Self {
        value.0
    }
}

impl TargetAmount {
    pub fn to_asset(self, order: &Order) -> eth::Asset {
        eth::Asset {
            amount: self.0,
            token: match order.side {
                Side::Buy => order.buy.token,
                Side::Sell => order.sell.token,
            },
        }
    }
}

/// Order fee denominated in the sell token.
#[derive(Debug, Default, Clone)]
pub struct Fee {
    /// The order fee that is actually paid by the user.
    pub user: SellAmount,
    /// The fee used for scoring. This is a scaled version of the user fee to
    /// incentivize solvers to solve orders in batches.
    pub solver: SellAmount,
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
        self.receiver.unwrap_or(self.signature.signer)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Partial {
    /// A partially order doesn't require the full amount to be traded.
    /// E.g. only 10% of the requested amount may be traded, if this leads
    /// to the most optimal solution.
    Yes {
        /// The already-executed amount for the partial order.
        executed: TargetAmount,
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

impl Default for Uid {
    fn default() -> Self {
        Self([0; 56])
    }
}

impl PartialEq<[u8; 56]> for Uid {
    fn eq(&self, other: &[u8; 56]) -> bool {
        self.0 == *other
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// Buy an exact amount.
    Buy,
    /// Sell an exact amount.
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
#[derive(Debug, Default, Clone, Copy)]
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

#[derive(Debug, Clone, Copy)]
pub enum Kind {
    /// Order intended to be immediately executed. This is the "regular" type of
    /// order.
    Market,
    /// Order intended to be executed possibly far into the future, when the
    /// price is such that the order can be executed.
    Limit {
        /// The fee to be taken from the order surplus.
        surplus_fee: SellAmount,
    },
    /// An order submitted by a privileged user, which provides liquidity for
    /// our settlement contract.
    Liquidity,
}

/// [Balancer V2](https://docs.balancer.fi/) integration, used for settlement encoding.
#[derive(Debug, Clone, Copy)]
pub enum SellTokenBalance {
    Erc20,
    Internal,
    External,
}

/// [Balancer V2](https://docs.balancer.fi/) integration, used for settlement encoding.
#[derive(Debug, Clone, Copy)]
pub enum BuyTokenBalance {
    Erc20,
    Internal,
}

/// A just-in-time order. JIT orders are added at solving time by the solver to
/// generate a more optimal solution for the auction. Very similar to a regular
/// [`Order`].
#[derive(Debug)]
pub struct Jit {
    pub sell: eth::Asset,
    pub buy: eth::Asset,
    pub fee: SellAmount,
    pub receiver: eth::Address,
    pub valid_to: util::Timestamp,
    pub app_data: AppData,
    pub side: Side,
    pub partially_fillable: bool,
    pub sell_token_balance: SellTokenBalance,
    pub buy_token_balance: BuyTokenBalance,
    pub signature: Signature,
}
