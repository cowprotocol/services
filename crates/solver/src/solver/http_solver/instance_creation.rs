use super::{
    buffers::{BufferRetrievalError, BufferRetrieving},
    settlement::SettlementContext,
};
use crate::{
    liquidity::{Exchange, LimitOrder, Liquidity},
    settlement::external_prices::ExternalPrices,
};
use anyhow::{Context, Result};
use ethcontract::{errors::ExecutionError, U256};
use itertools::{Either, Itertools as _};
use maplit::{btreemap, hashset};
use model::{auction::AuctionId, order::OrderKind};
use num::{BigInt, BigRational};
use primitive_types::H160;
use shared::{
    http_solver::{gas_model::GasModel, model::*},
    sources::balancer_v2::pools::common::compute_scaling_rate,
    token_info::{TokenInfo, TokenInfoFetching},
    token_list::AutoUpdatingTokenList,
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    iter::FromIterator as _,
    sync::Arc,
};

pub struct Instances {
    pub plain: BatchAuctionModel,
    pub filtered: BatchAuctionModel,
    pub context: SettlementContext,
}

pub struct InstanceCreator {
    pub native_token: H160,
    pub token_info_fetcher: Arc<dyn TokenInfoFetching>,
    pub buffer_retriever: Arc<dyn BufferRetrieving>,
    pub market_makable_token_list: AutoUpdatingTokenList,
    pub environment_metadata: String,
}

impl InstanceCreator {
    #[allow(clippy::too_many_arguments)]
    pub async fn prepare_instances(
        &self,
        auction_id: AuctionId,
        run_id: u64,
        mut orders: Vec<LimitOrder>,
        liquidity: Vec<Liquidity>,
        gas_price: f64,
        external_prices: &ExternalPrices,
    ) -> Instances {
        // The HTTP solver interface expects liquidity limit orders (like 0x
        // limit orders) to be added to the `orders` models and NOT the
        // `liquidity` models. Split the two here to avoid indexing errors
        // later on.
        let (limit_orders, amms): (Vec<_>, Vec<_>) =
            liquidity
                .into_iter()
                .partition_map(|liquidity| match liquidity {
                    Liquidity::LimitOrder(limit_order) => Either::Left(limit_order),
                    amm => Either::Right(amm),
                });
        orders.extend(limit_orders);

        let market_makable_token_list = self.market_makable_token_list.addresses();

        let tokens = map_tokens_for_solver(&orders, &amms, &market_makable_token_list);
        let (token_infos, buffers_result) = futures::join!(
            shared::measure_time(
                self.token_info_fetcher.get_token_infos(tokens.as_slice()),
                |duration| tracing::debug!("get_token_infos took {} s", duration.as_secs_f32()),
            ),
            shared::measure_time(
                self.buffer_retriever.get_buffers(tokens.as_slice()),
                |duration| tracing::debug!("get_buffers took {} s", duration.as_secs_f32()),
            ),
        );

        let buffers: HashMap<_, _> = buffers_result
            .into_iter()
            .filter_map(|(token, buffer)| match buffer {
                Err(BufferRetrievalError::Erc20(err)) if is_transaction_failure(&err.inner) => {
                    tracing::debug!(
                        "Failed to fetch buffers for token {} with transaction failure {}",
                        token,
                        err
                    );
                    None
                }
                Err(err) => {
                    tracing::error!(
                        "Failed to fetch buffers contract balance for token {} with error {:?}",
                        token,
                        err
                    );
                    None
                }
                Ok(b) => Some((token, b)),
            })
            .collect();

        // We are guaranteed to have price estimates for all tokens that are relevant to the
        // objective value by the driver. It is possible that we have AMM pools that contain tokens
        // that are not any order's tokens. We used to fetch these extra prices but it would often
        // slow down the solver and the solver can estimate them on its own.
        let price_estimates = external_prices.into_http_solver_prices();

        let gas_model = GasModel {
            native_token: self.native_token,
            gas_price,
        };

        // Some solvers require that there are no isolated islands of orders whose tokens are
        // unconnected to the native token.
        let fee_connected_tokens: HashSet<H160> =
            compute_fee_connected_tokens(&amms, self.native_token);
        let filtered_order_models = order_models(&orders, &fee_connected_tokens, &gas_model);

        let tokens: HashSet<H160> = tokens.into_iter().collect();
        let order_models = order_models(&orders, &tokens, &gas_model);

        let token_models = token_models(
            &token_infos,
            &price_estimates,
            &buffers,
            &gas_model,
            &market_makable_token_list,
        );

        let amm_models = amm_models(&amms, &gas_model);

        let model = BatchAuctionModel {
            tokens: token_models,
            orders: order_models,
            amms: amm_models,
            metadata: Some(MetadataModel {
                environment: Some(self.environment_metadata.clone()),
                auction_id: Some(auction_id),
                run_id: Some(run_id),
                gas_price: Some(gas_price),
                native_token: Some(self.native_token),
            }),
        };

        let mut filtered_model = model.clone();
        filtered_model.orders = filtered_order_models;

        let context = SettlementContext {
            orders,
            liquidity: amms,
        };

        Instances {
            plain: model,
            filtered: filtered_model,
            context,
        }
    }
}

fn map_tokens_for_solver(
    orders: &[LimitOrder],
    liquidity: &[Liquidity],
    market_makable_token_list: &HashSet<H160>,
) -> Vec<H160> {
    let mut token_set = HashSet::new();
    token_set.extend(
        orders
            .iter()
            .flat_map(|order| [order.sell_token, order.buy_token]),
    );
    for liquidity in liquidity.iter() {
        match liquidity {
            Liquidity::ConstantProduct(amm) => token_set.extend(amm.tokens),
            Liquidity::BalancerWeighted(amm) => token_set.extend(amm.reserves.keys()),
            Liquidity::BalancerStable(amm) => token_set.extend(amm.reserves.keys()),
            Liquidity::LimitOrder(_) => panic!("limit orders are expected to be filtered out"),
            Liquidity::Concentrated(amm) => token_set.extend(amm.tokens),
        }
    }
    token_set.extend(market_makable_token_list);

    Vec::from_iter(token_set)
}

fn order_fee(order: &LimitOrder) -> TokenAmount {
    let amount = match order.is_liquidity_order() {
        true => order.unscaled_subsidized_fee,
        false => order.scaled_unsubsidized_fee,
    };
    TokenAmount {
        amount,
        token: order.sell_token,
    }
}

fn token_models(
    token_infos: &HashMap<H160, TokenInfo>,
    price_estimates: &HashMap<H160, f64>,
    buffers: &HashMap<H160, U256>,
    gas_model: &GasModel,
    market_makable_token_list: &HashSet<H160>,
) -> BTreeMap<H160, TokenInfoModel> {
    token_infos
        .iter()
        .map(|(address, token_info)| {
            let external_price = match price_estimates.get(address).copied() {
                Some(price) if price.is_finite() => Some(price),
                _ => None,
            };
            (
                *address,
                TokenInfoModel {
                    decimals: token_info.decimals,
                    alias: token_info.symbol.clone(),
                    external_price,
                    normalize_priority: Some(u64::from(&gas_model.native_token == address)),
                    internal_buffer: buffers.get(address).copied(),
                    accepted_for_internalization: market_makable_token_list.contains(address),
                },
            )
        })
        .collect()
}

fn order_models(
    orders: &[LimitOrder],
    fee_connected_tokens: &HashSet<H160>,
    gas_model: &GasModel,
) -> BTreeMap<usize, OrderModel> {
    orders
        .iter()
        .enumerate()
        .filter_map(|(index, order)| {
            if ![order.sell_token, order.buy_token]
                .iter()
                .any(|token| fee_connected_tokens.contains(token))
            {
                return None;
            }

            let cost = match order.exchange {
                Exchange::GnosisProtocol => gas_model.gp_order_cost(),
                Exchange::ZeroEx => gas_model.zeroex_order_cost(),
            };

            Some((
                index,
                OrderModel {
                    id: order.id.order_uid(),
                    sell_token: order.sell_token,
                    buy_token: order.buy_token,
                    sell_amount: order.sell_amount,
                    buy_amount: order.buy_amount,
                    allow_partial_fill: order.partially_fillable,
                    is_sell_order: matches!(order.kind, OrderKind::Sell),
                    fee: order_fee(order),
                    cost,
                    is_liquidity_order: order.is_liquidity_order(),
                    mandatory: false,
                    has_atomic_execution: !matches!(order.exchange, Exchange::GnosisProtocol),
                    reward: order.reward,
                    is_mature: order.is_mature,
                },
            ))
        })
        .collect()
}

fn amm_models(liquidity: &[Liquidity], gas_model: &GasModel) -> BTreeMap<H160, AmmModel> {
    liquidity
        .iter()
        .map(|liquidity| -> Result<_> {
            Ok(match liquidity {
                Liquidity::ConstantProduct(amm) => AmmModel {
                    parameters: AmmParameters::ConstantProduct(ConstantProductPoolParameters {
                        reserves: btreemap! {
                            amm.tokens.get().0 => amm.reserves.0.into(),
                            amm.tokens.get().1 => amm.reserves.1.into(),
                        },
                    }),
                    fee: BigRational::new(
                        BigInt::from(*amm.fee.numer()),
                        BigInt::from(*amm.fee.denom()),
                    ),
                    cost: gas_model.uniswap_cost(),
                    mandatory: false,
                    address: amm.address,
                },
                Liquidity::BalancerWeighted(amm) => AmmModel {
                    parameters: AmmParameters::WeightedProduct(WeightedProductPoolParameters {
                        reserves: amm
                            .reserves
                            .iter()
                            .map(|(token, state)| {
                                (
                                    *token,
                                    WeightedPoolTokenData {
                                        balance: state.common.balance,
                                        weight: BigRational::from(state.weight),
                                    },
                                )
                            })
                            .collect(),
                    }),
                    fee: amm.fee.into(),
                    cost: gas_model.balancer_cost(),
                    mandatory: false,
                    address: amm.address,
                },
                Liquidity::BalancerStable(amm) => AmmModel {
                    parameters: AmmParameters::Stable(StablePoolParameters {
                        reserves: amm
                            .reserves
                            .iter()
                            .map(|(token, state)| (*token, state.balance))
                            .collect(),
                        scaling_rates: amm
                            .reserves
                            .iter()
                            .map(|(token, state)| {
                                Ok((*token, compute_scaling_rate(state.scaling_exponent)?))
                            })
                            .collect::<Result<_>>()
                            .with_context(|| {
                                format!("error converting stable pool to solver model: {amm:?}")
                            })?,
                        amplification_parameter: amm.amplification_parameter.as_big_rational(),
                    }),
                    fee: amm.fee.clone(),
                    cost: gas_model.balancer_cost(),
                    mandatory: false,
                    address: amm.address,
                },
                Liquidity::LimitOrder(_) => panic!("limit orders are expected to be filtered out"),
                Liquidity::Concentrated(amm) => AmmModel {
                    parameters: AmmParameters::Concentrated(ConcentratedPoolParameters {
                        pool: amm.pool.clone(),
                    }),
                    fee: BigRational::new(
                        BigInt::from(*amm.pool.state.fee.numer()),
                        BigInt::from(*amm.pool.state.fee.denom()),
                    ),
                    cost: gas_model.cost_for_gas(amm.pool.gas_stats.mean_gas),
                    mandatory: false,
                    address: amm.pool.address,
                },
            })
        })
        .filter_map(|result| match result {
            Ok(value) => Some((value.address, value)),
            Err(err) => {
                tracing::error!(?err, "error converting liquidity to solver model");
                None
            }
        })
        .collect()
}

fn compute_fee_connected_tokens(liquidity: &[Liquidity], native_token: H160) -> HashSet<H160> {
    // Find all tokens that are connected through potentially multiple amm hops to the fee.
    // TODO: Replace with a more optimal graph algorithm.
    let mut pairs = liquidity
        .iter()
        .flat_map(|amm| amm.all_token_pairs())
        .collect::<HashSet<_>>();
    let mut fee_connected_tokens = hashset![native_token];
    loop {
        let mut added_token = false;
        pairs.retain(|token_pair| {
            let tokens = token_pair.get();
            if fee_connected_tokens.contains(&tokens.0) {
                fee_connected_tokens.insert(tokens.1);
                added_token = true;
                false
            } else if fee_connected_tokens.contains(&tokens.1) {
                fee_connected_tokens.insert(tokens.0);
                added_token = true;
                false
            } else {
                true
            }
        });
        if pairs.is_empty() || !added_token {
            break;
        }
    }

    fee_connected_tokens
}

/// Failure indicating the transaction reverted for some reason
fn is_transaction_failure(error: &ExecutionError) -> bool {
    matches!(error, ExecutionError::Failure(_))
        || matches!(error, ExecutionError::Revert(_))
        || matches!(error, ExecutionError::InvalidOpcode)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        liquidity::{tests::CapturingSettlementHandler, ConstantProductOrder},
        settlement::external_prices::externalprices,
        solver::http_solver::buffers::MockBufferRetrieving,
    };
    use model::TokenPair;
    use shared::token_info::MockTokenInfoFetching;

    #[tokio::test]
    async fn remove_orders_without_native_connection_() {
        let limit_handling = CapturingSettlementHandler::arc();
        let amm_handling = CapturingSettlementHandler::arc();

        let native_token = H160::from_low_u64_be(0);
        let tokens = [
            H160::from_low_u64_be(1),
            H160::from_low_u64_be(2),
            H160::from_low_u64_be(3),
            H160::from_low_u64_be(4),
        ];

        let amms = [(native_token, tokens[0]), (tokens[0], tokens[1])]
            .iter()
            .map(|tokens| {
                Liquidity::ConstantProduct(ConstantProductOrder {
                    address: H160::from_low_u64_be(1),
                    tokens: TokenPair::new(tokens.0, tokens.1).unwrap(),
                    reserves: (0, 0),
                    fee: 0.into(),
                    settlement_handling: amm_handling.clone(),
                })
            })
            .collect::<Vec<_>>();

        let orders = [
            (native_token, tokens[0]),
            (native_token, tokens[1]),
            (tokens[0], tokens[1]),
            (tokens[1], tokens[0]),
            (tokens[1], tokens[2]),
            (tokens[2], tokens[1]),
            (tokens[2], tokens[3]),
            (tokens[3], tokens[2]),
        ]
        .iter()
        .map(|tokens| LimitOrder {
            sell_token: tokens.0,
            buy_token: tokens.1,
            kind: OrderKind::Sell,
            settlement_handling: limit_handling.clone(),
            ..Default::default()
        })
        .collect::<Vec<_>>();

        let mut token_infos = MockTokenInfoFetching::new();
        token_infos.expect_get_token_infos().returning(|tokens| {
            tokens
                .iter()
                .map(|token| (*token, TokenInfo::default()))
                .collect()
        });

        let mut buffer_retriever = MockBufferRetrieving::new();
        buffer_retriever.expect_get_buffers().returning(|tokens| {
            tokens
                .iter()
                .map(|token| (*token, Ok(U256::zero())))
                .collect()
        });

        let solver = InstanceCreator {
            native_token: H160::zero(),
            token_info_fetcher: Arc::new(token_infos),
            buffer_retriever: Arc::new(buffer_retriever),
            market_makable_token_list: Default::default(),
            environment_metadata: Default::default(),
        };

        let instances = solver
            .prepare_instances(0, 0, orders, amms, 0., &Default::default())
            .await;
        assert_eq!(instances.filtered.orders.len(), 6);
        assert_eq!(instances.plain.orders.len(), 8);
    }

    #[tokio::test]
    async fn prepares_models_with_mixed_liquidity() {
        let address = |x| H160([x; 20]);
        let native_token = address(0xef);

        let mut token_infos = MockTokenInfoFetching::new();
        token_infos.expect_get_token_infos().returning(|tokens| {
            tokens
                .iter()
                .map(|token| (*token, TokenInfo::default()))
                .collect()
        });

        let mut buffer_retriever = MockBufferRetrieving::new();
        buffer_retriever.expect_get_buffers().returning(|tokens| {
            tokens
                .iter()
                .map(|token| (*token, Ok(U256::zero())))
                .collect()
        });

        let solver = InstanceCreator {
            native_token: H160::zero(),
            token_info_fetcher: Arc::new(token_infos),
            buffer_retriever: Arc::new(buffer_retriever),
            market_makable_token_list: Default::default(),
            environment_metadata: Default::default(),
        };

        let instances = solver
            .prepare_instances(
                42,
                1337,
                vec![
                    LimitOrder {
                        sell_token: address(1),
                        buy_token: address(2),
                        sell_amount: 1.into(),
                        buy_amount: 2.into(),
                        ..Default::default()
                    },
                    LimitOrder {
                        sell_token: address(3),
                        buy_token: address(4),
                        sell_amount: 3.into(),
                        buy_amount: 4.into(),
                        ..Default::default()
                    },
                ],
                vec![
                    Liquidity::ConstantProduct(ConstantProductOrder {
                        address: address(0x56),
                        tokens: TokenPair::new(address(5), address(6)).unwrap(),
                        reserves: (5, 6),
                        ..Default::default()
                    }),
                    Liquidity::LimitOrder(LimitOrder {
                        sell_token: address(7),
                        buy_token: address(8),
                        sell_amount: 7.into(),
                        buy_amount: 8.into(),
                        ..Default::default()
                    }),
                    Liquidity::ConstantProduct(ConstantProductOrder {
                        address: address(0x9a),
                        tokens: TokenPair::new(address(9), address(10)).unwrap(),
                        reserves: (9, 10),
                        ..Default::default()
                    }),
                ],
                1e9,
                &externalprices! {
                    native_token: native_token,
                    address(1) => BigRational::new(1.into(), 1.into()),
                    address(2) => BigRational::new(2.into(), 2.into()),
                    address(3) => BigRational::new(3.into(), 3.into()),
                    address(4) => BigRational::new(4.into(), 4.into()),
                },
            )
            .await;

        assert_btreemap_size(&instances.plain.orders, 3);
        assert_eq!(instances.plain.amms.len(), 2);

        assert_eq!(instances.context.orders.len(), 3);
        assert_eq!(instances.context.liquidity.len(), 2);
    }

    fn assert_btreemap_size<V>(map: &BTreeMap<usize, V>, len: usize) {
        assert_eq!(map.len(), len);
        for i in 0..len {
            assert!(map.contains_key(&i));
        }
    }
}
