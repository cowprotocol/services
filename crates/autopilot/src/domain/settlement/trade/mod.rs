use {
    super::{transaction, transaction::Prices},
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
    /// A regular user order that exist in the orderbook.
    Fulfillment(Fulfillment),
    // JIT trades are not part of the orderbook and are created by solvers at the time of
    // settlement.
    Jit(Jit),
}

impl Trade {
    /// Order UID that was settled in this trade.
    pub fn uid(&self) -> &domain::OrderUid {
        match self {
            Self::Fulfillment(trade) => &trade.uid,
            Self::Jit(trade) => &trade.uid,
        }
    }

    /// Return JIT order if it's a JIT order.
    pub fn as_jit(&self) -> Option<&Jit> {
        match self {
            Self::Fulfillment(_) => None,
            Self::Jit(trade) => Some(trade),
        }
    }

    /// CIP38 score defined as surplus + protocol fee
    pub fn score(&self, auction: &super::Auction) -> Result<eth::Ether, math::Error> {
        match self {
            Self::Fulfillment(trade) => math::Trade::from(trade.clone()).score(auction),
            Self::Jit(trade) => math::Trade::from(trade.clone()).score(auction),
        }
    }

    /// Surplus of a trade.
    pub fn surplus_in_ether(&self, prices: &auction::Prices) -> Result<eth::Ether, math::Error> {
        match self {
            Self::Fulfillment(trade) => math::Trade::from(trade.clone()).surplus_in_ether(prices),
            Self::Jit(trade) => {
                if trade.surplus_capturing {
                    math::Trade::from(trade.clone()).surplus_in_ether(prices)
                } else {
                    Ok(eth::Ether::zero())
                }
            }
        }
    }

    /// Total fee taken for the trade.
    pub fn fee_in_ether(&self, prices: &auction::Prices) -> Result<eth::Ether, math::Error> {
        match self {
            Self::Fulfillment(trade) => math::Trade::from(trade.clone()).fee_in_ether(prices),
            Self::Jit(trade) => math::Trade::from(trade.clone()).fee_in_ether(prices),
        }
    }

    /// Total fee (protocol fee + network fee). Equal to a surplus difference
    /// before and after applying the fees.
    pub fn fee_in_sell_token(&self) -> Result<eth::SellTokenAmount, math::Error> {
        match self {
            Self::Fulfillment(trade) => math::Trade::from(trade.clone()).fee_in_sell_token(),
            Self::Jit(trade) => math::Trade::from(trade.clone()).fee_in_sell_token(),
        }
    }

    /// Protocol fees are defined by fee policies attached to the order.
    pub fn protocol_fees_in_sell_token(
        &self,
        auction: &super::Auction,
    ) -> Result<Vec<(eth::SellTokenAmount, fee::Policy)>, math::Error> {
        match self {
            Self::Fulfillment(trade) => {
                math::Trade::from(trade.clone()).protocol_fees_in_sell_token(auction)
            }
            Self::Jit(trade) => {
                math::Trade::from(trade.clone()).protocol_fees_in_sell_token(auction)
            }
        }
    }

    pub fn new(trade: transaction::EncodedTrade, auction: &super::Auction, created: u32) -> Self {
        if auction.orders.contains_key(&trade.uid) {
            Trade::Fulfillment(Fulfillment {
                uid: trade.uid,
                sell: trade.sell,
                buy: trade.buy,
                side: trade.side,
                executed: trade.executed,
                prices: trade.prices,
            })
        } else {
            Trade::Jit(Jit {
                uid: trade.uid,
                sell: trade.sell,
                buy: trade.buy,
                side: trade.side,
                receiver: trade.receiver,
                valid_to: trade.valid_to,
                app_data: trade.app_data,
                fee_amount: trade.fee_amount,
                sell_token_balance: trade.sell_token_balance,
                buy_token_balance: trade.buy_token_balance,
                partially_fillable: trade.partially_fillable,
                signature: trade.signature,
                executed: trade.executed,
                prices: trade.prices,
                created,
                surplus_capturing: auction
                    .surplus_capturing_jit_order_owners
                    .contains(&trade.uid.owner()),
            })
        }
    }
}

/// Trade representing an user trade. User trades are part of the orderbook.
#[derive(Debug, Clone)]
pub struct Fulfillment {
    pub uid: domain::OrderUid,
    pub sell: eth::Asset,
    pub buy: eth::Asset,
    pub side: order::Side,
    pub executed: order::TargetAmount,
    pub prices: Prices,
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
    pub partially_fillable: bool,
    pub signature: order::Signature,
    pub executed: order::TargetAmount,
    pub prices: super::transaction::Prices,
    pub created: u32,
    pub surplus_capturing: bool,
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
