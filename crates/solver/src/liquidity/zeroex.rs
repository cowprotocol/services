use {
    super::{LimitOrderId, LiquidityOrderId, SettlementHandling},
    crate::{
        interactions::{
            allowances::{AllowanceManager, AllowanceManaging, Allowances},
            ZeroExInteraction,
        },
        liquidity::{Exchange, LimitOrder, Liquidity},
        liquidity_collector::LiquidityCollecting,
        settlement::SettlementEncoder,
    },
    anyhow::Result,
    contracts::{GPv2Settlement, IZeroEx},
    model::{order::OrderKind, TokenPair},
    primitive_types::{H160, U256},
    shared::{
        ethrpc::Web3,
        http_solver::model::TokenAmount,
        recent_block_cache::Block,
        zeroex_api::{Order, OrderRecord, OrdersQuery, ZeroExApi},
    },
    std::{
        collections::{HashMap, HashSet},
        sync::Arc,
    },
};

pub struct ZeroExLiquidity {
    pub api: Arc<dyn ZeroExApi>,
    pub zeroex: IZeroEx,
    pub gpv2: GPv2Settlement,
    pub allowance_manager: Box<dyn AllowanceManaging>,
}

type OrderBuckets = HashMap<(H160, H160), Vec<OrderRecord>>;

impl ZeroExLiquidity {
    pub fn new(web3: Web3, api: Arc<dyn ZeroExApi>, zeroex: IZeroEx, gpv2: GPv2Settlement) -> Self {
        let allowance_manager = AllowanceManager::new(web3, gpv2.address());
        Self {
            api,
            zeroex,
            gpv2,
            allowance_manager: Box::new(allowance_manager),
        }
    }

    /// Turns 0x OrderRecord into liquidity which solvers can use.
    fn record_into_liquidity(
        &self,
        record: OrderRecord,
        allowances: Arc<Allowances>,
    ) -> Option<Liquidity> {
        let sell_amount: U256 = record.remaining_maker_amount().ok()?.into();
        if sell_amount.is_zero() || record.metadata.remaining_fillable_taker_amount == 0 {
            // filter out orders with 0 amounts to prevent errors in the solver
            return None;
        }

        let limit_order = LimitOrder {
            id: LimitOrderId::Liquidity(LiquidityOrderId::ZeroEx(hex::encode(
                &record.metadata.order_hash,
            ))),
            sell_token: record.order.maker_token,
            buy_token: record.order.taker_token,
            sell_amount,
            buy_amount: record.metadata.remaining_fillable_taker_amount.into(),
            kind: OrderKind::Buy,
            partially_fillable: true,
            unscaled_subsidized_fee: U256::zero(),
            scaled_unsubsidized_fee: U256::zero(),
            settlement_handling: Arc::new(OrderSettlementHandler {
                order: record.order,
                zeroex: self.zeroex.clone(),
                allowances,
            }),
            exchange: Exchange::ZeroEx,
            reward: 0.,
            is_mature: false, // irrelevant for liquidity orders
        };
        Some(Liquidity::LimitOrder(limit_order))
    }
}

#[async_trait::async_trait]
impl LiquidityCollecting for ZeroExLiquidity {
    async fn get_liquidity(
        &self,
        pairs: HashSet<TokenPair>,
        _block: Block,
    ) -> Result<Vec<Liquidity>> {
        let queries = &[
            // orders fillable by anyone
            OrdersQuery::default(),
            // orders fillable only by our settlement contract
            OrdersQuery {
                sender: Some(self.gpv2.address()),
                ..Default::default()
            },
        ];

        let zeroex_orders_results =
            futures::future::join_all(queries.iter().map(|query| self.api.get_orders(query))).await;
        let zeroex_orders = zeroex_orders_results
            .into_iter()
            .flat_map(|result| match result {
                Ok(order_record_vec) => order_record_vec,
                Err(err) => {
                    tracing::warn!("ZeroExResponse error during liqudity fetching: {}", err);
                    vec![]
                }
            });

        let order_buckets = generate_order_buckets(zeroex_orders, pairs);
        let filtered_zeroex_orders = get_useful_orders(order_buckets, 5);
        let tokens: HashSet<_> = filtered_zeroex_orders
            .iter()
            .map(|o| o.order.taker_token)
            .collect();

        let allowances = Arc::new(
            self.allowance_manager
                .get_allowances(tokens, self.zeroex.address())
                .await?,
        );

        let zeroex_liquidity_orders: Vec<_> = filtered_zeroex_orders
            .into_iter()
            .flat_map(|order| self.record_into_liquidity(order, allowances.clone()))
            .collect();

        Ok(zeroex_liquidity_orders)
    }
}

fn generate_order_buckets(
    zeroex_orders: impl Iterator<Item = OrderRecord>,
    relevant_pairs: HashSet<TokenPair>,
) -> OrderBuckets {
    // divide orders in buckets
    let mut buckets = OrderBuckets::default();
    zeroex_orders
        .filter(
            |record| match TokenPair::new(record.order.taker_token, record.order.maker_token) {
                Some(pair) => relevant_pairs.contains(&pair),
                None => false,
            },
        )
        .for_each(|order| {
            let bucket = buckets
                .entry((order.order.taker_token, order.order.maker_token))
                .or_default();
            bucket.push(order);
        });
    buckets
}

/// Get the `orders_per_type` best priced and biggest volume orders.
fn get_useful_orders(order_buckets: OrderBuckets, orders_per_type: usize) -> Vec<OrderRecord> {
    let mut filtered_zeroex_orders = vec![];
    for mut orders in order_buckets.into_values() {
        if orders.len() <= 2 * orders_per_type {
            filtered_zeroex_orders.extend(orders);
            continue;
        }
        // Sorting to have best priced orders at the end of the vector
        // best priced orders are those that have the maximum maker_amount /
        // taker_amount ratio
        orders.sort_by(|order_1, order_2| {
            let price_1 = order_1.order.maker_amount as f64 / order_1.order.taker_amount as f64;
            let price_2 = order_2.order.maker_amount as f64 / order_2.order.taker_amount as f64;
            price_1.total_cmp(&price_2)
        });
        filtered_zeroex_orders.extend(orders.drain(orders.len() - orders_per_type..));

        orders.sort_by_key(|order| order.metadata.remaining_fillable_taker_amount);
        filtered_zeroex_orders.extend(orders.into_iter().rev().take(orders_per_type));
    }
    filtered_zeroex_orders
}

struct OrderSettlementHandler {
    order: Order,
    zeroex: IZeroEx,
    allowances: Arc<Allowances>,
}

impl SettlementHandling<LimitOrder> for OrderSettlementHandler {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn encode(&self, executed_amount: U256, encoder: &mut SettlementEncoder) -> Result<()> {
        if executed_amount > u128::MAX.into() {
            anyhow::bail!("0x only supports executed amounts of size u128");
        }
        if let Some(approval) = self
            .allowances
            .approve_token(TokenAmount::new(self.order.taker_token, executed_amount))?
        {
            encoder.append_to_execution_plan(approval);
        }
        encoder.append_to_execution_plan(ZeroExInteraction {
            taker_token_fill_amount: executed_amount.as_u128(),
            order: self.order.clone(),
            zeroex: self.zeroex.clone(),
        });
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use {
        super::*,
        crate::interactions::allowances::Approval,
        maplit::hashmap,
        shared::{
            baseline_solver::BaseTokens,
            http_solver::model::InternalizationStrategy,
            interaction::Interaction,
            zeroex_api::OrderMetadata,
        },
    };

    fn get_relevant_pairs(token_a: H160, token_b: H160) -> HashSet<TokenPair> {
        let base_tokens = Arc::new(BaseTokens::new(H160::zero(), &[]));
        let fake_order = [TokenPair::new(token_a, token_b).unwrap()].into_iter();
        base_tokens.relevant_pairs(fake_order)
    }

    #[test]
    fn order_buckets_get_created() {
        let token_a = H160([0x00; 20]);
        let token_b = H160([0xff; 20]);
        let relevant_pairs = get_relevant_pairs(token_a, token_b);
        let order = Order::default();
        let metadata = OrderMetadata::default();
        let order_with_tokens = |token_a, token_b| OrderRecord {
            order: Order {
                taker_token: token_a,
                maker_token: token_b,
                ..order.clone()
            },
            metadata: metadata.clone(),
        };
        let order_1 = order_with_tokens(token_a, token_b);
        let order_2 = order_with_tokens(token_b, token_a);
        let order_3 = order_with_tokens(token_b, token_a);
        let order_buckets =
            generate_order_buckets([order_1, order_2, order_3].into_iter(), relevant_pairs);
        assert_eq!(order_buckets.keys().len(), 2);
        assert_eq!(order_buckets[&(token_a, token_b)].len(), 1);
        assert_eq!(order_buckets[&(token_b, token_a)].len(), 2);
    }

    #[test]
    fn empty_bucket_no_relevant_orders() {
        let token_a = H160([0x00; 20]);
        let token_b = H160([0xff; 20]);
        let token_ignore = H160([0x11; 20]);
        let relevant_pairs = get_relevant_pairs(token_a, token_b);
        let order = Order::default();
        let metadata = OrderMetadata::default();
        let order_with_tokens = |token_a, token_b| OrderRecord {
            order: Order {
                taker_token: token_a,
                maker_token: token_b,
                ..order.clone()
            },
            metadata: metadata.clone(),
        };
        let order_1 = order_with_tokens(token_ignore, token_b);
        let order_2 = order_with_tokens(token_a, token_ignore);
        let order_3 = order_with_tokens(token_ignore, token_ignore);
        let order_buckets =
            generate_order_buckets([order_1, order_2, order_3].into_iter(), relevant_pairs);
        let filtered_zeroex_orders = get_useful_orders(order_buckets, 1);
        assert_eq!(filtered_zeroex_orders.len(), 0);
    }

    #[test]
    fn biggest_volume_orders_get_selected() {
        let token_a = H160([0x00; 20]);
        let token_b = H160([0xff; 20]);
        let relevant_pairs = get_relevant_pairs(token_a, token_b);
        let order_with_fillable_amount = |remaining_fillable_taker_amount| OrderRecord {
            order: Order {
                taker_token: token_a,
                maker_token: token_b,
                taker_amount: 100_000_000,
                maker_amount: 100_000_000,
                ..Default::default()
            },
            metadata: OrderMetadata {
                remaining_fillable_taker_amount,
                ..Default::default()
            },
        };
        let order_1 = order_with_fillable_amount(1_000);
        let order_2 = order_with_fillable_amount(100);
        let order_3 = order_with_fillable_amount(10_000);
        let order_buckets =
            generate_order_buckets([order_1, order_2, order_3].into_iter(), relevant_pairs);
        let filtered_zeroex_orders = get_useful_orders(order_buckets, 1);
        assert_eq!(filtered_zeroex_orders.len(), 2);
        assert_eq!(
            filtered_zeroex_orders[0]
                .metadata
                .remaining_fillable_taker_amount,
            10_000
        );
        assert_eq!(
            filtered_zeroex_orders[1]
                .metadata
                .remaining_fillable_taker_amount,
            1_000
        );
    }

    #[test]
    fn best_priced_orders_get_selected() {
        let token_a = H160([0x00; 20]);
        let token_b = H160([0xff; 20]);
        let relevant_pairs = get_relevant_pairs(token_a, token_b);
        let order_with_amount = |taker_amount, remaining_fillable_taker_amount| OrderRecord {
            order: Order {
                taker_token: token_a,
                maker_token: token_b,
                taker_amount,
                maker_amount: 100_000_000,
                ..Default::default()
            },
            metadata: OrderMetadata {
                remaining_fillable_taker_amount,
                ..Default::default()
            },
        };
        let order_1 = order_with_amount(10_000_000, 1_000_000);
        let order_2 = order_with_amount(1_000, 100);
        let order_3 = order_with_amount(100_000, 1_000);
        let order_buckets =
            generate_order_buckets([order_1, order_2, order_3].into_iter(), relevant_pairs);
        let filtered_zeroex_orders = get_useful_orders(order_buckets, 1);
        assert_eq!(filtered_zeroex_orders.len(), 2);
        // First item in the list will be on the basis of maker_amount/taker_amount
        // ratio
        assert_eq!(filtered_zeroex_orders[0].order.taker_amount, 1_000);
        // Second item in the list will be on the basis of
        // remaining_fillable_taker_amount
        assert_eq!(filtered_zeroex_orders[1].order.taker_amount, 10_000_000);
    }

    #[tokio::test]
    async fn interaction_encodes_approval_when_insufficient() {
        let sell_token = H160::from_low_u64_be(1);
        let zeroex = shared::dummy_contract!(IZeroEx, H160::default());
        let allowances = Allowances::new(zeroex.address(), hashmap! { sell_token => 99.into() });
        let order = Order {
            taker_amount: 100,
            taker_token: sell_token,
            ..Default::default()
        };
        let handler = OrderSettlementHandler {
            order: order.clone(),
            zeroex: zeroex.clone(),
            allowances: Arc::new(allowances),
        };
        let mut encoder = SettlementEncoder::default();
        handler.encode(100.into(), &mut encoder).unwrap();
        let [_, interactions, _] = encoder
            .finish(InternalizationStrategy::SkipInternalizableInteraction)
            .interactions;
        assert_eq!(
            interactions,
            [
                Approval {
                    token: sell_token,
                    spender: zeroex.address(),
                }
                .encode(),
                ZeroExInteraction {
                    order,
                    taker_token_fill_amount: 100,
                    zeroex
                }
                .encode(),
            ]
            .concat(),
        );
    }

    #[tokio::test]
    async fn interaction_encodes_no_approval_when_sufficient() {
        let sell_token = H160::from_low_u64_be(1);
        let zeroex = shared::dummy_contract!(IZeroEx, H160::default());
        let allowances = Allowances::new(zeroex.address(), hashmap! { sell_token => 100.into() });
        let order = Order {
            taker_amount: 100,
            taker_token: sell_token,
            ..Default::default()
        };
        let handler = OrderSettlementHandler {
            order: order.clone(),
            zeroex: zeroex.clone(),
            allowances: Arc::new(allowances),
        };
        let mut encoder = SettlementEncoder::default();
        handler.encode(100.into(), &mut encoder).unwrap();
        let [_, interactions, _] = encoder
            .finish(InternalizationStrategy::SkipInternalizableInteraction)
            .interactions;
        assert_eq!(
            interactions,
            [ZeroExInteraction {
                order,
                taker_token_fill_amount: 100,
                zeroex
            }
            .encode(),]
            .concat(),
        );
    }
}
