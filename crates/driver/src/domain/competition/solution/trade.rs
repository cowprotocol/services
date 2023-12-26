use {
    crate::domain::{
        competition::{self, order},
        eth,
    },
    std::collections::HashMap,
};

/// A trade which executes an order as part of this solution.
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum Trade {
    Fulfillment(Fulfillment),
    Jit(Jit),
}

/// A trade which fulfills an order from the auction.
#[derive(Debug, Clone)]
pub struct Fulfillment {
    order: competition::Order,
    /// The amount executed by this fulfillment. See [`order::Partial`]. If the
    /// order is not partial, the executed amount must equal the amount from the
    /// order.
    executed: order::TargetAmount,
    fee: Fee,
    protocol_fee: order::SellAmount,
}

impl Fulfillment {
    pub fn new(
        order: competition::Order,
        executed: order::TargetAmount,
        fee: Fee,
        uniform_sell_price: eth::U256,
        uniform_buy_price: eth::U256,
    ) -> Result<Self, InvalidFullfilment> {
        let protocol_fee = {
            let surplus_fee = match fee {
                Fee::Static => eth::U256::default(),
                Fee::Dynamic(fee) => fee.0,
            };

            let mut protocol_fee = Default::default();
            for fee_policy in &order.fee_policies {
                match fee_policy {
                    order::FeePolicy::PriceImprovement {
                        factor,
                        max_volume_factor,
                    } => {
                        let fee = match order.side {
                            order::Side::Buy => {
                                // How much `sell_token` we need to sell to buy `executed` amount of
                                // `buy_token`
                                let executed_sell_amount = executed
                                    .0
                                    .checked_mul(uniform_buy_price)
                                    .ok_or(InvalidFullfilment)?
                                    .checked_div(uniform_sell_price)
                                    .ok_or(InvalidFullfilment)?;
                                // We have to sell slightly more `sell_token` to capture the
                                // `surplus_fee`
                                let executed_sell_amount_with_surplus_fee = executed_sell_amount
                                    .checked_add(surplus_fee)
                                    .ok_or(InvalidFullfilment)?;
                                // What is the maximum amount of `sell_token` we are allowed to
                                // sell based on limit price?
                                // Equal to full sell amount for FOK orders, otherwise scalled with
                                // executed amount for partially fillable orders
                                let limit_sell_amount = order
                                    .sell
                                    .amount
                                    .0
                                    .checked_mul(executed.0)
                                    .ok_or(InvalidFullfilment)?
                                    .checked_div(order.buy.amount.0)
                                    .ok_or(InvalidFullfilment)?;
                                // Take protocol fee from the surplus
                                // Surplus is the diff between the limit price and executed amount
                                let surplus = limit_sell_amount
                                    .checked_sub(executed_sell_amount_with_surplus_fee)
                                    .ok_or(InvalidFullfilment)?;
                                let price_improvement_fee =
                                    surplus * (eth::U256::from_f64_lossy(factor * 100.)) / 100;
                                let max_volume_fee = executed_sell_amount_with_surplus_fee
                                    * (eth::U256::from_f64_lossy(max_volume_factor * 100.))
                                    / 100;
                                // take the smaller of the two
                                std::cmp::min(price_improvement_fee, max_volume_fee)
                            }
                            order::Side::Sell => {
                                // How much `buy_token` we get for `executed` amount of `sell_token`
                                let executed_buy_amount = executed
                                    .0
                                    .checked_mul(uniform_sell_price)
                                    .ok_or(InvalidFullfilment)?
                                    .checked_div(uniform_buy_price)
                                    .ok_or(InvalidFullfilment)?;
                                let executed_sell_amount = executed
                                    .0
                                    .checked_add(surplus_fee)
                                    .ok_or(InvalidFullfilment)?;
                                // What is the minimum amount of `buy_token` we have to buy based on
                                // limit price?
                                // Equal to full buy amount for FOK orders, otherwise scalled with
                                // executed amount for partially fillable orders
                                let limit_buy_amount = order
                                    .buy
                                    .amount
                                    .0
                                    .checked_mul(executed_sell_amount)
                                    .ok_or(InvalidFullfilment)?
                                    .checked_div(order.sell.amount.0)
                                    .ok_or(InvalidFullfilment)?;
                                // Bought exactly `executed_buy_amount` while the limit price is
                                // `limit_buy_amount` Take protocol fee from the surplus
                                let surplus = executed_buy_amount
                                    .checked_sub(limit_buy_amount)
                                    .ok_or(InvalidFullfilment)?;
                                let surplus_in_sell_token = surplus
                                    .checked_mul(uniform_buy_price)
                                    .ok_or(InvalidFullfilment)?
                                    .checked_div(uniform_sell_price)
                                    .ok_or(InvalidFullfilment)?;
                                let price_improvement_fee = surplus_in_sell_token
                                    * (eth::U256::from_f64_lossy(factor * 100.))
                                    / 100;
                                let max_volume_fee = executed_sell_amount
                                    * (eth::U256::from_f64_lossy(max_volume_factor * 100.))
                                    / 100;
                                // take the smaller of the two
                                std::cmp::min(price_improvement_fee, max_volume_fee)
                            }
                        };
                        protocol_fee += fee;
                    }
                    order::FeePolicy::Volume { factor: _ } => unimplemented!(),
                }
            }
            order::SellAmount(protocol_fee)
        };

        // Adjust the executed amount by the protocol fee. This is because solvers are
        // unaware of the protocol fee that driver introduces and they only account
        // for the surplus fee.
        let executed = match order.side {
            order::Side::Buy => executed,
            order::Side::Sell => order::TargetAmount(
                executed
                    .0
                    .checked_sub(protocol_fee.0)
                    .ok_or(InvalidFullfilment)?,
            ),
        };

        // If the order is partial, the total executed amount can be smaller than
        // the target amount. Otherwise, the executed amount must be equal to the target
        // amount.
        let valid_execution = {
            let surplus_fee = match order.side {
                order::Side::Buy => order::TargetAmount::default(),
                order::Side::Sell => order::TargetAmount(match fee {
                    Fee::Static => eth::U256::default(),
                    Fee::Dynamic(fee) => fee.0,
                }),
            };

            let protocol_fee = match order.side {
                order::Side::Buy => order::TargetAmount::default(),
                order::Side::Sell => order::TargetAmount(protocol_fee.0),
            };

            match order.partial {
                order::Partial::Yes { available } => {
                    executed + surplus_fee + protocol_fee <= available
                }
                order::Partial::No => executed + surplus_fee + protocol_fee == order.target(),
            }
        };

        // Only accept solver-computed fees if the order requires them, otherwise the
        // protocol pre-determines the fee and the solver must respect it.
        let valid_fee = match &fee {
            Fee::Static => !order.solver_determines_fee(),
            Fee::Dynamic(_) => order.solver_determines_fee(),
        };

        if valid_execution && valid_fee {
            Ok(Self {
                order,
                executed,
                fee,
                protocol_fee,
            })
        } else {
            Err(InvalidFullfilment)
        }
    }

    pub fn order(&self) -> &competition::Order {
        &self.order
    }

    pub fn executed(&self) -> order::TargetAmount {
        self.executed
    }

    /// Returns the fee that should be considered as collected when
    /// scoring a solution.
    pub fn scoring_fee(&self) -> order::SellAmount {
        match self.fee {
            Fee::Static => self.order.fee.solver,
            Fee::Dynamic(fee) => fee + self.protocol_fee,
        }
    }

    /// Returns the effectively paid fee from the user's perspective
    /// considering their signed order and the uniform clearing prices
    pub fn fee(&self) -> order::SellAmount {
        match self.fee {
            Fee::Static => self.order.fee.user,
            Fee::Dynamic(fee) => fee + self.protocol_fee,
        }
    }

    /// The effective amount that left the user's wallet including all fees.
    pub fn sell_amount(
        &self,
        prices: &HashMap<eth::TokenAddress, eth::U256>,
        weth: eth::WethAddress,
    ) -> Option<eth::TokenAmount> {
        let before_fee = match self.order.side {
            order::Side::Sell => self.executed.0,
            order::Side::Buy => self
                .executed
                .0
                .checked_mul(*prices.get(&self.order.buy.token.wrap(weth))?)?
                .checked_div(*prices.get(&self.order.sell.token.wrap(weth))?)?,
        };
        Some(eth::TokenAmount(before_fee.checked_add(self.fee().0)?))
    }

    /// The effective amount the user received after all fees.
    pub fn buy_amount(
        &self,
        prices: &HashMap<eth::TokenAddress, eth::U256>,
        weth: eth::WethAddress,
    ) -> Option<eth::TokenAmount> {
        let amount = match self.order.side {
            order::Side::Buy => self.executed.0,
            order::Side::Sell => self
                .executed
                .0
                .checked_mul(*prices.get(&self.order.sell.token.wrap(weth))?)?
                .checked_div(*prices.get(&self.order.buy.token.wrap(weth))?)?,
        };
        Some(eth::TokenAmount(amount))
    }
}

/// A fee that is charged for executing an order.
#[derive(Clone, Copy, Debug)]
pub enum Fee {
    /// A static protocol computed fee.
    ///
    /// That is, the fee is known upfront and is signed as part of the order
    Static,
    /// A dynamic solver computed surplus fee.
    Dynamic(order::SellAmount),
}

/// A trade which adds a JIT order. See [`order::Jit`].
#[derive(Debug, Clone)]
pub struct Jit {
    order: order::Jit,
    /// The amount executed by this JIT trade. See
    /// [`order::Jit::partially_fillable`]. If the order is not
    /// partially fillable, the executed amount must equal the amount from the
    /// order.
    executed: order::TargetAmount,
}

impl Jit {
    pub fn new(
        order: order::Jit,
        executed: order::TargetAmount,
    ) -> Result<Self, InvalidFullfilment> {
        // If the order is partially fillable, the executed amount can be smaller than
        // the target amount. Otherwise, the executed amount must be equal to the target
        // amount.
        let is_valid = if order.partially_fillable {
            executed <= order.target()
        } else {
            executed == order.target()
        };
        if is_valid {
            Ok(Self { order, executed })
        } else {
            Err(InvalidFullfilment)
        }
    }

    pub fn order(&self) -> &order::Jit {
        &self.order
    }

    pub fn executed(&self) -> order::TargetAmount {
        self.executed
    }
}

/// The amounts executed by a trade.
#[derive(Debug, Clone, Copy)]
pub struct Execution {
    /// The total amount being sold.
    pub sell: eth::Asset,
    /// The total amount being bought.
    pub buy: eth::Asset,
}

#[derive(Debug, thiserror::Error)]
#[error("invalid executed amount")]
pub struct InvalidFullfilment;

#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    #[error("overflow error while calculating executed amounts")]
    Overflow,
    #[error("missing clearing price for {0:?}")]
    ClearingPriceMissing(eth::TokenAddress),
}

mod tests {
    use {
        super::*,
        crate::{
            domain::competition::order::{
                signature::Scheme,
                AppData,
                BuyTokenBalance,
                FeePolicy,
                SellTokenBalance,
                Signature,
            },
            util,
        },
        primitive_types::{H160, U256},
        std::str::FromStr,
    };

    #[test]
    fn test_fulfillment_sell_limit_order_fok() {
        // https://explorer.cow.fi/orders/0xef6de27933bde867c768ead05d34a08c806d35b89f6bea565bdeb40108265e9a6f419390da10911abd1e1c962b569312a9c9c7b1658a2936?tab=overview
        let order = competition::Order {
            uid: Default::default(),
            side: order::Side::Sell,
            buy: eth::Asset {
                token: eth::TokenAddress(eth::ContractAddress(
                    H160::from_str("0xba3335588d9403515223f109edc4eb7269a9ab5d").unwrap(),
                )),
                amount: eth::TokenAmount(778310860032541096349039u128.into()),
            },
            sell: eth::Asset {
                token: eth::TokenAddress(eth::ContractAddress(
                    H160::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap(),
                )),
                amount: eth::TokenAmount(4166666666666666666u128.into()),
            },
            kind: order::Kind::Limit,
            fee: Default::default(),
            fee_policies: vec![FeePolicy::PriceImprovement {
                factor: 0.5,
                max_volume_factor: 1.0,
            }],
            partial: order::Partial::No,
            receiver: Default::default(),
            pre_interactions: Default::default(),
            post_interactions: Default::default(),
            valid_to: util::Timestamp(0),
            app_data: AppData(Default::default()),
            sell_token_balance: SellTokenBalance::Erc20,
            buy_token_balance: BuyTokenBalance::Erc20,
            signature: Signature {
                scheme: Scheme::Eip712,
                data: Default::default(),
                signer: eth::Address::default(),
            },
        };

        // taken from https://production-6de61f.kb.eu-central-1.aws.cloud.es.io/app/discover#/doc/c0e240e0-d9b3-11ed-b0e6-e361adffce0b/cowlogs-prod-2023.12.25?id=m8dnoowB4Ql8nk7a5ber
        let uniform_sell_price = eth::U256::from(913320970421237626580182u128);
        let uniform_buy_price = eth::U256::from(4149866666666666668u128);
        let executed = order::TargetAmount(4149866666666666668u128.into());
        let fee = Fee::Dynamic(order::SellAmount(16799999999999998u128.into()));
        let fulfillment = Fulfillment::new(
            order.clone(),
            executed,
            fee,
            uniform_sell_price,
            uniform_buy_price,
        )
        .unwrap();
        // fee contains protocol fee
        assert_eq!(
            fulfillment.fee(),
            order::SellAmount((16799999999999998u128 + 306723471216604081u128).into())
        );
        // executed amount reduced by protocol fee
        assert_eq!(
            fulfillment.executed(),
            U256::from(3843143195450062587u128).into()
        ); // 4149866666666666668 - 306723471216604081
    }

    #[test]
    pub fn test_fullfilment_buy_limit_order_fok() {
        // https://explorer.cow.fi/orders/0xc9096a3dbfb1f661e65ecc14644adec6bd8e385ae818aa73181def24996affb589e4042fd85e857e81a4fa89831b1f5ad4f384b7659357d7?tab=overview
        let order = competition::Order {
            uid: Default::default(),
            side: order::Side::Buy,
            buy: eth::Asset {
                token: eth::TokenAddress(eth::ContractAddress(
                    H160::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap(),
                )),
                amount: eth::TokenAmount(170000000000000000u128.into()),
            },
            sell: eth::Asset {
                token: eth::TokenAddress(eth::ContractAddress(
                    H160::from_str("0xdef1ca1fb7fbcdc777520aa7f396b4e015f497ab").unwrap(),
                )),
                amount: eth::TokenAmount(1781433576205823004786u128.into()),
            },
            kind: order::Kind::Limit,
            fee: Default::default(),
            fee_policies: vec![FeePolicy::PriceImprovement {
                factor: 0.5,
                max_volume_factor: 1.0,
            }],
            partial: order::Partial::No,
            receiver: Default::default(),
            pre_interactions: Default::default(),
            post_interactions: Default::default(),
            valid_to: util::Timestamp(0),
            app_data: AppData(Default::default()),
            sell_token_balance: SellTokenBalance::Erc20,
            buy_token_balance: BuyTokenBalance::Erc20,
            signature: Signature {
                scheme: Scheme::Eip712,
                data: Default::default(),
                signer: eth::Address::default(),
            },
        };

        // taken from https://production-6de61f.kb.eu-central-1.aws.cloud.es.io/app/discover#/doc/c0e240e0-d9b3-11ed-b0e6-e361adffce0b/cowlogs-prod-2023.12.26?id=cYSDo4wBlutGF6Gybl6x
        let uniform_sell_price = eth::U256::from(7213317128720734077u128);
        let uniform_buy_price = eth::U256::from(74745150907421124481191u128);
        let executed = order::TargetAmount(170000000000000000u128.into());
        let fee = Fee::Dynamic(order::SellAmount(19868323826701104280u128.into()));
        let fulfillment = Fulfillment::new(
            order.clone(),
            executed,
            fee,
            uniform_sell_price,
            uniform_buy_price,
        )
        .unwrap();
        // fee contains protocol fee
        assert_eq!(
            fulfillment.fee(),
            order::SellAmount((19868323826701104280u128 + 3684441086061450u128).into())
        );
        // executed amount same as before
        assert_eq!(fulfillment.executed(), executed);
    }
}
