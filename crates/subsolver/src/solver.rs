use {
    crate::{config::SolverConfig, orderbook},
    alloy_primitives::U256,
    solvers::domain::{auction, eth, liquidity as domain_liquidity, order, solution, solver},
};

pub struct BaselineWrapper {
    solver: solver::Baseline,
}

impl BaselineWrapper {
    pub async fn new(config: &SolverConfig, weth: alloy_primitives::Address) -> Self {
        let solver_config = solver::Config {
            weth: eth::WethAddress(weth),
            base_tokens: config
                .base_tokens
                .iter()
                .map(|a| eth::TokenAddress(*a))
                .collect(),
            max_hops: config.max_hops,
            max_partial_attempts: config.max_partial_attempts,
            solution_gas_offset: eth::SignedGas::default(),
            native_token_price_estimation_amount: config.native_token_price_estimation_amount,
            uni_v3_node_url: None,
        };
        Self {
            solver: solver::Baseline::new(solver_config).await,
        }
    }

    pub async fn solve(
        &self,
        orders: &[orderbook::Order],
        pools: &[domain_liquidity::Liquidity],
        gas_price: U256,
        prices: &std::collections::BTreeMap<alloy_primitives::Address, U256>,
    ) -> Vec<solution::Solution> {
        let domain_orders: Vec<order::Order> = orders
            .iter()
            .filter_map(|o| {
                let uid: [u8; 56] = o.uid.clone().try_into().ok()?;
                Some(order::Order {
                    uid: order::Uid(uid),
                    sell: eth::Asset {
                        token: eth::TokenAddress(o.sell_token),
                        amount: o.sell_amount,
                    },
                    buy: eth::Asset {
                        token: eth::TokenAddress(o.buy_token),
                        amount: o.buy_amount,
                    },
                    side: match o.kind {
                        orderbook::OrderKind::Buy => order::Side::Buy,
                        orderbook::OrderKind::Sell => order::Side::Sell,
                    },
                    class: match o.class {
                        orderbook::OrderClass::Market => order::Class::Market,
                        orderbook::OrderClass::Limit | orderbook::OrderClass::Liquidity => {
                            order::Class::Limit
                        }
                    },
                    partially_fillable: o.partially_fillable,
                    flashloan_hint: None,
                    wrappers: vec![],
                })
            })
            .collect();

        let tokens = auction::Tokens(
            prices
                .iter()
                .map(|(addr, price)| {
                    (
                        eth::TokenAddress(*addr),
                        auction::Token {
                            decimals: None,
                            symbol: None,
                            reference_price: Some(auction::Price(eth::Ether(*price))),
                            available_balance: U256::ZERO,
                            trusted: false,
                        },
                    )
                })
                .collect(),
        );

        let auction = auction::Auction {
            id: auction::Id::Solve(0),
            tokens,
            orders: domain_orders,
            liquidity: pools.to_vec(),
            gas_price: auction::GasPrice(eth::Ether(gas_price)),
            deadline: auction::Deadline(chrono::Utc::now() + chrono::Duration::seconds(10)),
        };

        self.solver.solve(auction).await
    }
}
