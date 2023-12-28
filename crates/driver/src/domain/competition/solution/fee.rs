//! Applies the protocol fee to the solution received from the solver.
//!
//! Solvers respond differently for the sell and buy orders.
//! 
//! EXAMPLES:
//! 
//! SELL ORDER
//! Selling 1 WETH for at least `x` amount of USDC. Solvers respond with
//! Fee = 0.05 WETH
//! Executed = 0.95 WETH
//! 
//! This response is adjusted by the protocol fee of 0.1 WETH:
//! Fee = 0.05 WETH + 0.1 WETH = 0.15 WETH
//! Executed = 0.95 WETH - 0.1 WETH = 0.85 WETH
//! 
//! BUY ORDER
//! Buying 1 WETH for at most `x` amount of USDC. Solvers respond with
//! Fee = 0.05 WETH
//! Executed = 1 WETH
//! 
//! This response is adjusted by the protocol fee of 0.1 WETH:
//! Fee = 0.05 WETH + 0.1 WETH = 0.15 WETH
//! Executed = 1 WETH

use {
    super::trade::{Fee, Fulfillment, InvalidExecutedAmount},
    crate::domain::{
        competition::{
            order,
            order::{FeePolicy, Side},
        },
        eth,
    },
};

impl Fulfillment {
    /// Applies the protocol fee to the existing fulfillment.
    pub fn with_protocol_fee(&self, prices: ClearingPrices) -> Result<Self, InvalidExecutedAmount> {
        let protocol_fee = self.protocol_fee(prices)?;

        // Increase the fee by the protocol fee
        let fee = match self.raw_fee() {
            Fee::Static => Fee::Static,
            Fee::Dynamic(fee) => Fee::Dynamic((fee.0 + protocol_fee).into()),
        };

        // Reduce the executed amount by the protocol fee. This is because solvers are
        // unaware of the protocol fee that driver introduces and they only account
        // for their own fee.
        let order = self.order().clone();
        let executed = match order.side {
            order::Side::Buy => self.executed(),
            order::Side::Sell => order::TargetAmount(
                self.executed()
                    .0
                    .checked_sub(protocol_fee)
                    .ok_or(InvalidExecutedAmount)?,
            ),
        };

        Fulfillment::new(order, executed, fee)
    }

    fn protocol_fee(&self, prices: ClearingPrices) -> Result<eth::U256, InvalidExecutedAmount> {
        let mut protocol_fee = eth::U256::zero();
        for fee_policy in self.order().fee_policies.iter() {
            match fee_policy {
                FeePolicy::PriceImprovement {
                    factor,
                    max_volume_factor,
                } => {
                    let price_improvement_fee = self
                        .price_improvement_fee(prices, *factor)
                        .ok_or(InvalidExecutedAmount)?;
                    let max_volume_fee = self
                        .volume_fee(prices, *max_volume_factor)
                        .ok_or(InvalidExecutedAmount)?;
                    // take the smaller of the two
                    protocol_fee = protocol_fee
                        .checked_add(std::cmp::min(price_improvement_fee, max_volume_fee))
                        .ok_or(InvalidExecutedAmount)?;
                }
                FeePolicy::Volume { factor } => {
                    let fee = self
                        .volume_fee(prices, *factor)
                        .ok_or(InvalidExecutedAmount)?;
                    protocol_fee = protocol_fee.checked_add(fee).ok_or(InvalidExecutedAmount)?;
                }
            }
        }
        Ok(protocol_fee)
    }

    fn price_improvement_fee(&self, prices: ClearingPrices, factor: f64) -> Option<eth::U256> {
        let sell_amount = self.order().sell.amount.0;
        let buy_amount = self.order().buy.amount.0;
        let executed = self.executed().0;
        let surplus_fee = match self.raw_fee() {
            Fee::Static => eth::U256::zero(),
            Fee::Dynamic(fee) => fee.0,
        };
        match self.order().side {
            Side::Buy => {
                // How much `sell_token` we need to sell to buy `executed` amount of `buy_token`
                let executed_sell_amount =
                    executed.checked_mul(prices.buy)?.checked_div(prices.sell)?;
                // Sell slightly more `sell_token` to capture the `surplus_fee`
                let executed_sell_amount_with_fee =
                    executed_sell_amount.checked_add(surplus_fee)?;
                // Scale to support partially fillable orders
                let limit_sell_amount =
                    sell_amount.checked_mul(executed)?.checked_div(buy_amount)?;
                // Remaining surplus after fees
                let surplus = limit_sell_amount
                    .checked_sub(executed_sell_amount_with_fee)
                    .unwrap_or(eth::U256::zero());
                Some(surplus.checked_mul(eth::U256::from_f64_lossy(factor * 100.))? / 100)
            }
            Side::Sell => {
                // How much `buy_token` we get for `executed` amount of `sell_token`
                let executed_buy_amount =
                    executed.checked_mul(prices.sell)?.checked_div(prices.buy)?;
                let executed_sell_amount_with_fee = executed.checked_add(surplus_fee)?;
                // Scale to support partially fillable orders
                let limit_buy_amount = buy_amount
                    .checked_mul(executed_sell_amount_with_fee)?
                    .checked_div(sell_amount)?;
                // Remaining surplus after fees
                let surplus = executed_buy_amount
                    .checked_sub(limit_buy_amount)
                    .unwrap_or(eth::U256::zero());
                let surplus_in_sell_token =
                    surplus.checked_mul(prices.buy)?.checked_div(prices.sell)?;
                Some(
                    surplus_in_sell_token.checked_mul(eth::U256::from_f64_lossy(factor * 100.))?
                        / 100,
                )
            }
        }
    }

    fn volume_fee(&self, prices: ClearingPrices, factor: f64) -> Option<eth::U256> {
        let executed = self.executed().0;
        let surplus_fee = match self.raw_fee() {
            Fee::Static => eth::U256::zero(),
            Fee::Dynamic(fee) => fee.0,
        };
        match self.order().side {
            Side::Buy => {
                // How much `sell_token` we need to sell to buy `executed` amount of `buy_token`
                let executed_sell_amount =
                    executed.checked_mul(prices.buy)?.checked_div(prices.sell)?;
                // Sell slightly more `sell_token` to capture the `surplus_fee`
                let executed_sell_amount_with_fee =
                    executed_sell_amount.checked_add(surplus_fee)?;
                Some(
                    executed_sell_amount_with_fee
                        .checked_mul(eth::U256::from_f64_lossy(factor * 100.))?
                        / 100,
                )
            }
            Side::Sell => {
                let executed_sell_amount_with_fee = executed.checked_add(surplus_fee)?;
                Some(
                    executed_sell_amount_with_fee
                        .checked_mul(eth::U256::from_f64_lossy(factor * 100.))?
                        / 100,
                )
            }
        }
    }
}

/// Uniform clearing prices at which the trade was executed.
#[derive(Debug, Clone, Copy)]
pub struct ClearingPrices {
    pub sell: eth::U256,
    pub buy: eth::U256,
}

mod tests {
    use {
        super::*,
        crate::{
            domain::{
                competition,
                competition::order::{
                    signature::Scheme,
                    AppData,
                    BuyTokenBalance,
                    FeePolicy,
                    SellTokenBalance,
                    Signature,
                    TargetAmount,
                },
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
        let prices = ClearingPrices {
            sell: eth::U256::from(913320970421237626580182u128),
            buy: eth::U256::from(4149866666666666668u128),
        };
        let executed = order::TargetAmount(4149866666666666668u128.into());
        let fee = Fee::Dynamic(order::SellAmount(16799999999999998u128.into()));
        let fulfillment = Fulfillment::new(order.clone(), executed, fee).unwrap();
        // fee does not contains protocol fee
        assert_eq!(
            fulfillment.fee(),
            order::SellAmount(16799999999999998u128.into())
        );
        // executed amount before protocol fee
        assert_eq!(fulfillment.executed(), executed);

        let fulfillment = fulfillment.with_protocol_fee(prices).unwrap();
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
    pub fn test_fulfillment_buy_limit_order_fok() {
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
        let prices = ClearingPrices {
            sell: eth::U256::from(7213317128720734077u128),
            buy: eth::U256::from(74745150907421124481191u128),
        };
        let executed = order::TargetAmount(170000000000000000u128.into());
        let fee = Fee::Dynamic(order::SellAmount(19868323826701104280u128.into()));
        let fulfillment = Fulfillment::new(order.clone(), executed, fee).unwrap();
        // fee does not contains protocol fee
        assert_eq!(
            fulfillment.fee(),
            order::SellAmount(19868323826701104280u128.into())
        );
        // executed amount before protocol fee
        assert_eq!(fulfillment.executed(), executed);

        let fulfillment = fulfillment.with_protocol_fee(prices).unwrap();
        // fee contains protocol fee
        assert_eq!(
            fulfillment.fee(),
            order::SellAmount((19868323826701104280u128 + 3684441086061450u128).into())
        );
        // executed amount same as before
        assert_eq!(fulfillment.executed(), executed);
    }

    #[test]
    fn test_fulfillment_sell_limit_order_partial() {
        // https://explorer.cow.fi/orders/0x1a146dba48512326c647aae1ce511206b373b151e1b9ada9772c313e7d24ec2e0960da039bb8151cacfef620476e8baf34bd95656594209e?tab=overview
        // 3 fullfillments
        //
        // 1. tx hash 0xbc95b97d09a62e6a68b15a8dfd4655a6e25d100ce0dd98a6a43e3b7eac9951cc
        //
        // https://production-6de61f.kb.eu-central-1.aws.cloud.es.io/app/discover#/doc/c0e240e0-d9b3-11ed-b0e6-e361adffce0b/cowlogs-prod-2023.12.26?id=W-uxp4wBlutGF6GyxkCq
        let order1 = competition::Order {
            uid: Default::default(),
            side: order::Side::Sell,
            buy: eth::Asset {
                token: eth::TokenAddress(eth::ContractAddress(
                    H160::from_str("0x70edf1c215d0ce69e7f16fd4e6276ba0d99d4de7").unwrap(),
                )),
                amount: eth::TokenAmount(136363636363636u128.into()),
            },
            sell: eth::Asset {
                token: eth::TokenAddress(eth::ContractAddress(
                    H160::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),
                )),
                amount: eth::TokenAmount(9000000000u128.into()),
            },
            kind: order::Kind::Limit,
            fee: Default::default(),
            fee_policies: vec![FeePolicy::PriceImprovement {
                factor: 0.5,
                max_volume_factor: 1.0,
            }],
            partial: order::Partial::Yes {
                available: TargetAmount(9000000000u128.into()),
            },
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

        let prices = ClearingPrices {
            sell: eth::U256::from(452471455796126723289489746u128),
            buy: eth::U256::from(29563373796548615411833u128),
        };
        let executed = order::TargetAmount(1746031488u128.into());
        let fee = Fee::Dynamic(order::SellAmount(11566733u128.into()));
        let fulfillment = Fulfillment::new(order1.clone(), executed, fee).unwrap();
        // fee does not contains protocol fee
        assert_eq!(fulfillment.fee(), order::SellAmount(11566733u128.into()));
        // executed amount before protocol fee
        assert_eq!(fulfillment.executed(), executed);

        let fulfillment = fulfillment.with_protocol_fee(prices).unwrap();
        // fee contains protocol fee
        assert_eq!(
            fulfillment.fee(),
            order::SellAmount((11566733u128 + 3037322u128).into())
        );
        // executed amount reduced by protocol fee
        assert_eq!(fulfillment.executed(), U256::from(1742994166u128).into()); // 1746031488 - 3037322

        // 2. tx hash 0x2f9b928182649aad2eaf04361fff1aff3cb8d37e4988c952aed49465eff01c9e
        //
        // https://production-6de61f.kb.eu-central-1.aws.cloud.es.io/app/discover#/doc/c0e240e0-d9b3-11ed-b0e6-e361adffce0b/cowlogs-prod-2023.12.26?id=uvXcp4wB4Ql8nk7aQgeZ

        let order2 = competition::Order {
            uid: Default::default(),
            side: order::Side::Sell,
            buy: eth::Asset {
                token: eth::TokenAddress(eth::ContractAddress(
                    H160::from_str("0x70edf1c215d0ce69e7f16fd4e6276ba0d99d4de7").unwrap(),
                )),
                amount: eth::TokenAmount(136363636363636u128.into()),
            },
            sell: eth::Asset {
                token: eth::TokenAddress(eth::ContractAddress(
                    H160::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),
                )),
                amount: eth::TokenAmount(9000000000u128.into()),
            },
            kind: order::Kind::Limit,
            fee: Default::default(),
            fee_policies: vec![FeePolicy::PriceImprovement {
                factor: 0.5,
                max_volume_factor: 1.0,
            }],
            partial: order::Partial::Yes {
                available: TargetAmount(7242401779u128.into()),
            },
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

        let prices = ClearingPrices {
            sell: eth::U256::from(49331008874302634851980418220032u128),
            buy: eth::U256::from(3204738565525085525012119552u128),
        };
        let executed = order::TargetAmount(2887238741u128.into());
        let fee = Fee::Dynamic(order::SellAmount(27827963u128.into()));
        let fulfillment = Fulfillment::new(order2.clone(), executed, fee).unwrap();
        // fee does not contains protocol fee
        assert_eq!(fulfillment.fee(), order::SellAmount(27827963u128.into()));
        // executed amount before protocol fee
        assert_eq!(fulfillment.executed(), executed);

        let fulfillment = fulfillment.with_protocol_fee(prices).unwrap();
        // fee contains protocol fee
        assert_eq!(
            fulfillment.fee(),
            order::SellAmount((27827963u128 + 8965365u128).into())
        );
        // executed amount reduced by protocol fee
        assert_eq!(fulfillment.executed(), U256::from(2878273376u128).into()); // 2887238741 - 8965365

        // 3. 0x813dab5983fd3643e1ce3e7efbdbfe1ca8c41419bcfaf1e898e067e37c455d75
        //
        // https://production-6de61f.kb.eu-central-1.aws.cloud.es.io/app/discover#/doc/c0e240e0-d9b3-11ed-b0e6-e361adffce0b/cowlogs-prod-2023.12.26?id=xPXdp4wB4Ql8nk7a8ert

        let order3 = competition::Order {
            uid: Default::default(),
            side: order::Side::Sell,
            buy: eth::Asset {
                token: eth::TokenAddress(eth::ContractAddress(
                    H160::from_str("0x70edf1c215d0ce69e7f16fd4e6276ba0d99d4de7").unwrap(),
                )),
                amount: eth::TokenAmount(136363636363636u128.into()),
            },
            sell: eth::Asset {
                token: eth::TokenAddress(eth::ContractAddress(
                    H160::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap(),
                )),
                amount: eth::TokenAmount(9000000000u128.into()),
            },
            kind: order::Kind::Limit,
            fee: Default::default(),
            fee_policies: vec![FeePolicy::PriceImprovement {
                factor: 0.5,
                max_volume_factor: 1.0,
            }],
            partial: order::Partial::Yes {
                available: TargetAmount(4327335075u128.into()),
            },
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

        let prices = ClearingPrices {
            sell: eth::U256::from(65841033847428u128),
            buy: eth::U256::from(4302554937u128),
        };
        let executed = order::TargetAmount(4302554937u128.into());
        let fee = Fee::Dynamic(order::SellAmount(24780138u128.into()));
        let fulfillment = Fulfillment::new(order3.clone(), executed, fee).unwrap();
        // fee does not contains protocol fee
        assert_eq!(fulfillment.fee(), order::SellAmount(24780138u128.into()));
        // executed amount before protocol fee
        assert_eq!(fulfillment.executed(), executed);

        let fulfillment = fulfillment.with_protocol_fee(prices).unwrap();
        // fee contains protocol fee
        assert_eq!(
            fulfillment.fee(),
            order::SellAmount((24780138u128 + 8996762u128).into())
        );
        // executed amount reduced by protocol fee
        assert_eq!(fulfillment.executed(), U256::from(4293558175u128).into()); // 4302554937 - 8996762
    }

    #[test]
    fn test_checked_sub() {
        assert_eq!(U256::from(1u128).checked_sub(U256::from(2u128)), None);
        assert_eq!(
            U256::from(2u128).checked_sub(U256::from(1u128)),
            Some(U256::from(1u128))
        );
    }
}
