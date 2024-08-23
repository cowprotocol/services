use {
    super::{transaction, transaction::Prices},
    crate::domain::{
        self,
        auction::{self, order},
        eth,
        fee,
    },
    bigdecimal::Zero,
    std::collections::HashSet,
};

mod math;

/// Trade type when evaluated in a context of an Auction.
#[derive(Clone, Debug)]
pub enum Trade {
    /// A regular user order that was pare part of the `Auction`.
    Fulfillment(Fulfillment),
    /// A regular user order that was not part of the `Auction`.
    FulfillmentOutOfAuction(Fulfillment),
    // JIT trades are not part of the orderbook and are created by solvers at the time of
    // settlement.
    Jit(Jit),
}

impl Trade {
    /// Order UID that was settled in this trade.
    pub fn uid(&self) -> &domain::OrderUid {
        match self {
            Self::Fulfillment(trade) => &trade.uid,
            Self::FulfillmentOutOfAuction(trade) => &trade.uid,
            Self::Jit(trade) => &trade.uid,
        }
    }

    /// Return JIT order if it's a JIT order.
    pub fn as_jit(&self) -> Option<&Jit> {
        match self {
            Self::Fulfillment(_) => None,
            Self::FulfillmentOutOfAuction(_) => None,
            Self::Jit(trade) => Some(trade),
        }
    }

    /// CIP38 score defined as surplus + protocol fee
    pub fn score(&self, auction: &super::Auction) -> Result<eth::Ether, math::Error> {
        self.as_math().score(auction)
    }

    /// Surplus of a trade.
    pub fn surplus_in_ether(&self, prices: &auction::Prices) -> Result<eth::Ether, math::Error> {
        match self {
            Self::Fulfillment(trade) => trade.as_math().surplus_in_ether(prices),
            Self::FulfillmentOutOfAuction(_) => Ok(eth::Ether::zero()),
            Self::Jit(trade) => {
                if trade.surplus_capturing {
                    trade.as_math().surplus_in_ether(prices)
                } else {
                    Ok(eth::Ether::zero())
                }
            }
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
            Trade::Fulfillment(trade) => trade.as_math(),
            Trade::FulfillmentOutOfAuction(trade) => trade.as_math(),
            Trade::Jit(trade) => trade.as_math(),
        }
    }

    pub fn new(
        trade: transaction::EncodedTrade,
        auction: &super::Auction,
        database_orders: &HashSet<domain::OrderUid>,
        created: u32,
    ) -> Self {
        // If it's not in a database, then it's definitely a JIT order
        if !database_orders.contains(&trade.uid) {
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
                signature: trade.signature,
                executed: trade.executed,
                prices: trade.prices,
                created,
                surplus_capturing: auction
                    .surplus_capturing_jit_order_owners
                    .contains(&trade.uid.owner()),
            })
        } else {
            // Otherwise it's a user order from orderbook, it's just a question whether it's
            // in the auction or not
            if auction.orders.contains_key(&trade.uid) {
                Trade::Fulfillment(Fulfillment {
                    uid: trade.uid,
                    sell: trade.sell,
                    buy: trade.buy,
                    side: trade.side,
                    executed: trade.executed,
                    prices: trade.prices,
                })
            }
            // User order that was settled outside of the auction
            else {
                Trade::FulfillmentOutOfAuction(Fulfillment {
                    uid: trade.uid,
                    sell: trade.sell,
                    buy: trade.buy,
                    side: trade.side,
                    executed: trade.executed,
                    prices: trade.prices,
                })
            }
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

impl Fulfillment {
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
    pub surplus_capturing: bool,
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
