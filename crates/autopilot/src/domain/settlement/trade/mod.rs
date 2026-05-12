use {
    super::{transaction, transaction::Prices},
    crate::domain::{
        self,
        auction::{self, order},
        fee,
    },
    bigdecimal::Zero,
    eth_domain_types as eth,
};

pub mod math;

/// Trade type evaluated in a context of an Auction.
#[derive(Clone, Debug)]
pub enum Trade {
    /// A regular user order that exist in the associated Auction.
    Fulfillment(Fulfillment),
    /// JIT trades are not part of the orderbook and are created by solvers at
    /// the time of settlement.
    /// Note that user orders can also be classified as JIT orders if they are
    /// settled outside of the Auction.
    Jit(Jit),
}

impl Trade {
    /// UID of the order that was settled in this trade.
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

    /// Surplus of a trade.
    pub fn surplus_in_ether(&self, prices: &auction::Prices) -> Result<eth::Ether, math::Error> {
        match self {
            Self::Fulfillment(trade) => math::Trade::from(trade).surplus_in_ether(prices),
            Self::Jit(trade) => {
                if trade.surplus_capturing {
                    math::Trade::from(trade).surplus_in_ether(prices)
                } else {
                    // JIT orders that are not surplus capturing have zero
                    // surplus, even if they settled at a better price than
                    // limit price.
                    Ok(eth::Ether::zero())
                }
            }
        }
    }

    /// Total fee taken for the trade.
    pub fn fee_in_ether(&self, prices: &auction::Prices) -> Result<eth::Ether, math::Error> {
        math::Trade::from(self).fee_in_ether(prices)
    }

    /// All fees broke down into protocol fees per policy and total fee.
    pub fn fee_breakdown(&self, auction: &super::Auction) -> Result<FeeBreakdown, math::Error> {
        let trade = math::Trade::from(self);
        let total = trade.fee_in_sell_token()?;
        let protocol = trade.protocol_fees(&auction.orders)?;
        Ok(FeeBreakdown {
            total: eth::Asset {
                token: self.sell_token(),
                amount: total.into(),
            },
            protocol,
        })
    }

    /// calculate all metrics (surplus, fee, breakdown) in a single
    /// pass.
    pub fn calculate_all_metrics(
        &self,
        auction: &super::Auction,
    ) -> Result<TradeMetrics, math::Error> {
        let math_trade = math::Trade::from(self);

        let surplus_after = math_trade.surplus_over_limit_price()?;
        let surplus_before = math_trade.surplus_over_limit_price_before_fee()?;

        let surplus = math_trade.surplus_to_ether(surplus_after, &auction.prices)?;
        let surplus = self.apply_jit_surplus_rules(surplus);

        let fee = math_trade.fee_to_ether(surplus_before, surplus_after, &auction.prices)?;

        let breakdown = math_trade.fee_breakdown_from_surplus(
            surplus_before,
            surplus_after,
            &auction.orders,
        )?;

        Ok(TradeMetrics {
            surplus,
            fee,
            breakdown,
        })
    }

    fn apply_jit_surplus_rules(&self, surplus: eth::Ether) -> eth::Ether {
        match self {
            Self::Jit(trade) if !trade.surplus_capturing => eth::Ether::zero(),
            _ => surplus,
        }
    }

    pub fn sell_token(&self) -> eth::TokenAddress {
        match self {
            Self::Fulfillment(trade) => trade.sell.token,
            Self::Jit(trade) => trade.sell.token,
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
            // All orders that were settled outside of the auction are JIT orders. This
            // includes regular JIT orders that the protocol is not aware of upfront, as
            // well as user orders that were not listed in the auction during competition.
            let surplus_capturing = auction
                .surplus_capturing_jit_order_owners
                .contains(&trade.uid.owner());
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
                prices: if surplus_capturing {
                    trade.prices
                } else {
                    // for non-surplus capturing jit orders (AKA liquidity JIT orders) the
                    // expectation is that the trade was executed at its limit price, without
                    // incurred fees.
                    Prices {
                        uniform: trade.prices.custom,
                        custom: trade.prices.custom,
                    }
                },
                created,
                surplus_capturing,
            })
        }
    }
}

/// A trade event emitted by a settlement smart contract.
#[derive(Debug, Clone, Copy)]
pub struct TradeEvent {
    pub block: eth::BlockNo,
    pub log_index: u64,
    pub order_uid: model::order::OrderUid,
}

/// A trade filling an order that was part of the auction.
#[derive(Debug, Clone)]
pub struct Fulfillment {
    uid: domain::OrderUid,
    sell: eth::Asset,
    buy: eth::Asset,
    side: order::Side,
    executed: order::TargetAmount,
    prices: Prices,
}

/// A trade filling an order that was not part of the auction.
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

/// Combined metrics for a trade calculated in a single pass.
#[derive(Debug)]
pub struct TradeMetrics {
    /// Surplus generated by the trade in ETH.
    pub surplus: eth::Ether,
    /// Total fee charged for the trade in ETH.
    pub fee: eth::Ether,
    /// Detailed breakdown of fees.
    pub breakdown: FeeBreakdown,
}

/// Fee per trade in a solution. These fees are taken for the execution of the
/// trade.
#[derive(Debug, Clone)]
pub struct FeeBreakdown {
    /// Total fee the trade was charged (network fee + protocol fee)
    // TODO surplus token
    pub total: eth::Asset,
    /// Breakdown of protocol fees.
    pub protocol: Vec<ExecutedProtocolFee>,
}

#[derive(Debug, Clone)]
pub struct ExecutedProtocolFee {
    /// Policy that was used to calculate the fee.
    pub policy: fee::Policy,
    /// Fee that was taken for the trade, in surplus token.
    pub fee: eth::Asset,
}
