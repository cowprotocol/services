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
        let protocol = trade.protocol_fees(&auction.orders)?;
        // Protocol fees only use custom prices, so whenever the uniform-price
        // total fails (e.g. a solver paid an order more than the uniform
        // price), fall back to them (no network fee).
        let total = trade
            .fee_in_sell_token()
            .or_else(|_| trade.protocol_fee_in_sell_token(&protocol))?;
        Ok(FeeBreakdown {
            total: eth::Asset {
                token: self.sell_token(),
                amount: total.into(),
            },
            protocol,
        })
    }

    pub fn sell_token(&self) -> eth::TokenAddress {
        match self {
            Self::Fulfillment(trade) => trade.sell.token,
            Self::Jit(trade) => trade.sell.token,
        }
    }

    pub fn new(trade: transaction::EncodedTrade, auction: &super::Auction, created: u32) -> Self {
        // Without a uniform price the fee can't be derived, so book none.
        // Protocol fees only need the custom prices.
        let uniform = || {
            trade.uniform_prices.unwrap_or_else(|| {
                tracing::warn!(uid = %trade.uid, "settlement lists no uniform price for trade");
                // NOTE: this will lead to the fee being shown in FE as 0
                // because it's calculated as:
                // 1. `surplus(uniform) - surplus(custom)`
                // 2. `surplus(custom) - surplus(custom) = 0`
                trade.custom_prices
            })
        };
        if auction.orders.contains_key(&trade.uid) {
            return Trade::Fulfillment(Fulfillment {
                uid: trade.uid,
                sell: trade.sell,
                buy: trade.buy,
                side: trade.side,
                executed: trade.executed,
                prices: Prices {
                    uniform: uniform(),
                    custom: trade.custom_prices,
                },
            });
        }

        // All orders that were settled outside of the auction are JIT
        // orders. This includes regular JIT orders that the
        // protocol is not aware of upfront, as well as user
        // orders that were not listed in the auction during competition.
        let surplus_capturing = auction
            .surplus_capturing_jit_order_owners
            .contains(&trade.uid.owner());

        let prices = if surplus_capturing {
            Prices {
                uniform: uniform(),
                custom: trade.custom_prices,
            }
        } else {
            // for non-surplus capturing jit orders (AKA liquidity JIT
            // orders) the expectation is that the
            // trade was executed at its limit price, without
            // incurred fees.
            Prices {
                uniform: trade.custom_prices,
                custom: trade.custom_prices,
            }
        };

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
            prices,
            created,
            surplus_capturing,
        })
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
    pub receiver: eth::Receiver,
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

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::domain::settlement::Auction,
        configs::fee_factor::FeeFactor,
        std::collections::HashMap,
        transaction::ClearingPrices,
    };

    /// The trade from prod mainnet settlement
    /// 0xe730d6c5ffe17e6ebfbdbb5b073f730510ce774c49ee5846937398d64d10ebcc
    /// (auction 14123387). The solver paid the order its limit price while the
    /// uniform price sat below that limit, so the uniform-price total fails.
    #[test]
    fn fee_breakdown_falls_back_to_protocol_fees() {
        let u = |amount: u128| eth::U256::from(amount);
        let uid = domain::OrderUid([0; 56]);
        let trade = Trade::Fulfillment(Fulfillment {
            uid,
            sell: eth::Asset {
                amount: u(3000000000).into(),
                token: eth::Address::default().into(),
            },
            buy: eth::Asset {
                amount: u(170454545454545454545454).into(),
                token: eth::Address::default().into(),
            },
            side: order::Side::Sell,
            executed: order::TargetAmount(u(2885193698)),
            prices: Prices {
                uniform: ClearingPrices {
                    sell: u(440391362314230861497954714),
                    buy: u(7835823451623),
                },
                custom: ClearingPrices {
                    sell: u(163931542146642908821950),
                    buy: u(2885193698),
                },
            },
        });
        let auction = Auction {
            id: 14123387,
            block: eth::BlockNo(0),
            orders: HashMap::from([(
                uid,
                vec![
                    fee::Policy::Surplus {
                        factor: FeeFactor::try_from(0.5).unwrap(),
                        max_volume_factor: FeeFactor::try_from(0.0098).unwrap(),
                    },
                    fee::Policy::Volume {
                        factor: FeeFactor::try_from(0.0002).unwrap(),
                    },
                ],
            )]),
            prices: Default::default(),
            surplus_capturing_jit_order_owners: Default::default(),
        };
        assert!(math::Trade::from(&trade).fee_in_sell_token().is_err());

        let breakdown = trade.fee_breakdown(&auction).unwrap();

        assert_eq!(breakdown.total.amount.0, u(1168417));
        assert_eq!(
            breakdown
                .protocol
                .iter()
                .map(|fee| fee.fee.amount.0)
                .collect::<Vec<_>>(),
            [u(32874900009274260707), u(32792867002729075121)]
        );
    }
}
