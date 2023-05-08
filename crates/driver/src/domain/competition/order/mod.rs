use crate::{
    domain::eth,
    infra::{blockchain, Ethereum},
    util,
};

pub mod signature;

pub use signature::Signature;

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
    /// The onchain calls necessary to fulfill this order. These are set by the
    /// user and included in the settlement transaction.
    pub interactions: Vec<eth::Interaction>,
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

    /// The sell asset to pass to the solver. This is a special case due to
    /// limit orders. For limit orders, the interaction produced by the
    /// solver needs to leave the surplus fee inside the settlement
    /// contract, since that's the fee taken by the protocol. For that
    /// reason, the solver solves for the sell amount reduced by the surplus
    /// fee; then, the settlement transaction will move the sell amount from
    /// the order *into* the settlement contract, while the interaction
    /// produced by the solver will move (sell amount - surplus fee) *out*
    /// of the settlement contract and into the AMMs, hence leaving the
    /// surplus fee inside the contract.
    pub fn solver_sell(&self) -> eth::Asset {
        if let Kind::Limit { surplus_fee } = self.kind {
            eth::Asset {
                amount: self.sell.amount - surplus_fee.0,
                token: self.sell.token,
            }
        } else {
            self.sell
        }
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

    /// For some orders the protocol doesn't precompute a fee. Instead solvers
    /// are supposed to compute a reasonable fee themselves.
    pub fn solver_determines_fee(&self) -> bool {
        self.is_partial() && matches!(self.kind, Kind::Limit { .. })
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
