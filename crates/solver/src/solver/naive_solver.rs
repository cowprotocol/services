mod multi_order_solver;

use {
    crate::{
        liquidity::{
            slippage::{SlippageCalculator, SlippageContext},
            ConstantProductOrder,
            LimitOrder,
            Liquidity,
        },
        settlement::Settlement,
        solver::{Auction, Solver},
    },
    anyhow::Result,
    ethcontract::Account,
    model::TokenPair,
    std::collections::HashMap,
};

pub struct NaiveSolver {
    account: Account,
    slippage_calculator: SlippageCalculator,
}

impl NaiveSolver {
    pub fn new(account: Account, slippage_calculator: SlippageCalculator) -> Self {
        Self {
            account,
            slippage_calculator,
        }
    }
}

#[async_trait::async_trait]
impl Solver for NaiveSolver {
    async fn solve(
        &self,
        Auction {
            orders,
            liquidity,
            external_prices,
            ..
        }: Auction,
    ) -> Result<Vec<Settlement>> {
        let slippage = self.slippage_calculator.context(&external_prices);
        let uniswaps = extract_deepest_amm_liquidity(&liquidity);
        Ok(settle(slippage, orders, uniswaps))
    }

    fn account(&self) -> &Account {
        &self.account
    }

    fn name(&self) -> &'static str {
        "NaiveSolver"
    }
}

fn settle(
    slippage: SlippageContext,
    orders: Vec<LimitOrder>,
    uniswaps: HashMap<TokenPair, ConstantProductOrder>,
) -> Vec<Settlement> {
    // The multi order solver matches as many orders as possible together with one
    // uniswap pool. Settlements between different token pairs are thus
    // independent.
    organize_orders_by_token_pair(orders)
        .into_iter()
        .filter_map(|(pair, orders)| settle_pair(&slippage, pair, orders, &uniswaps))
        .collect()
}

fn settle_pair(
    slippage: &SlippageContext,
    pair: TokenPair,
    orders: Vec<LimitOrder>,
    uniswaps: &HashMap<TokenPair, ConstantProductOrder>,
) -> Option<Settlement> {
    if orders.iter().all(|order| order.is_liquidity_order()) {
        tracing::debug!(?pair, "no user orders");
        return None;
    }
    let uniswap = match uniswaps.get(&pair) {
        Some(uniswap) => uniswap,
        None => {
            tracing::debug!(?pair, "no AMM");
            return None;
        }
    };
    multi_order_solver::solve(slippage, orders.into_iter(), uniswap)
}

fn organize_orders_by_token_pair(orders: Vec<LimitOrder>) -> HashMap<TokenPair, Vec<LimitOrder>> {
    let mut result = HashMap::<_, Vec<LimitOrder>>::new();
    for (order, token_pair) in orders.into_iter().filter(usable_order).filter_map(|order| {
        let pair = TokenPair::new(order.buy_token, order.sell_token)?;
        Some((order, pair))
    }) {
        result.entry(token_pair).or_default().push(order);
    }
    result
}

fn usable_order(order: &LimitOrder) -> bool {
    !order.sell_amount.is_zero() && !order.buy_amount.is_zero()
}

fn extract_deepest_amm_liquidity(
    liquidity: &[Liquidity],
) -> HashMap<TokenPair, ConstantProductOrder> {
    let mut result = HashMap::new();
    for liquidity in liquidity {
        match liquidity {
            Liquidity::ConstantProduct(order) => {
                let deepest_so_far = result.entry(order.tokens).or_insert_with(|| order.clone());
                if deepest_so_far.constant_product() < order.constant_product() {
                    result.insert(order.tokens, order.clone());
                }
            }
            _ => continue,
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::liquidity::{
            order_converter::OrderConverter,
            tests::CapturingSettlementHandler,
            LimitOrderId,
            LiquidityOrderId,
        },
        ethcontract::{H160, U256},
        maplit::hashmap,
        model::order::{
            LimitOrderClass,
            Order,
            OrderClass,
            OrderData,
            OrderKind,
            OrderMetadata,
            OrderUid,
            BUY_ETH_ADDRESS,
        },
        num::rational::Ratio,
        shared::addr,
    };

    #[test]
    fn test_extract_deepest_amm_liquidity() {
        let token_pair =
            TokenPair::new(H160::from_low_u64_be(0), H160::from_low_u64_be(1)).unwrap();
        let unrelated_token_pair =
            TokenPair::new(H160::from_low_u64_be(2), H160::from_low_u64_be(3)).unwrap();
        let handler = CapturingSettlementHandler::arc();
        let liquidity = vec![
            // Deep pool
            ConstantProductOrder {
                address: H160::from_low_u64_be(1),
                tokens: token_pair,
                reserves: (10_000_000, 10_000_000),
                fee: Ratio::new(3, 1000),
                settlement_handling: handler.clone(),
            },
            // Shallow pool
            ConstantProductOrder {
                address: H160::from_low_u64_be(2),
                tokens: token_pair,
                reserves: (100, 100),
                fee: Ratio::new(3, 1000),
                settlement_handling: handler.clone(),
            },
            // unrelated pool
            ConstantProductOrder {
                address: H160::from_low_u64_be(3),
                tokens: unrelated_token_pair,
                reserves: (10_000_000, 10_000_000),
                fee: Ratio::new(3, 1000),
                settlement_handling: handler,
            },
        ];
        let result = extract_deepest_amm_liquidity(
            &liquidity
                .iter()
                .cloned()
                .map(Liquidity::ConstantProduct)
                .collect::<Vec<_>>(),
        );
        assert_eq!(result[&token_pair].reserves, liquidity[0].reserves);
        assert_eq!(
            result[&unrelated_token_pair].reserves,
            liquidity[2].reserves
        );
    }

    #[test]
    fn respects_liquidity_order_limit_price() {
        // We have a "perfect CoW" where the spot price of the Uniswap pool does
        // not satisfy the liquidity order's limit price. Hence, there should be
        // NO solutions for this auction.
        // Test case recovered from the following settlement where a user order
        // was settled directly with a liquidity order, and we paid out WAY more
        // than the market maker order provided:
        // <https://etherscan.io/tx/0x02e858f10c5b3ab41031421f6634dc0c7799c9aec65160f516af53673dafa92c>

        let orders = vec![
            LimitOrder::from(Order {
                data: OrderData {
                    sell_token: addr!("d533a949740bb3306d119cc777fa900ba034cd52"),
                    buy_token: addr!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
                    sell_amount: 995952859647034749952_u128.into(),
                    buy_amount: 2461209365_u128.into(),
                    kind: OrderKind::Sell,
                    ..Default::default()
                },
                ..Default::default()
            }),
            LimitOrder {
                id: LimitOrderId::Liquidity(LiquidityOrderId::Protocol(OrderUid::from_integer(1))),
                ..LimitOrder::from(Order {
                    data: OrderData {
                        sell_token: addr!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
                        buy_token: addr!("d533a949740bb3306d119cc777fa900ba034cd52"),
                        sell_amount: 2469904889_u128.into(),
                        buy_amount: 995952859647034749952_u128.into(),
                        kind: OrderKind::Buy,
                        ..Default::default()
                    },
                    ..Default::default()
                })
            },
        ];

        let tokens = TokenPair::new(
            addr!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
            addr!("d533a949740bb3306d119cc777fa900ba034cd52"),
        )
        .unwrap();
        let liquidity = hashmap! {
            tokens => ConstantProductOrder {
                address: H160::from_low_u64_be(1),
                tokens,
                reserves: (58360914, 17856367410307570970),
                fee: Ratio::new(3, 1000),
                settlement_handling: CapturingSettlementHandler::arc(),
            },
        };

        assert!(settle(SlippageContext::default(), orders, liquidity).is_empty());
    }

    #[test]
    fn requires_at_least_one_non_liquidity_order() {
        let orders = vec![
            LimitOrder::from(Order {
                data: OrderData {
                    sell_token: H160([1; 20]),
                    buy_token: H160([2; 20]),
                    sell_amount: 1_000_000_000_u128.into(),
                    buy_amount: 900_000_000_u128.into(),
                    kind: OrderKind::Sell,
                    ..Default::default()
                },
                metadata: OrderMetadata {
                    class: OrderClass::Liquidity,
                    ..Default::default()
                },
                ..Default::default()
            }),
            LimitOrder::from(Order {
                data: OrderData {
                    sell_token: H160([1; 20]),
                    buy_token: H160([2; 20]),
                    sell_amount: 1_000_000_000_u128.into(),
                    buy_amount: 900_000_000_u128.into(),
                    kind: OrderKind::Sell,
                    ..Default::default()
                },
                metadata: OrderMetadata {
                    class: OrderClass::Liquidity,
                    ..Default::default()
                },
                ..Default::default()
            }),
        ];

        let tokens = TokenPair::new(H160([1; 20]), H160([2; 20])).unwrap();
        let liquidity = hashmap! {
            tokens => ConstantProductOrder {
                address: H160::from_low_u64_be(1),
                tokens,
                reserves: (1_000_000_000_000_000_000, 1_000_000_000_000_000_000),
                fee: Ratio::new(3, 1000),
                settlement_handling: CapturingSettlementHandler::arc(),
            },
        };

        assert!(settle(SlippageContext::default(), orders, liquidity).is_empty());
    }

    #[test]
    fn works_with_eth_liquidity_orders() {
        let native_token = H160([1; 20]);
        let converter = OrderConverter::test(native_token);

        let orders = vec![
            converter
                .normalize_limit_order(Order {
                    data: OrderData {
                        sell_token: native_token,
                        buy_token: H160([2; 20]),
                        sell_amount: 1_000_000_000_u128.into(),
                        buy_amount: 900_000_000_u128.into(),
                        kind: OrderKind::Sell,
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .unwrap(),
            converter
                .normalize_limit_order(Order {
                    data: OrderData {
                        sell_token: H160([2; 20]),
                        buy_token: BUY_ETH_ADDRESS,
                        sell_amount: 1_000_000_000_u128.into(),
                        buy_amount: 900_000_000_u128.into(),
                        kind: OrderKind::Sell,
                        ..Default::default()
                    },
                    metadata: OrderMetadata {
                        class: OrderClass::Liquidity,
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .unwrap(),
        ];

        let tokens = TokenPair::new(native_token, H160([2; 20])).unwrap();
        let liquidity = hashmap! {
            tokens => ConstantProductOrder {
                address: H160::from_low_u64_be(1),
                tokens,
                reserves: (1_000_000_000_000_000_000, 1_000_000_000_000_000_000),
                fee: Ratio::new(3, 1000),
                settlement_handling: CapturingSettlementHandler::arc(),
            },
        };

        assert_eq!(
            settle(SlippageContext::default(), orders, liquidity).len(),
            1
        );
    }

    #[test]
    fn respects_limit_order_price_for_limit_orders_after_surplus_fee() {
        // This order can be settled before but not after surplus fees
        let orders = vec![LimitOrder::from(Order {
            data: OrderData {
                sell_token: addr!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
                buy_token: addr!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
                sell_amount: (22397494).into(),
                buy_amount: 18477932550000000u128.into(),
                kind: OrderKind::Sell,
                ..Default::default()
            },
            metadata: OrderMetadata {
                class: OrderClass::Limit(LimitOrderClass {
                    surplus_fee: Some(1675785.into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        })];

        let tokens = TokenPair::new(
            addr!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
            addr!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
        )
        .unwrap();
        let liquidity = hashmap! {
            tokens => ConstantProductOrder {
                address: H160::from_low_u64_be(1),
                tokens,
                reserves: (36338096110368, 30072348537379906026018),
                fee: Ratio::new(3, 1000),
                settlement_handling: CapturingSettlementHandler::arc(),
            },
        };

        assert!(settle(SlippageContext::default(), orders, liquidity).is_empty());
    }

    #[test]
    fn does_not_swap_more_than_reserves() {
        let usdc = addr!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
        let crv = addr!("D533a949740bb3306d119CC777fa900bA034cd52");

        let orders = vec![
            LimitOrder::from(Order {
                data: OrderData {
                    sell_token: crv,
                    buy_token: usdc,
                    sell_amount: 2161740107040163317224_u128.into(),
                    buy_amount: 2146544862_u128.into(),
                    fee_amount: 6177386651128093696_u128.into(),
                    kind: OrderKind::Sell,
                    ..Default::default()
                },
                metadata: OrderMetadata {
                    class: OrderClass::Liquidity,
                    ..Default::default()
                },
                ..Default::default()
            }),
            LimitOrder::from(Order {
                data: OrderData {
                    sell_token: usdc,
                    buy_token: crv,
                    sell_amount: 500000000_u128.into(),
                    buy_amount: 1428571428571428571428_u128.into(),
                    kind: OrderKind::Sell,
                    ..Default::default()
                },
                metadata: OrderMetadata {
                    class: OrderClass::Limit(LimitOrderClass {
                        surplus_fee: Some(4834012_u128.into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                ..Default::default()
            }),
        ];

        let amm_handler = CapturingSettlementHandler::arc();
        let tokens = TokenPair::new(usdc, crv).unwrap();
        let liquidity = hashmap! {
            tokens => ConstantProductOrder {
                address: addr!("210a97ba874a8e279c95b350ae8ba143a143c159"),
                tokens,
                reserves: (32275540, 33308141034569852391),
                fee: Ratio::new(3, 1000),
                settlement_handling: amm_handler.clone(),
            },
        };

        settle(SlippageContext::default(), orders, liquidity);
        let (out_token, out_amount) = amm_handler.calls.lock().unwrap()[0].output;
        assert_eq!(out_token, usdc);
        assert!(out_amount < U256::from(32275540_u128));
    }
}
