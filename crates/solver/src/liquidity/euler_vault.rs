use {
    super::{LimitOrderExecution, LimitOrderId, LiquidityOrderId, SettlementHandling},
    crate::{
        interactions::{
            allowances::{AllowanceManager, AllowanceManaging, Allowances, Approval}, EulerVaultInteraction
        },
        liquidity::{AmmOrderExecution, Exchange, LimitOrder, Liquidity, WrappedLiquidityOrder},
        liquidity_collector::LiquidityCollecting,
        settlement::SettlementEncoder,
    },
    anyhow::Result,
    arc_swap::ArcSwap,
    contracts::{alloy::EulerVault, GPv2Settlement},
    ethrpc::{
        alloy::conversions::IntoLegacy,
        block_stream::{into_stream, CurrentBlockWatcher},
    },
    futures::StreamExt,
    itertools::Itertools,
    model::{order::OrderKind, TokenPair},
    primitive_types::{H160, U256},
    shared::{
        ethrpc::Web3,
        http_solver::model::TokenAmount,
        recent_block_cache::Block,
    },
    std::{
        collections::{HashMap, HashSet},
        sync::Arc,
    },
    tracing::instrument,
};

type OrderBuckets = HashMap<(H160, H160), Vec<OrderRecord>>;
type OrderbookCache = ArcSwap<OrderBuckets>;

pub struct EulerVaultLiquidity {
    // todo: remove Arc
    pub vault: Arc<EulerVault::Instance>,
    pub allowance_manager: Box<dyn AllowanceManaging>,
    pub orderbook_cache: Arc<OrderbookCache>,
}

impl EulerVaultLiquidity {
    pub async fn new(
        web3: Web3,
        vault: EulerVault::Instance,
        gpv2: GPv2Settlement,
        blocks_stream: CurrentBlockWatcher,
    ) -> Self {
        let gpv2_address = gpv2.address();
        let allowance_manager = AllowanceManager::new(web3, gpv2_address);
        let orderbook_cache: Arc<OrderbookCache> = Default::default();
        let cache = orderbook_cache.clone();

        Self {
            vault: Arc::new(vault),
            allowance_manager: Box::new(allowance_manager),
            orderbook_cache,
        }
    }
}

#[derive(Clone)]
pub struct EulerSettlementHandler {
    // todo: remove Arc
    pub vault: Arc<IEulerVault::Instance>,
    allowances: Arc<Allowances>,
}

impl EulerSettlementHandler {
    pub fn settle(
        &self,
        token_amount_in_max: TokenAmount,
        token_amount_out: TokenAmount,
    ) -> (Option<Approval>, EulerVaultInteraction) {
        let approval = self
            .allowances
            .approve_token_or_default(token_amount_in_max.clone());

        (
            approval,
            UniswapInteraction {
                router: self.router.clone(),
                settlement: self.gpv2_settlement.clone(),
                amount_out: token_amount_out.amount,
                amount_in_max: token_amount_in_max.amount,
                token_in: token_amount_in_max.token,
                token_out: token_amount_out.token,
            },
        )
    }

}

impl SettlementHandling<WrappedLiquidityOrder> for OrderSettlementHandler {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn encode(
        &self,
        execution: AmmOrderExecution,
        encoder: &mut SettlementEncoder,
    ) -> Result<()> {
        let (approval, swap) = self.settle(execution.input_max, execution.output);
        if let Some(approval) = approval {
            encoder.append_to_execution_plan_internalizable(
                Arc::new(approval),
                execution.internalizable,
            );
        }
        encoder.append_to_execution_plan_internalizable(Arc::new(swap), execution.internalizable);
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use {
        super::*,
        crate::interactions::allowances::Approval,
        ethrpc::alloy::conversions::IntoAlloy,
        maplit::hashmap,
        shared::{
            baseline_solver::BaseTokens,
            http_solver::model::InternalizationStrategy,
            interaction::Interaction,
            zeroex_api::{self, OrderMetadata},
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
        let order_with_tokens = |token_a, token_b| {
            OrderRecord::new(
                zeroex_api::Order {
                    taker_token: token_a,
                    maker_token: token_b,
                    ..Default::default()
                },
                OrderMetadata::default(),
            )
        };
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
        let token_a = H160([0x00; 20]);
        let token_b = H160([0xff; 20]);
        let token_ignore = H160([0x11; 20]);
        let relevant_pairs = get_relevant_pairs(token_a, token_b);
        let order_with_tokens = |token_a, token_b| {
            OrderRecord::new(
                zeroex_api::Order {
                    taker_token: token_a,
                    maker_token: token_b,
                    ..Default::default()
                },
                OrderMetadata::default(),
            )
        };
        let order_1 = order_with_tokens(token_ignore, token_b);
        let order_2 = order_with_tokens(token_a, token_ignore);
        let order_3 = order_with_tokens(token_ignore, token_ignore);
        let order_buckets = group_by_token_pair(vec![order_1, order_2, order_3].into_iter());
        let filtered_zeroex_orders = get_useful_orders(&order_buckets, &relevant_pairs, 1);
        assert_eq!(filtered_zeroex_orders.len(), 0);
    }

    #[test]
    fn biggest_volume_orders_get_selected() {
        let token_a = H160([0x00; 20]);
        let token_b = H160([0xff; 20]);
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
        let token_a = H160([0x00; 20]);
        let token_b = H160([0xff; 20]);
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
    async fn interaction_encodes_approval_when_insufficient() {
        let sell_token = H160::from_low_u64_be(1);
        let zeroex = Arc::new(IZeroex::Instance::new(
            H160::default().into_alloy(),
            ethrpc::mock::web3().alloy,
        ));
        let allowances = Allowances::new(
            zeroex.address().into_legacy(),
            hashmap! { sell_token => 99.into() },
        );
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
            allowances: Arc::new(allowances),
        };
        let mut encoder = SettlementEncoder::default();
        let execution = LimitOrderExecution::new(100.into(), 0.into());
        handler.encode(execution, &mut encoder).unwrap();
        let [_, interactions, _] = encoder
            .finish(InternalizationStrategy::SkipInternalizableInteraction)
            .interactions;
        assert_eq!(
            interactions,
            [
                Approval {
                    token: sell_token,
                    spender: zeroex.address().into_legacy(),
                }
                .encode(),
                ZeroExInteraction {
                    order: order_record.order().clone(),
                    taker_token_fill_amount: 100,
                    zeroex: zeroex.clone(),
                }
                .encode(),
            ],
        );
    }

    #[tokio::test]
    async fn interaction_encodes_no_approval_when_sufficient() {
        let sell_token = H160::from_low_u64_be(1);
        let zeroex = Arc::new(IZeroex::Instance::new(
            H160::default().into_alloy(),
            ethrpc::mock::web3().alloy,
        ));
        let allowances = Allowances::new(
            zeroex.address().into_legacy(),
            hashmap! { sell_token => 100.into() },
        );
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
            allowances: Arc::new(allowances),
        };
        let mut encoder = SettlementEncoder::default();
        let execution = LimitOrderExecution::new(100.into(), 0.into());
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
