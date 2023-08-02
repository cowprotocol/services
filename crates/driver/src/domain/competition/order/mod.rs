use crate::{
    domain::eth,
    infra::{blockchain, Ethereum},
    util::{self, conv, Bytes},
};

pub mod signature;

pub use signature::Signature;
use {super::auction, bigdecimal::Zero, num::CheckedDiv};

/// An order in the auction.
#[derive(Debug, Clone)]
pub struct Order {
    pub uid: Uid,
    /// The user specified a custom address to receive the output of this order.
    pub receiver: Option<eth::Address>,
    pub valid_to: util::Timestamp,
    /// The minimum amount this order must buy when completely filled.
    pub buy: eth::Asset,
    /// The maximum amount this order is allowed to sell when completely filled.
    pub sell: eth::Asset,
    pub side: Side,
    pub fee: Fee,
    pub kind: Kind,
    pub app_data: AppData,
    pub partial: Partial,
    /// The onchain calls to run before sending user funds to the settlement
    /// contract.
    /// These are set by the user and included in the settlement transaction.
    pub pre_interactions: Vec<eth::Interaction>,
    /// The onchain calls to run after sending tokens from the settlement
    /// contract to the user.
    /// These are set by the user and included in the settlement transaction.
    pub post_interactions: Vec<eth::Interaction>,
    pub sell_token_balance: SellTokenBalance,
    pub buy_token_balance: BuyTokenBalance,
    pub signature: Signature,
}

/// An amount denominated in the sell token of an [`Order`].
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SellAmount(pub eth::U256);

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

/// An amount denominated in the sell token for [`Side::Sell`] [`Order`]s, or in
/// the buy token for [`Side::Buy`] [`Order`]s.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TargetAmount(pub eth::U256);

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

impl From<eth::TokenAmount> for TargetAmount {
    fn from(value: eth::TokenAmount) -> Self {
        Self(value.0)
    }
}

impl From<TargetAmount> for eth::TokenAmount {
    fn from(value: TargetAmount) -> Self {
        Self(value.0)
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
    /// The buy amount for [`Side::Buy`] orders, or the sell amount for
    /// [`Side::Sell`] orders.
    pub fn target(&self) -> TargetAmount {
        match self.side {
            Side::Buy => self.buy.amount.into(),
            Side::Sell => self.sell.amount.into(),
        }
    }

    pub fn is_partial(&self) -> bool {
        matches!(self.partial, Partial::Yes { .. })
    }

    /// Does this order pay to a smart contract?
    pub async fn pays_to_contract(&self, eth: &Ethereum) -> Result<bool, blockchain::Error> {
        eth.is_contract(self.receiver()).await
    }

    /// Does this order buy ETH?
    pub fn buys_eth(&self) -> bool {
        self.buy.token == eth::ETH_TOKEN
    }

    /// The address which will receive the output of this order. If a custom
    /// receiver address was specified by the user explicitly, return that
    /// address. Otherwise, return the address which was used to place the
    /// order.
    pub fn receiver(&self) -> eth::Address {
        self.receiver.unwrap_or(self.signature.signer)
    }

    pub fn is_liquidity(&self) -> bool {
        matches!(self.kind, Kind::Liquidity)
    }

    /// The buy asset to pass to the solver. This is a special case due to
    /// orders which buy ETH. The settlement contract only works with ERC20
    /// tokens, but unfortunately ETH is not an ERC20 token. We still want to
    /// provide a seamless user experience for ETH trades, so the driver
    /// will encode the settlement to automatically unwrap the WETH into ETH
    /// after the trade is done.
    ///
    /// For this reason, we want the solvers to solve the orders which buy ETH
    /// as if they were buying WETH, and then add our unwrap interaction to that
    /// solution.
    pub fn solver_buy(&self, weth: eth::WethAddress) -> eth::Asset {
        eth::Asset {
            amount: self.buy.amount,
            token: self.buy.token.wrap(weth),
        }
    }

    /// Should the order fee be determined by the solver? This is true for
    /// partial limit orders.
    pub fn solver_determines_fee(&self) -> bool {
        matches!(self.kind, Kind::Limit { .. })
    }

    /// The likelihood that this order will be fulfilled, based on token prices.
    /// A larger value means that the order is more likely to be fulfilled.
    /// This is used to prioritize orders when solving.
    pub fn likelihood(&self, tokens: &auction::Tokens) -> num::BigRational {
        match (
            tokens.get(self.buy.token).price,
            tokens.get(self.sell.token).price,
        ) {
            (Some(buy_price), Some(sell_price)) => {
                let buy = buy_price.apply(self.buy.amount);
                let sell = sell_price.apply(self.sell.amount);
                conv::u256::to_big_rational(buy.0)
                    .checked_div(&conv::u256::to_big_rational(sell.0))
                    .unwrap_or_else(num::BigRational::zero)
            }
            _ => num::BigRational::zero(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// The length of an order UID in bytes.
pub const UID_LEN: usize = 56;

/// UID of an order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Uid(pub Bytes<[u8; UID_LEN]>);

impl Default for Uid {
    fn default() -> Self {
        Self([0; UID_LEN].into())
    }
}

impl PartialEq<[u8; UID_LEN]> for Uid {
    fn eq(&self, other: &[u8; UID_LEN]) -> bool {
        self.0 .0 == *other
    }
}

// TODO These doc comments are incorrect for limit orders
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// Buy an exact amount. The sell amount can vary due to e.g. partial fills
    /// or slippage.
    Buy,
    /// Sell an exact amount. The buy amount can vary due to e.g. partial fills
    /// or slippage.
    Sell,
}

impl From<[u8; UID_LEN]> for Uid {
    fn from(inner: [u8; UID_LEN]) -> Self {
        Self(inner.into())
    }
}

impl From<Uid> for [u8; UID_LEN] {
    fn from(uid: Uid) -> Self {
        uid.0.into()
    }
}

/// The length of the app data hash in bytes.
pub const APP_DATA_LEN: usize = 32;

/// This is a hash allowing arbitrary user data to be associated with an order.
/// While this type holds the hash, the data itself is uploaded to IPFS. This
/// hash is signed along with the order.
#[derive(Debug, Default, Clone, Copy)]
pub struct AppData(pub Bytes<[u8; APP_DATA_LEN]>);

impl From<[u8; APP_DATA_LEN]> for AppData {
    fn from(inner: [u8; APP_DATA_LEN]) -> Self {
        Self(inner.into())
    }
}

impl From<AppData> for [u8; APP_DATA_LEN] {
    fn from(app_data: AppData) -> Self {
        app_data.0.into()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Order intended to be immediately executed. This is the "regular" type of
    /// order.
    Market,
    /// Order intended to be fulfilled possibly far into the future, when the
    /// price is such that the order can be executed. Because the fulfillment
    /// can happen any time into the future, it's impossible to calculate
    /// the order fees ahead of time, so the fees are taken from the order
    /// surplus instead. (The order surplus is the additional money that the
    /// solver managed to solve for, above what the user specified in the
    /// order.)
    Limit {
        /// The fee to be taken from the order surplus. The surplus is always
        /// taken from the sell amount.
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
#[derive(Debug, Clone)]
pub struct Jit {
    /// The amount this order wants to sell when completely filled.
    /// The actual executed amount depends on partial fills and the order side.
    pub sell: eth::Asset,
    /// The amount this order wants to buy when completely filled.
    /// The actual executed amount depends on partial fills and the order side.
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

impl Jit {
    /// The buy amount for [`Side::Buy`] orders, or the sell amount for
    /// [`Side::Sell`] orders.
    pub fn target(&self) -> TargetAmount {
        match self.side {
            Side::Buy => self.buy.amount.into(),
            Side::Sell => self.sell.amount.into(),
        }
    }
}
