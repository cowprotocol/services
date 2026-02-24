use {
    super::{LimitOrderExecution, LimitOrderId, LiquidityOrderId, SettlementHandling},
    crate::{
        interactions::ZeroExInteraction,
        liquidity::{Exchange, LimitOrder, Liquidity},
        liquidity_collector::LiquidityCollecting,
        settlement::SettlementEncoder,
    },
    alloy::primitives::{Address, U256},
    anyhow::Result,
    arc_swap::ArcSwap,
    contracts::alloy::IZeroex,
    ethrpc::block_stream::{CurrentBlockWatcher, into_stream},
    futures::StreamExt,
    itertools::Itertools,
    model::{TokenPair, order::OrderKind},
    shared::{
        recent_block_cache::Block,
        zeroex_api::{OrderRecord, OrdersQuery, ZeroExApi},
    },
    std::{
        collections::{HashMap, HashSet},
        sync::Arc,
    },
    tracing::instrument,
};

type OrderBuckets = HashMap<(Address, Address), Vec<OrderRecord>>;
type OrderbookCache = ArcSwap<OrderBuckets>;

pub struct ZeroExLiquidity {
    // todo: remove Arc
    pub zeroex: Arc<IZeroex::Instance>,
    pub orderbook_cache: Arc<OrderbookCache>,
}

impl ZeroExLiquidity {
    pub async fn new(
        api: Arc<dyn ZeroExApi>,
        zeroex: IZeroex::Instance,
        gpv2: Address,
        blocks_stream: CurrentBlockWatcher,
    ) -> Self {
        let orderbook_cache: Arc<OrderbookCache> = Default::default();
        let cache = orderbook_cache.clone();
        tokio::spawn(
            async move { Self::run_orderbook_fetching(api, blocks_stream, cache, gpv2).await },
        );

        Self {
            zeroex: Arc::new(zeroex),
            orderbook_cache,
        }
    }

    /// Turns 0x OrderRecord into liquidity which solvers can use.
    fn record_into_liquidity(&self, record: OrderRecord) -> Option<Liquidity> {
        let sell_amount = U256::from(record.remaining_maker_amount().ok()?);
        if sell_amount.is_zero() || record.metadata().remaining_fillable_taker_amount == 0 {
            // filter out orders with 0 amounts to prevent errors in the solver
            return None;
        }

        let limit_order = LimitOrder {
            id: LimitOrderId::Liquidity(LiquidityOrderId::ZeroEx(const_hex::encode(
                &record.metadata().order_hash,
            ))),
            sell_token: record.order().maker_token,
            buy_token: record.order().taker_token,
            sell_amount,
            buy_amount: U256::from(record.metadata().remaining_fillable_taker_amount),
            kind: OrderKind::Buy,
            partially_fillable: true,
            user_fee: U256::ZERO,
            settlement_handling: Arc::new(OrderSettlementHandler {
                order_record: record,
                zeroex: self.zeroex.clone(),
            }),
            exchange: Exchange::ZeroEx,
        };
        Some(Liquidity::LimitOrder(limit_order))
    }

    async fn run_orderbook_fetching(
        api: Arc<dyn ZeroExApi>,
        blocks_stream: CurrentBlockWatcher,
        orderbook_cache: Arc<OrderbookCache>,
        gpv2_address: Address,
    ) {
        let mut block_stream = into_stream(blocks_stream);
        while block_stream.next().await.is_some() {
            let queries = &[
                // orders fillable by anyone
                OrdersQuery::default(),
                // orders fillable only by our settlement contract
                OrdersQuery {
                    sender: Some(gpv2_address),
                    ..Default::default()
                },
            ];
            let zeroex_orders_results =
                futures::future::join_all(queries.iter().map(|query| api.get_orders(query))).await;
            let order_buckets =
                group_by_token_pair(zeroex_orders_results.into_iter().flat_map(|result| {
                    result.unwrap_or_else(|err| {
                        tracing::error!(?err, "ZeroExResponse error during liqudity fetching");
                        vec![]
                    })
                }));

            orderbook_cache.store(Arc::new(order_buckets));
        }
    }
}

#[async_trait::async_trait]
impl LiquidityCollecting for ZeroExLiquidity {
    #[instrument(name = "zeroex_liquidity", skip_all)]
    async fn get_liquidity(
        &self,
        pairs: HashSet<TokenPair>,
        _block: Block,
    ) -> Result<Vec<Liquidity>> {
        let zeroex_order_buckets = self.orderbook_cache.load();
        let filtered_zeroex_orders = get_useful_orders(zeroex_order_buckets.as_ref(), &pairs, 5);

        let zeroex_liquidity_orders: Vec<_> = filtered_zeroex_orders
            .into_iter()
            .flat_map(|order| self.record_into_liquidity(order))
            .collect();

        Ok(zeroex_liquidity_orders)
    }
}

fn group_by_token_pair(
    orders: impl Iterator<Item = OrderRecord>,
) -> HashMap<(Address, Address), Vec<OrderRecord>> {
    orders
        .filter_map(|record| {
            TokenPair::new(record.order().taker_token, record.order().maker_token).map(|_| {
                (
                    (record.order().taker_token, record.order().maker_token),
                    record,
                )
            })
        })
        .into_group_map()
}

/// Get the `orders_per_type` best priced and biggest volume orders.
fn get_useful_orders(
    order_buckets: &OrderBuckets,
    relevant_pairs: &HashSet<TokenPair>,
    orders_per_type: usize,
) -> Vec<OrderRecord> {
    let mut filtered_zeroex_orders = vec![];
    for orders in order_buckets
        .iter()
        .filter_map(|((token_a, token_b), record)| {
            TokenPair::new(*token_a, *token_b)
                .is_some_and(|pair| relevant_pairs.contains(&pair))
                .then_some(record)
        })
    {
        let mut orders = orders.clone();
        if orders.len() <= 2 * orders_per_type {
            filtered_zeroex_orders.extend(orders);
            continue;
        }
        // Sorting to have best priced orders at the end of the vector
        // best priced orders are those that have the maximum maker_amount /
        // taker_amount ratio
        orders.sort_by(|order_1, order_2| {
            let price_1 = order_1.order().maker_amount as f64 / order_1.order().taker_amount as f64;
            let price_2 = order_2.order().maker_amount as f64 / order_2.order().taker_amount as f64;
            price_1.total_cmp(&price_2)
        });
        filtered_zeroex_orders.extend(orders.drain(orders.len() - orders_per_type..));

        orders.sort_by_key(|order| order.metadata().remaining_fillable_taker_amount);
        filtered_zeroex_orders.extend(orders.into_iter().rev().take(orders_per_type));
    }
    filtered_zeroex_orders
}

#[derive(Clone)]
pub struct OrderSettlementHandler {
    pub order_record: OrderRecord,
    // todo: remove Arc
    pub zeroex: Arc<IZeroex::Instance>,
}

impl SettlementHandling<LimitOrder> for OrderSettlementHandler {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn encode(
        &self,
        execution: LimitOrderExecution,
        encoder: &mut SettlementEncoder,
    ) -> Result<()> {
        let Ok(execution_filled) = u128::try_from(execution.filled) else {
            anyhow::bail!("0x only supports executed amounts of size u128");
        };
        encoder.append_to_execution_plan(Arc::new(ZeroExInteraction {
            taker_token_fill_amount: execution_filled,
            order: self.order_record.order().clone(),
            zeroex: self.zeroex.clone(),
        }));
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use {
        super::*,
        shared::{
            baseline_solver::BaseTokens,
            http_solver::model::InternalizationStrategy,
            interaction::Interaction,
            zeroex_api::{self, OrderMetadata},
        },
    };

    fn get_relevant_pairs(token_a: Address, token_b: Address) -> HashSet<TokenPair> {
        let base_tokens = Arc::new(BaseTokens::new(Address::ZERO, &[]));
        let fake_order = [TokenPair::new(token_a, token_b).unwrap()].into_iter();
        base_tokens.relevant_pairs(fake_order)
    }

    fn order_with_tokens(token_a: Address, token_b: Address) -> OrderRecord {
        OrderRecord::new(
            zeroex_api::Order {
                taker_token: token_a,
                maker_token: token_b,
                ..Default::default()
            },
            OrderMetadata::default(),
        )
    }

    #[test]
    fn order_buckets_get_created() {
        let token_a = Address::repeat_byte(0x00);
        let token_b = Address::repeat_byte(0xff);
        let relevant_pairs = get_relevant_pairs(token_a, token_b);
        let order_1 = order_with_tokens(token_a, token_b);
        let order_2 = order_with_tokens(token_b, token_a);
        let order_3 = order_with_tokens(token_b, token_a);
        let order_buckets = group_by_token_pair(vec![order_1, order_2, order_3].into_iter());
        let useful_orders = get_useful_orders(&order_buckets, &relevant_pairs, 1);
        assert_eq!(order_buckets.keys().len(), 2);
        assert_eq!(useful_orders.len(), 3);
    }

    #[test]
    fn empty_bucket_no_relevant_orders() {
        let token_a = Address::repeat_byte(0x00);
        let token_b = Address::repeat_byte(0xff);
        let token_ignore = Address::repeat_byte(0x11);
        let relevant_pairs = get_relevant_pairs(token_a, token_b);
        let order_1 = order_with_tokens(token_ignore, token_b);
        let order_2 = order_with_tokens(token_a, token_ignore);
        let order_3 = order_with_tokens(token_ignore, token_ignore);
        let order_buckets = group_by_token_pair(vec![order_1, order_2, order_3].into_iter());
        let filtered_zeroex_orders = get_useful_orders(&order_buckets, &relevant_pairs, 1);
        assert_eq!(filtered_zeroex_orders.len(), 0);
    }

    #[test]
    fn biggest_volume_orders_get_selected() {
        let token_a = Address::repeat_byte(0x00);
        let token_b = Address::repeat_byte(0xff);
        let relevant_pairs = get_relevant_pairs(token_a, token_b);
        let order_with_fillable_amount = |remaining_fillable_taker_amount| {
            OrderRecord::new(
                zeroex_api::Order {
                    taker_token: token_a,
                    maker_token: token_b,
                    taker_amount: 100_000_000,
                    maker_amount: 100_000_000,
                    ..Default::default()
                },
                OrderMetadata {
                    remaining_fillable_taker_amount,
                    ..Default::default()
                },
            )
        };
        let order_1 = order_with_fillable_amount(1_000);
        let order_2 = order_with_fillable_amount(100);
        let order_3 = order_with_fillable_amount(10_000);
        let order_buckets = group_by_token_pair(vec![order_1, order_2, order_3].into_iter());
        let filtered_zeroex_orders = get_useful_orders(&order_buckets, &relevant_pairs, 1);
        assert_eq!(filtered_zeroex_orders.len(), 2);
        assert_eq!(
            filtered_zeroex_orders[0]
                .metadata()
                .remaining_fillable_taker_amount,
            10_000
        );
        assert_eq!(
            filtered_zeroex_orders[1]
                .metadata()
                .remaining_fillable_taker_amount,
            1_000
        );
    }

    #[test]
    fn best_priced_orders_get_selected() {
        let token_a = Address::repeat_byte(0x00);
        let token_b = Address::repeat_byte(0xff);
        let relevant_pairs = get_relevant_pairs(token_a, token_b);
        let order_with_amount = |taker_amount, remaining_fillable_taker_amount| {
            OrderRecord::new(
                zeroex_api::Order {
                    taker_token: token_a,
                    maker_token: token_b,
                    taker_amount,
                    maker_amount: 100_000_000,
                    ..Default::default()
                },
                OrderMetadata {
                    remaining_fillable_taker_amount,
                    ..Default::default()
                },
            )
        };
        let order_1 = order_with_amount(10_000_000, 1_000_000);
        let order_2 = order_with_amount(1_000, 100);
        let order_3 = order_with_amount(100_000, 1_000);
        let order_buckets = group_by_token_pair(vec![order_1, order_2, order_3].into_iter());
        let filtered_zeroex_orders = get_useful_orders(&order_buckets, &relevant_pairs, 1);
        assert_eq!(filtered_zeroex_orders.len(), 2);
        // First item in the list will be on the basis of maker_amount/taker_amount
        // ratio
        assert_eq!(filtered_zeroex_orders[0].order().taker_amount, 1_000);
        // Second item in the list will be on the basis of
        // remaining_fillable_taker_amount
        assert_eq!(filtered_zeroex_orders[1].order().taker_amount, 10_000_000);
    }

    #[tokio::test]
    async fn interaction_encoding() {
        let sell_token = Address::with_last_byte(1);
        let zeroex = Arc::new(IZeroex::Instance::new(
            Default::default(),
            ethrpc::mock::web3().provider,
        ));
        let order_record = OrderRecord::new(
            zeroex_api::Order {
                taker_amount: 100,
                taker_token: sell_token,
                ..Default::default()
            },
            OrderMetadata::default(),
        );
        let handler = OrderSettlementHandler {
            order_record: order_record.clone(),
            zeroex: zeroex.clone(),
        };
        let mut encoder = SettlementEncoder::default();
        let execution = LimitOrderExecution::new(U256::from(100), U256::ZERO);
        handler.encode(execution, &mut encoder).unwrap();
        let [_, interactions, _] = encoder
            .finish(InternalizationStrategy::SkipInternalizableInteraction)
            .interactions;
        assert_eq!(
            interactions,
            [ZeroExInteraction {
                order: order_record.order().clone(),
                taker_token_fill_amount: 100,
                zeroex: zeroex.clone(),
            }
            .encode()],
        );
    }
}
