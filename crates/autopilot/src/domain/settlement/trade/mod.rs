use {
    super::transaction::Prices,
    crate::domain::{
        self,
        auction::{self, order},
        eth,
        fee,
    },
    bigdecimal::Zero,
};

mod math;

/// Trade type when evaluated in a context of an Auction.
#[derive(Clone, Debug)]
pub enum Trade {
    /// A regular user order. These orders are part of the `Auction`.
    User(User),
    /// A regular user order that was not part of the `Auction`.
    UserOutOfAuction(User),
    /// A JIT order that captures surplus. These orders are usually not part of
    /// the `Auction`.
    SurplusCapturingJit(Jit),
    /// A JIT order that does not capture surplus, doesn't apply for protocol
    /// fees and is filled at it's limit prices. These orders are never part of
    /// the `Auction`.
    Jit(Jit),
}

impl Trade {
    /// Order UID that was settled in this trade.
    pub fn uid(&self) -> &domain::OrderUid {
        match self {
            Self::User(trade) => &trade.uid,
            Self::UserOutOfAuction(trade) => &trade.uid,
            Self::SurplusCapturingJit(trade) => &trade.uid,
            Self::Jit(trade) => &trade.uid,
        }
    }

    /// Return JIT order if it's a JIT order.
    pub fn as_jit(&self) -> Option<&Jit> {
        match self {
            Self::User(_) => None,
            Self::UserOutOfAuction(_) => None,
            Self::SurplusCapturingJit(trade) => Some(trade),
            Self::Jit(trade) => Some(trade),
        }
    }

    /// Surplus of a trade.
    pub fn surplus_in_ether(&self, prices: &auction::Prices) -> Result<eth::Ether, math::Error> {
        match self {
            Self::User(trade) => trade.as_math().surplus_in_ether(prices),
            Self::UserOutOfAuction(_) => Ok(eth::Ether::zero()),
            Self::SurplusCapturingJit(trade) => trade.as_math().surplus_in_ether(prices),
            Self::Jit(_) => Ok(eth::Ether::zero()),
        }
    }

    /// Total fee taken for the trade.
    pub fn fee_in_ether(&self, prices: &auction::Prices) -> Result<eth::Ether, math::Error> {
        self.as_math().fee_in_ether(prices)
    }

    /// Total fee (protocol fee + network fee). Equal to a surplus difference
    /// before and after applying the fees.
    pub fn fee_in_sell_token(&self) -> Result<eth::SellTokenAmount, math::Error> {
        self.as_math().fee_in_sell_token()
    }

    /// Protocol fees are defined by fee policies attached to the order.
    pub fn protocol_fees_in_sell_token(
        &self,
        auction: &super::Auction,
    ) -> Result<Vec<(eth::SellTokenAmount, fee::Policy)>, math::Error> {
        self.as_math().protocol_fees_in_sell_token(auction)
    }

    /// Represent the current trade as a math trade, for which we know how
    /// to calculate surplus, fees, etc.
    fn as_math(&self) -> math::Trade {
        match self {
            Trade::User(trade) => trade.as_math(),
            Trade::UserOutOfAuction(trade) => trade.as_math(),
            Trade::SurplusCapturingJit(trade) => trade.as_math(),
            Trade::Jit(trade) => trade.as_math(),
        }
    }
}

/// Trade representing an user trade. User trades are part of the orderbook.
#[derive(Debug, Clone)]
pub struct User {
    pub uid: domain::OrderUid,
    pub sell: eth::Asset,
    pub buy: eth::Asset,
    pub side: order::Side,
    pub executed: order::TargetAmount,
    pub prices: Prices,
}

impl User {
    /// Converts the trade to a math trade, which can be used to calculate
    /// surplus, fees, etc.
    pub fn as_math(&self) -> math::Trade {
        math::Trade {
            uid: self.uid,
            sell: self.sell,
            buy: self.buy,
            side: self.side,
            executed: self.executed,
            prices: self.prices,
        }
    }
}

/// Trade representing a JIT trade. JIT trades are not part of the orderbook and
/// are created by solvers at the time of settlement.
#[derive(Debug, Clone)]
pub struct Jit {
    pub uid: domain::OrderUid,
    pub sell: eth::Asset,
    pub buy: eth::Asset,
    pub side: order::Side,
    pub receiver: eth::Address,
    pub valid_to: u32,
    pub app_data: order::AppDataHash,
    pub fee_amount: eth::TokenAmount,
    pub sell_token_balance: order::SellTokenSource,
    pub buy_token_balance: order::BuyTokenDestination,
    pub signature: order::Signature,
    pub executed: order::TargetAmount,
    pub prices: super::transaction::Prices,
    pub created: u32,
}

impl Jit {
    /// Converts the trade to a math trade, which can be used to calculate
    /// surplus, fees, etc.
    pub fn as_math(&self) -> math::Trade {
        math::Trade {
            uid: self.uid,
            sell: self.sell,
            buy: self.buy,
            side: self.side,
            executed: self.executed,
            prices: self.prices,
        }
    }
}

/// Fee per trade in a solution. These fees are taken for the execution of the
/// trade.
#[derive(Debug, Clone)]
pub struct ExecutedFee {
    /// Gas fee spent to bring the order onchain
    pub network: eth::SellTokenAmount,
    /// Breakdown of protocol fees. Executed protocol fees are in the same order
    /// as policies are defined for an order.
    pub protocol: Vec<(eth::SellTokenAmount, fee::Policy)>,
}

impl ExecutedFee {
    /// Total fee paid for the trade.
    pub fn total(&self) -> eth::SellTokenAmount {
        self.network + self.protocol.iter().map(|(fee, _)| *fee).sum()
    }
}
