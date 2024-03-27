use {
    crate::{
        boundary::liquidity::constant_product::to_boundary_pool,
        domain::{
            auction,
            eth,
            liquidity,
            order::{self, Side},
            solution::{self, Fee},
        },
    },
    ethereum_types::H160,
    itertools::Itertools,
    model::order::{Order, OrderClass, OrderData, OrderKind, OrderMetadata, OrderUid},
    num::{BigRational, One},
    shared::{
        external_prices::ExternalPrices,
        price_estimation::gas::{ERC20_TRANSFER, INITIALIZATION_COST, SETTLEMENT, TRADE},
    },
    solver::{
        liquidity::{
            slippage::{SlippageCalculator, SlippageContext},
            AmmOrderExecution,
            ConstantProductOrder,
            Exchange,
            LimitOrder,
            LimitOrderExecution,
            LimitOrderId,
            LiquidityOrderId,
            SettlementHandling,
        },
        settlement::SettlementEncoder,
        solver::naive_solver::multi_order_solver,
    },
    std::sync::{Arc, Mutex},
};

pub fn solve(
    orders: &[&order::Order],
    liquidity: &liquidity::Liquidity,
    gas_price: auction::GasPrice,
    tokens: &auction::Tokens,
) -> Option<solution::Solution> {
    let pool = match &liquidity.state {
        liquidity::State::ConstantProduct(pool) => pool,
        _ => return None,
    };

    // Note that the `order::Order` -> `boundary::LimitOrder` mapping here is
    // not exact. Among other things, the signature and various signed order
    // fields are missing from the `order::Order` data that the solver engines
    // have access to. This means that the naive solver in the `solver` crate
    // will encode "incorrect" settlements. This is fine, since we give it just
    // enough data to compute the correct swapped orders and the swap amounts
    // which is what the naive solver in the `solvers` crate cares about. The
    // `driver` is then responsible for encoding the solution into a valid
    // settlement transaction anyway.
    let boundary_orders = orders
        .iter()
        .map(|order| LimitOrder {
            id: match order.class {
                order::Class::Market => LimitOrderId::Market(OrderUid(order.uid.0)),
                order::Class::Limit => LimitOrderId::Limit(OrderUid(order.uid.0)),
                order::Class::Liquidity => {
                    LimitOrderId::Liquidity(LiquidityOrderId::Protocol(OrderUid(order.uid.0)))
                }
            },
            sell_token: order.sell.token.0,
            buy_token: order.buy.token.0,
            sell_amount: order.sell.amount,
            buy_amount: order.buy.amount,
            kind: match order.side {
                order::Side::Buy => OrderKind::Buy,
                order::Side::Sell => OrderKind::Sell,
            },
            partially_fillable: order.partially_fillable,
            user_fee: order.fee().amount,
            settlement_handling: Arc::new(OrderHandler {
                order: Order {
                    metadata: OrderMetadata {
                        uid: OrderUid(order.uid.0),
                        class: match order.class {
                            order::Class::Market => OrderClass::Market,
                            order::Class::Limit => OrderClass::Limit,
                            order::Class::Liquidity => OrderClass::Liquidity,
                        },
                        solver_fee: order.fee().amount,
                        ..Default::default()
                    },
                    data: OrderData {
                        sell_token: order.sell.token.0,
                        buy_token: order.buy.token.0,
                        sell_amount: order.sell.amount,
                        buy_amount: order.buy.amount,
                        fee_amount: order.fee().amount,
                        kind: match order.side {
                            order::Side::Buy => OrderKind::Buy,
                            order::Side::Sell => OrderKind::Sell,
                        },
                        partially_fillable: order.partially_fillable,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            }),
            exchange: Exchange::GnosisProtocol,
        })
        .collect_vec();

    let slippage = Slippage::new(pool.tokens());
    let pool_handler = Arc::new(PoolHandler::default());
    let boundary_pool = ConstantProductOrder::for_pool(
        to_boundary_pool(liquidity.address, pool)?,
        pool_handler.clone(),
    );

    let boundary_solution =
        multi_order_solver::solve(&slippage.context(), boundary_orders, &boundary_pool)?;

    let swap = pool_handler.swap.lock().unwrap().take();

    // Evenly divide settlement execution cost across all settled orders that need a
    // fee. Does not take pre- and post-interactions into account.
    let eth_per_order = {
        let num_trades = boundary_solution
            .trades()
            .filter(|t| t.order.solver_determines_fee())
            .count() as u64;
        let gas_interaction: u64 = swap.iter().map(|_| liquidity.gas.0.as_u64()).sum();
        let total_gas = INITIALIZATION_COST
            + SETTLEMENT
            + num_trades * TRADE
            + 2 * num_trades * ERC20_TRANSFER
            + gas_interaction;
        let gas_per_trade = eth::U256::from(total_gas.checked_div(num_trades).unwrap_or_default());
        eth::Ether(gas_per_trade.checked_mul(gas_price.0 .0)?)
    };

    Some(solution::Solution {
        id: Default::default(),
        prices: solution::ClearingPrices::new(
            boundary_solution
                .clearing_prices()
                .iter()
                .map(|(token, price)| (eth::TokenAddress(*token), *price)),
        ),
        trades: boundary_solution
            .traded_orders()
            .map(|order| {
                let order = orders
                    .iter()
                    .copied()
                    .find(|o| o.uid.0 == order.metadata.uid.0)?
                    .clone();

                // partial fills not supported so always execute the full amount
                let executed = match order.side {
                    Side::Buy => order.buy.amount,
                    Side::Sell => order.sell.amount,
                };

                let fee = if order.solver_determines_fee() {
                    let sell_price = tokens.0.get(&order.sell.token)?.reference_price?;
                    Fee::Surplus(sell_price.ether_value(eth_per_order)?)
                } else {
                    Fee::Protocol
                };

                Some(solution::Trade::Fulfillment(solution::Fulfillment::new(
                    order, executed, fee,
                )?))
            })
            .collect::<Option<Vec<_>>>()?,
        // We can skip computing a gas estimate here because that is only used by the protocol
        // when quoting trades. And because the naive solver is not able to solve single order
        // auctions (which is how we model a /quote request) it can not be used for
        // quoting anyway.
        gas: None,
        interactions: swap
            .into_iter()
            .map(|(input, output)| {
                solution::Interaction::Liquidity(solution::LiquidityInteraction {
                    liquidity: liquidity.clone(),
                    input,
                    output,
                    internalize: false,
                })
            })
            .collect(),
        score: Default::default(),
    })
}

// Beyond this point is... well... nameless and boundless chaos. The
// unfathomable horrors that follow are not for the faint of heart!
//
// Joking aside, the existing naive solver implementation is tightly coupled
// with the `Settlement` and `SettlementEncoder` types in the `solver` crate.
// This means that there is no convenient way to say: "please compute a solution
// given this list of orders and constant product pool" without it creating a
// full settlement for encoding. In order to adapt that API into something that
// is useful in this boundary module, we create a fake slippage context that
// applies 0 slippage (so that we can recover the exact executed amounts from
// the constant product pool) and we create capturing settlement handler
// implementations that record the swap that gets added to each settlement so
// that it can be recovered later to build a solution.

struct Slippage {
    calculator: SlippageCalculator,
    prices: ExternalPrices,
}

impl Slippage {
    fn new(tokens: liquidity::TokenPair) -> Self {
        // We don't actually want to include slippage yet. This is because the
        // Naive solver encodes liquidity interactions and the driver is
        // responsible for applying slippage to those. Create a dummy slippage
        // context for use with the legacy Naive solver.
        let (token0, token1) = tokens.get();
        Self {
            calculator: SlippageCalculator::from_bps(0, None),
            prices: ExternalPrices::new(
                H160::default(),
                [
                    (token0.0, BigRational::one()),
                    (token1.0, BigRational::one()),
                ]
                .into_iter()
                .collect(),
            )
            .unwrap(),
        }
    }

    fn context(&self) -> SlippageContext {
        self.calculator.context(&self.prices)
    }
}

struct OrderHandler {
    order: Order,
}

impl SettlementHandling<LimitOrder> for OrderHandler {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn encode(
        &self,
        execution: LimitOrderExecution,
        encoder: &mut SettlementEncoder,
    ) -> anyhow::Result<()> {
        encoder.add_trade(self.order.clone(), execution.filled, execution.fee)?;
        Ok(())
    }
}

#[derive(Default)]
struct PoolHandler {
    swap: Mutex<Option<(eth::Asset, eth::Asset)>>,
}

impl SettlementHandling<ConstantProductOrder> for PoolHandler {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn encode(
        &self,
        execution: AmmOrderExecution,
        _: &mut SettlementEncoder,
    ) -> anyhow::Result<()> {
        *self.swap.lock().unwrap() = Some((
            eth::Asset {
                token: eth::TokenAddress(execution.input_max.token),
                amount: execution.input_max.amount,
            },
            eth::Asset {
                token: eth::TokenAddress(execution.output.token),
                amount: execution.output.amount,
            },
        ));
        Ok(())
    }
}
