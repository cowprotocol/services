use {
    crate::{
        domain::eth,
        infra::{Ethereum, blockchain},
        util,
    },
    alloy::primitives::FixedBytes,
    derive_more::{From, Into},
    model::order::{BuyTokenDestination, SellTokenSource},
};
pub use {fees::FeePolicy, signature::Signature};

pub mod app_data;
pub mod fees;
pub mod signature;

/// An order in the auction.
#[derive(Debug, Clone)]
pub struct Order {
    pub uid: Uid,
    /// The user specified a custom address to receive the output of this order.
    pub receiver: Option<eth::Address>,
    pub created: util::Timestamp,
    pub valid_to: util::Timestamp,
    /// The minimum amount this order must buy when completely filled.
    pub buy: eth::Asset,
    /// The maximum amount this order is allowed to sell when completely filled.
    pub sell: eth::Asset,
    pub side: Side,
    pub kind: Kind,
    pub app_data: app_data::AppData,
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
    /// The types of fees the protocol collects from the winning solver.
    /// Unless otherwise configured, the driver modifies solutions to take
    /// sufficient fee in the form of positive slippage.
    pub protocol_fees: Vec<FeePolicy>,
    /// The winning quote.
    pub quote: Option<Quote>,
}

/// An amount denominated in the sell token of an [`Order`].
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd, From, Into)]
pub struct SellAmount(pub eth::U256);

impl From<eth::TokenAmount> for SellAmount {
    fn from(value: eth::TokenAmount) -> Self {
        Self(value.into())
    }
}

/// An amount denominated in the sell token for [`Side::Sell`] [`Order`]s, or in
/// the buy token for [`Side::Buy`] [`Order`]s.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, From, Into)]
pub struct TargetAmount(pub eth::U256);

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

/// The available amounts for a specific order that gets passed to the solver.
///
/// These amounts differ from the order buy/sell/fee amounts in two ways:
/// 1. Partially fillable orders: they get pre-scaled before being passed to the
///    solver engine in order to simplify computation on their end. This uses
///    the order's `available` amount for scaling and considers both previously
///    executed amounts as well as remaining balances.
/// 1. Orders which buy ETH: The settlement contract only works with ERC20
///    tokens, but unfortunately ETH is not an ERC20 token. We still want to
///    provide a seamless user experience for ETH trades, so the driver will
///    encode the settlement to automatically unwrap the WETH into ETH after the
///    trade is done. For this reason, we want the solvers to solve the orders
///    which buy ETH as if they were buying WETH, and then add our unwrap
///    interaction to that solution.
pub struct Available {
    /// The available sell maximum amount for an order that gets passed to a
    /// solver engine.
    pub sell: eth::Asset,
    /// The available minimum buy amount for an order that gets passed to a
    /// solver engine.
    pub buy: eth::Asset,
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

    pub fn trader(&self) -> Trader {
        Trader(self.signature.signer)
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

    /// Returns the order's available amounts to be passed to a solver engine.
    ///
    /// See [`Available`] for more details.
    pub fn available(&self) -> Available {
        let mut amounts = Available {
            sell: self.sell,
            buy: self.buy,
        };

        let available = match self.partial {
            Partial::Yes { available } => available,
            Partial::No => return amounts,
        };
        let target = self.target();

        amounts.sell.amount = util::math::mul_ratio(amounts.sell.amount.0, available.0, target.0)
            .unwrap_or_default()
            .into();

        amounts.buy.amount =
            util::math::mul_ratio_ceil(amounts.buy.amount.0, available.0, target.0)
                .unwrap_or_default()
                .into();

        amounts
    }

    /// Should the order fee be determined by the solver? This is true for
    /// partial limit orders.
    pub fn solver_determines_fee(&self) -> bool {
        matches!(self.kind, Kind::Limit)
    }
}

impl Available {
    /// Returns `true` if any of the available orders amounts are `0`, thus
    /// making the order not suitable to send to solvers.
    ///
    /// TODO: It would be ideal to prohibit the construction of orders with bad
    /// available amounts (`0` or larger than the order) to prevent bugs.
    pub fn is_zero(&self) -> bool {
        self.sell.amount.0.is_zero() || self.buy.amount.0.is_zero()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Partial {
    /// A partially order doesn't require the full amount to be traded.
    /// E.g. only 10% of the requested amount may be traded, if this leads
    /// to the most optimal solution.
    Yes {
        /// The available amount that can be used from the order.
        ///
        /// This amount considers both how much of the order has already been
        /// executed as well as the trader's balance.
        available: TargetAmount,
    },
    No,
}

/// The length of an order UID in bytes.
pub const UID_LEN: usize = 56;

/// UID of an order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Uid(pub FixedBytes<UID_LEN>);

impl From<&solvers_dto::solution::OrderUid> for Uid {
    fn from(value: &solvers_dto::solution::OrderUid) -> Self {
        Self(value.0.into())
    }
}

impl Uid {
    pub fn from_parts(order_hash: eth::B256, owner: eth::Address, valid_to: u32) -> Self {
        let mut bytes = [0; UID_LEN];
        bytes[0..32].copy_from_slice(order_hash.as_slice());
        bytes[32..52].copy_from_slice(owner.as_slice());
        bytes[52..56].copy_from_slice(&valid_to.to_be_bytes());
        Self(FixedBytes(bytes))
    }

    /// Address that authorized the order. Sell tokens will be taken
    /// from that address.
    pub fn owner(&self) -> eth::Address {
        self.parts().1
    }

    /// Returns a UNIX timestamp after which the settlement
    /// contract will not allow the order to be settled anymore.
    pub fn valid_to(&self) -> u32 {
        self.parts().2
    }

    /// Splits an order UID into its parts.
    fn parts(&self) -> (eth::B256, eth::Address, u32) {
        (
            eth::B256::from_slice(&self.0.0[0..32]),
            eth::Address::from_slice(&self.0.0[32..52]),
            u32::from_be_bytes(self.0.0[52..].try_into().unwrap()),
        )
    }
}

impl Default for Uid {
    fn default() -> Self {
        Self([0; UID_LEN].into())
    }
}

impl PartialEq<[u8; UID_LEN]> for Uid {
    fn eq(&self, other: &[u8; UID_LEN]) -> bool {
        self.0.0 == *other
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

impl From<model::order::OrderKind> for Side {
    fn from(value: model::order::OrderKind) -> Self {
        match value {
            model::order::OrderKind::Sell => Self::Sell,
            model::order::OrderKind::Buy => Self::Buy,
        }
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Order intended to be immediately executed. This is the "regular" type of
    /// order.
    Market,
    /// Order intended to be fulfilled possibly far into the future, when the
    /// price is such that the order can be executed. Because the fulfillment
    /// can happen any time into the future, it's impossible to calculate
    /// the order fees ahead of time, so the fees are taken from the order
    /// surplus instead.
    ///
    /// The order surplus is the additional money that the solver managed to
    /// solve for, above what the user specified in the order. The exact amount
    /// of fees that are taken is determined by the solver.
    Limit,
}

/// [Balancer V2](https://docs.balancer.fi/) integration, used for settlement encoding.
#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
pub enum SellTokenBalance {
    Erc20,
    Internal,
    External,
}

impl From<SellTokenBalance> for SellTokenSource {
    fn from(value: SellTokenBalance) -> Self {
        match value {
            SellTokenBalance::Erc20 => Self::Erc20,
            SellTokenBalance::Internal => Self::Internal,
            SellTokenBalance::External => Self::External,
        }
    }
}

impl From<SellTokenSource> for SellTokenBalance {
    fn from(value: SellTokenSource) -> Self {
        match value {
            SellTokenSource::Erc20 => Self::Erc20,
            SellTokenSource::Internal => Self::Internal,
            SellTokenSource::External => Self::External,
        }
    }
}

impl SellTokenBalance {
    /// Returns the hash value for the specified source.
    pub fn hash(&self) -> eth::B256 {
        let name = match self {
            Self::Erc20 => "erc20",
            Self::Internal => "internal",
            Self::External => "external",
        };
        alloy::primitives::keccak256(name.as_bytes())
    }
}

/// [Balancer V2](https://docs.balancer.fi/) integration, used for settlement encoding.
#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
pub enum BuyTokenBalance {
    Erc20,
    Internal,
}

impl From<BuyTokenBalance> for BuyTokenDestination {
    fn from(value: BuyTokenBalance) -> Self {
        match value {
            BuyTokenBalance::Erc20 => Self::Erc20,
            BuyTokenBalance::Internal => Self::Internal,
        }
    }
}

impl From<BuyTokenDestination> for BuyTokenBalance {
    fn from(value: BuyTokenDestination) -> Self {
        match value {
            BuyTokenDestination::Erc20 => Self::Erc20,
            BuyTokenDestination::Internal => Self::Internal,
        }
    }
}

/// The address which placed the order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Into)]
pub struct Trader(pub eth::Address);

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
    pub receiver: eth::Address,
    pub valid_to: util::Timestamp,
    pub partially_fillable: bool,
    pub app_data: app_data::AppDataHash,
    pub side: Side,
    pub sell_token_balance: SellTokenBalance,
    pub buy_token_balance: BuyTokenBalance,
    pub signature: Signature,
    pub uid: Uid,
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

    /// Returns the signed partially fillable property of the order. You can't
    /// set this field in the API so it's enforced to be fill-or-kill. This
    /// function only exists to not have magic values scattered everywhere.
    pub fn partially_fillable(&self) -> Partial {
        Partial::No
    }
}

#[derive(Clone, Debug)]
pub struct Quote {
    pub sell: eth::Asset,
    pub buy: eth::Asset,
    pub fee: eth::Asset,
    pub solver: eth::Address,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_scaling() {
        let sell = |amount: u64| eth::Asset {
            token: eth::Address::left_padding_from(0x5e11_u64.to_be_bytes().as_slice()).into(),
            amount: eth::U256::from(amount).into(),
        };
        let buy = |amount: u64| eth::Asset {
            token: eth::Address::left_padding_from(0xbbbb_u64.to_be_bytes().as_slice()).into(),
            amount: eth::U256::from(amount).into(),
        };

        let order = |sell_amount: u64, buy_amount: u64, available: Option<eth::Asset>| Order {
            uid: Default::default(),
            receiver: Default::default(),
            created: util::Timestamp(100),
            valid_to: util::Timestamp(u32::MAX),
            buy: buy(buy_amount),
            sell: sell(sell_amount),
            side: match available {
                None => Side::Sell,
                Some(executed) if executed.token == sell(0).token => Side::Sell,
                Some(executed) if executed.token == buy(0).token => Side::Buy,
                _ => panic!(),
            },
            kind: Kind::Limit,
            app_data: Default::default(),
            partial: available
                .map(|available| Partial::Yes {
                    available: available.amount.into(),
                })
                .unwrap_or(Partial::No),
            pre_interactions: Default::default(),
            post_interactions: Default::default(),
            sell_token_balance: SellTokenBalance::Erc20,
            buy_token_balance: BuyTokenBalance::Erc20,
            signature: Signature {
                scheme: signature::Scheme::PreSign,
                data: Default::default(),
                signer: Default::default(),
            },
            protocol_fees: Default::default(),
            quote: Default::default(),
        };

        assert_eq!(
            order(1000, 1000, Some(sell(750))).available().sell,
            sell(750)
        );
        assert_eq!(order(1000, 1000, Some(sell(750))).available().buy, buy(750));
        assert_eq!(
            order(1000, 1000, Some(buy(750))).available().sell,
            sell(750)
        );
        assert_eq!(order(1000, 1000, Some(buy(750))).available().buy, buy(750));

        assert_eq!(
            order(1000, 100, Some(sell(901))).available().sell,
            sell(901)
        );
        assert_eq!(order(1000, 100, Some(sell(901))).available().buy, buy(91));

        assert_eq!(order(100, 1000, Some(buy(901))).available().sell, sell(90));
        assert_eq!(order(100, 1000, Some(buy(901))).available().buy, buy(901));

        assert_eq!(order(1000, 1, Some(sell(500))).available().sell, sell(500));
        assert_eq!(order(1000, 1, Some(sell(500))).available().buy, buy(1));

        assert_eq!(order(1, 1000, Some(buy(500))).available().sell, sell(0));
        assert_eq!(order(1, 1000, Some(buy(500))).available().buy, buy(500));

        assert_eq!(order(0, 0, Some(sell(0))).available().sell, sell(0));
    }
}
