use {
    crate::{
        interactions::allowances::{AllowanceManaging, Approval, ApprovalRequest},
        liquidity::{
            order_converter::OrderConverter,
            slippage::SlippageContext,
            AmmOrderExecution,
            LimitOrder,
            LimitOrderId,
            Liquidity,
        },
        settlement::Settlement,
    },
    anyhow::{anyhow, ensure, Context as _, Result},
    model::{
        order::{Order, OrderClass, OrderKind, OrderMetadata},
        DomainSeparator,
    },
    primitive_types::{H160, U256},
    shared::http_solver::model::*,
    std::{
        collections::{hash_map::Entry, HashMap, HashSet},
        sync::Arc,
    },
};

// To send an instance to the solver we need to identify tokens and orders
// through strings. This struct combines the created model and a mapping of
// those identifiers to their original value.
#[derive(Clone, Debug)]
pub struct SettlementContext {
    pub orders: Vec<LimitOrder>,
    pub liquidity: Vec<Liquidity>,
}

pub async fn convert_settlement(
    settled: SettledBatchAuctionModel,
    context: &SettlementContext,
    allowance_manager: Arc<dyn AllowanceManaging>,
    order_converter: Arc<OrderConverter>,
    slippage: SlippageContext<'_>,
    domain: &DomainSeparator,
) -> Result<Settlement, ConversionError> {
    IntermediateSettlement::new(
        settled,
        context,
        allowance_manager,
        order_converter,
        slippage,
        domain,
    )
    .await?
    .into_settlement()
    .map_err(Into::into)
}

#[derive(Clone, Debug)]
#[cfg_attr(test, derive(PartialEq))]
enum Execution {
    Amm(Box<ExecutedAmm>),
    CustomInteraction(Box<InteractionData>),
    LimitOrder(Box<ExecutedLimitOrder>),
}

impl Execution {
    fn execution_plan(&self) -> Option<&ExecutionPlan> {
        match self {
            Execution::Amm(executed_amm) => Some(&executed_amm.exec_plan),
            Execution::CustomInteraction(interaction) => interaction.exec_plan.as_ref(),
            Execution::LimitOrder(order) => order.exec_plan.as_ref(),
        }
    }

    fn coordinates(&self) -> Option<ExecutionPlanCoordinatesModel> {
        self.execution_plan()
            .map(|exec_plan| exec_plan.coordinates.clone())
    }

    fn add_to_settlement(
        &self,
        settlement: &mut Settlement,
        slippage: &SlippageContext,
        internalizable: bool,
    ) -> Result<()> {
        use Execution::*;

        match self {
            LimitOrder(order) => settlement.with_liquidity(&order.order, order.executed_amount()),
            Amm(executed_amm) => {
                let execution = slippage.apply_to_amm_execution(AmmOrderExecution {
                    input_max: executed_amm.input.clone(),
                    output: executed_amm.output.clone(),
                    internalizable,
                })?;
                match &executed_amm.order {
                    Liquidity::ConstantProduct(liquidity) => {
                        settlement.with_liquidity(liquidity, execution)
                    }
                    Liquidity::BalancerWeighted(liquidity) => {
                        settlement.with_liquidity(liquidity, execution)
                    }
                    Liquidity::BalancerStable(liquidity) => {
                        settlement.with_liquidity(liquidity, execution)
                    }
                    // This sort of liquidity gets used elsewhere
                    Liquidity::LimitOrder(_) => Ok(()),
                    Liquidity::Concentrated(liquidity) => {
                        settlement.with_liquidity(liquidity, execution)
                    }
                }
            }
            CustomInteraction(interaction_data) => {
                settlement.encoder.append_to_execution_plan_internalizable(
                    *interaction_data.clone(),
                    internalizable,
                );
                Ok(())
            }
        }
    }
}

// An intermediate representation between SettledBatchAuctionModel and
// Settlement useful for doing the error checking up front and then working with
// a more convenient representation.
struct IntermediateSettlement<'a> {
    approvals: Vec<Approval>,
    executions: Vec<Execution>, // executions are sorted by execution coordinate.
    prices: HashMap<H160, U256>,
    slippage: SlippageContext<'a>,
    submitter: SubmissionPreference,
    score: Option<Score>,
}

// Conversion error happens during building a settlement from a solution
// received from searcher
#[derive(Debug)]
pub enum ConversionError {
    InvalidExecutionPlans(anyhow::Error),
    Other(anyhow::Error),
}

impl From<anyhow::Error> for ConversionError {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err)
    }
}

impl From<ConversionError> for anyhow::Error {
    fn from(err: ConversionError) -> Self {
        match err {
            ConversionError::InvalidExecutionPlans(err) => err,
            ConversionError::Other(err) => err,
        }
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(test, derive(PartialEq))]
struct ExecutedLimitOrder {
    order: LimitOrder,
    executed_buy_amount: U256,
    executed_sell_amount: U256,
    exec_plan: Option<ExecutionPlan>,
}

impl ExecutedLimitOrder {
    fn executed_amount(&self) -> U256 {
        match self.order.kind {
            OrderKind::Buy => self.executed_buy_amount,
            OrderKind::Sell => self.executed_sell_amount,
        }
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(test, derive(PartialEq))]
struct ExecutedAmm {
    input: TokenAmount,
    output: TokenAmount,
    order: Liquidity,
    exec_plan: ExecutionPlan,
}

impl<'a> IntermediateSettlement<'a> {
    async fn new(
        settled: SettledBatchAuctionModel,
        context: &SettlementContext,
        allowance_manager: Arc<dyn AllowanceManaging>,
        order_converter: Arc<OrderConverter>,
        slippage: SlippageContext<'a>,
        domain: &DomainSeparator,
    ) -> Result<IntermediateSettlement<'a>, ConversionError> {
        let executed_limit_orders =
            match_prepared_and_settled_orders(&context.orders, settled.orders)?;
        let foreign_liquidity_orders = convert_foreign_liquidity_orders(
            order_converter,
            settled.foreign_liquidity_orders,
            domain,
        )?;
        let prices = match_settled_prices(executed_limit_orders.as_slice(), settled.prices)?;
        let approvals = compute_approvals(allowance_manager, settled.approvals).await?;
        let executions_amm = match_prepared_and_settled_amms(&context.liquidity, settled.amms)?;

        let executions = merge_and_order_executions(
            executions_amm,
            settled.interaction_data,
            [executed_limit_orders, foreign_liquidity_orders].concat(),
        );
        let submitter = settled.submitter;
        let score = settled.score;

        if duplicate_coordinates(&executions) {
            return Err(ConversionError::InvalidExecutionPlans(anyhow!(
                "Duplicate coordinates found."
            )));
        }

        Ok(Self {
            executions,
            prices,
            approvals,
            slippage,
            submitter,
            score,
        })
    }

    fn into_settlement(self) -> Result<Settlement> {
        let mut settlement = Settlement::new(self.prices);
        settlement.submitter = self.submitter;
        settlement.score = self.score;

        // Make sure to always add approval interactions **before** any
        // interactions from the execution plan - the execution plan typically
        // consists of AMM swaps that require these approvals to be in place.
        for approval in self.approvals {
            settlement.encoder.append_to_execution_plan(approval);
        }

        for execution in &self.executions {
            let internalizable = execution
                .execution_plan()
                .map(|exec_plan| exec_plan.internal)
                .unwrap_or_default();
            execution.add_to_settlement(&mut settlement, &self.slippage, internalizable)?;
        }

        Ok(settlement)
    }
}

fn match_prepared_and_settled_orders(
    prepared_orders: &[LimitOrder],
    settled_orders: HashMap<usize, ExecutedOrderModel>,
) -> Result<Vec<ExecutedLimitOrder>> {
    settled_orders
        .into_iter()
        .filter(|(_, settled)| {
            !(settled.exec_sell_amount.is_zero() && settled.exec_buy_amount.is_zero())
        })
        .map(|(index, settled)| {
            let prepared = prepared_orders
                .get(index)
                .ok_or_else(|| anyhow!("invalid order {}", index))?;
            if prepared.is_liquidity_order() {
                if let Some(internalizable) = settled.exec_plan.as_ref().map(|plan| plan.internal) {
                    ensure!(
                        !internalizable,
                        "liquidity orders are not allowed to be internalizable"
                    )
                }
            }
            Ok(ExecutedLimitOrder {
                order: prepared.clone(),
                executed_buy_amount: settled.exec_buy_amount,
                executed_sell_amount: settled.exec_sell_amount,
                exec_plan: settled.exec_plan,
            })
        })
        .collect()
}

fn convert_foreign_liquidity_orders(
    order_converter: Arc<OrderConverter>,
    foreign_liquidity_orders: Vec<ExecutedLiquidityOrderModel>,
    domain: &DomainSeparator,
) -> Result<Vec<ExecutedLimitOrder>> {
    foreign_liquidity_orders
        .into_iter()
        .map(|liquidity| {
            let converted = order_converter.normalize_limit_order(Order {
                metadata: OrderMetadata {
                    owner: liquidity.order.from,
                    full_fee_amount: liquidity.order.data.fee_amount,
                    // All foreign orders **MUST** be liquidity, this is
                    // important so they cannot be used to affect the objective.
                    class: OrderClass::Liquidity,
                    // Not needed for encoding but nice to have for logs and competition info.
                    uid: liquidity.order.data.uid(domain, &liquidity.order.from),
                    // These remaining fields do not seem to be used at all for order
                    // encoding, so we just use the default values.
                    ..Default::default()
                },
                data: liquidity.order.data,
                signature: liquidity.order.signature,
                interactions: liquidity.order.interactions,
            })?;
            Ok(ExecutedLimitOrder {
                order: converted,
                executed_sell_amount: liquidity.exec_sell_amount,
                executed_buy_amount: liquidity.exec_buy_amount,
                exec_plan: None,
            })
        })
        .collect()
}

fn match_prepared_and_settled_amms(
    prepared_amms: &[Liquidity],
    settled_amms: HashMap<H160, UpdatedAmmModel>,
) -> Result<Vec<ExecutedAmm>> {
    let prepared_amms: HashMap<H160, &Liquidity> = prepared_amms
        .iter()
        .filter_map(|amm| amm.address().map(|address| (address, amm)))
        .collect();
    settled_amms
        .into_iter()
        .filter(|(_, settled)| settled.is_non_trivial())
        .flat_map(|(address, settled)| {
            settled
                .execution
                .into_iter()
                .map(move |exec| (address, exec))
        })
        .map(|(address, settled)| {
            Ok(ExecutedAmm {
                order: prepared_amms
                    .get(&address)
                    .copied()
                    .ok_or_else(|| anyhow!("Invalid AMM {}", address))?
                    .clone(),
                input: TokenAmount {
                    token: settled.buy_token,
                    amount: settled.exec_buy_amount,
                },
                output: TokenAmount {
                    token: settled.sell_token,
                    amount: settled.exec_sell_amount,
                },
                exec_plan: settled.exec_plan,
            })
        })
        .collect()
}

fn merge_and_order_executions(
    executions_amms: Vec<ExecutedAmm>,
    interactions: Vec<InteractionData>,
    orders: Vec<ExecutedLimitOrder>,
) -> Vec<Execution> {
    let mut executions: Vec<_> = executions_amms
        .into_iter()
        .map(|amm| Execution::Amm(Box::new(amm)))
        .chain(
            interactions
                .into_iter()
                .map(|interaction| Execution::CustomInteraction(Box::new(interaction))),
        )
        .chain(
            orders
                .into_iter()
                .map(|order| Execution::LimitOrder(Box::new(order))),
        )
        .collect();
    // executions with optional execution plan will be executed first
    executions.sort_by_key(|execution| execution.coordinates());
    executions
}

fn match_settled_prices(
    executed_limit_orders: &[ExecutedLimitOrder],
    solver_prices: HashMap<H160, U256>,
) -> Result<HashMap<H160, U256>> {
    let mut prices = HashMap::new();
    let executed_tokens = executed_limit_orders
        .iter()
        .flat_map(|order| match order.order.id {
            LimitOrderId::Market(_) | LimitOrderId::Limit(_) => {
                vec![order.order.buy_token, order.order.sell_token]
            }
            LimitOrderId::Liquidity(_) => vec![],
        });
    for token in executed_tokens {
        if let Entry::Vacant(entry) = prices.entry(token) {
            let price = solver_prices
                .get(&token)
                .ok_or_else(|| anyhow!("invalid token {}", token))?;
            entry.insert(*price);
        }
    }
    Ok(prices)
}

async fn compute_approvals(
    allowance_manager: Arc<dyn AllowanceManaging>,
    approvals: Vec<ApprovalModel>,
) -> Result<Vec<Approval>> {
    if approvals.is_empty() {
        return Ok(Vec::new());
    }

    let requests = approvals
        .into_iter()
        .try_fold(HashMap::new(), |mut grouped, approval| {
            let amount = grouped
                .entry((approval.token, approval.spender))
                .or_insert(U256::zero());
            *amount = amount
                .checked_add(approval.amount)
                .context("overflow when computing total approval amount")?;

            Result::<_>::Ok(grouped)
        })?
        .into_iter()
        .map(|((token, spender), amount)| ApprovalRequest {
            token,
            spender,
            amount,
        })
        .collect::<Vec<_>>();

    allowance_manager.get_approvals(&requests).await
}

/// Check if executions contain execution plans with the same coordinates
fn duplicate_coordinates(executions: &[Execution]) -> bool {
    let mut coordinates = HashSet::new();
    executions.iter().any(|execution| {
        execution
            .coordinates()
            .map(|coordinate| !coordinates.insert(coordinate))
            .unwrap_or(false)
    })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            interactions::allowances::MockAllowanceManaging,
            liquidity::{
                tests::CapturingSettlementHandler,
                ConstantProductOrder,
                LiquidityOrderId,
                StablePoolOrder,
                WeightedProductOrder,
            },
            settlement::{PricedTrade, Trade},
        },
        hex_literal::hex,
        maplit::hashmap,
        model::{
            order::{OrderData, OrderUid},
            signature::Signature,
            TokenPair,
        },
        num::{rational::Ratio, BigRational},
        shared::sources::balancer_v2::{
            pool_fetching::{AmplificationParameter, TokenState, WeightedTokenState},
            swap::fixed_point::Bfp,
        },
    };

    #[tokio::test]
    async fn convert_settlement_() {
        let weth = H160([0xe7; 20]);

        let t0 = H160::zero();
        let t1 = H160::from_low_u64_be(1);

        let limit_handler = CapturingSettlementHandler::arc();
        let orders = vec![LimitOrder {
            sell_token: t0,
            buy_token: t1,
            sell_amount: 1.into(),
            buy_amount: 2.into(),
            kind: OrderKind::Sell,
            settlement_handling: limit_handler.clone(),
            id: 0.into(),
            ..Default::default()
        }];

        let cp_amm_handler = CapturingSettlementHandler::arc();
        let internal_amm_handler = CapturingSettlementHandler::arc();
        let wp_amm_handler = CapturingSettlementHandler::arc();
        let sp_amm_handler = CapturingSettlementHandler::arc();
        let liquidity = vec![
            Liquidity::ConstantProduct(ConstantProductOrder {
                address: H160::from_low_u64_be(0),
                tokens: TokenPair::new(t0, t1).unwrap(),
                reserves: (3, 4),
                fee: 5.into(),
                settlement_handling: cp_amm_handler.clone(),
            }),
            Liquidity::ConstantProduct(ConstantProductOrder {
                address: H160::from_low_u64_be(1),
                tokens: TokenPair::new(t0, t1).unwrap(),
                reserves: (6, 7),
                fee: 8.into(),
                settlement_handling: internal_amm_handler.clone(),
            }),
            Liquidity::BalancerWeighted(WeightedProductOrder {
                address: H160::from_low_u64_be(2),
                reserves: hashmap! {
                    t0 => WeightedTokenState {
                        common: TokenState {
                            balance: U256::from(200),
                            scaling_exponent: 4,
                        },
                        weight: Bfp::from(200_000_000_000_000_000),
                    },
                    t1 => WeightedTokenState {
                        common: TokenState {
                            balance: U256::from(800),
                            scaling_exponent: 6,
                        },
                        weight: Bfp::from(800_000_000_000_000_000),
                    }
                },
                fee: "0.03".parse().unwrap(),
                settlement_handling: wp_amm_handler.clone(),
            }),
            Liquidity::BalancerStable(StablePoolOrder {
                address: H160::from_low_u64_be(3),
                reserves: hashmap! {
                    t0 => TokenState {
                        balance: U256::from(300),
                        scaling_exponent: 0,
                    },
                    t1 => TokenState {
                        balance: U256::from(400),
                        scaling_exponent: 0,
                    },
                },
                fee: BigRational::new(3.into(), 1.into()),
                amplification_parameter: AmplificationParameter::new(1.into(), 1.into()).unwrap(),
                settlement_handling: sp_amm_handler.clone(),
            }),
        ];

        let executed_order = ExecutedOrderModel {
            exec_buy_amount: 6.into(),
            exec_sell_amount: 7.into(),
            cost: Default::default(),
            fee: Default::default(),
            exec_plan: None,
        };
        let foreign_liquidity_order = ExecutedLiquidityOrderModel {
            order: NativeLiquidityOrder {
                from: H160([99; 20]),
                data: OrderData {
                    sell_token: t1,
                    buy_token: t0,
                    sell_amount: 101.into(),
                    buy_amount: 102.into(),
                    fee_amount: 42.into(),
                    valid_to: u32::MAX,
                    kind: OrderKind::Sell,
                    ..Default::default()
                },
                signature: Signature::PreSign,
                interactions: Default::default(),
            },
            exec_sell_amount: 101.into(),
            exec_buy_amount: 102.into(),
        };
        let foreign_liquidity_order_uid = foreign_liquidity_order
            .order
            .data
            .uid(&Default::default(), &foreign_liquidity_order.order.from);
        let updated_uniswap = UpdatedAmmModel {
            execution: vec![ExecutedAmmModel {
                sell_token: t1,
                buy_token: t0,
                exec_sell_amount: U256::from(9),
                exec_buy_amount: U256::from(8),
                exec_plan: ExecutionPlan {
                    coordinates: ExecutionPlanCoordinatesModel {
                        sequence: 0,
                        position: 0,
                    },
                    internal: false,
                },
            }],
            cost: Default::default(),
        };
        let internal_uniswap = UpdatedAmmModel {
            execution: vec![ExecutedAmmModel {
                sell_token: t1,
                buy_token: t0,
                exec_sell_amount: U256::from(1),
                exec_buy_amount: U256::from(1),
                exec_plan: ExecutionPlan {
                    coordinates: ExecutionPlanCoordinatesModel {
                        sequence: 1,
                        position: 0,
                    },
                    internal: true,
                },
            }],
            cost: Default::default(),
        };
        let updated_balancer_weighted = UpdatedAmmModel {
            execution: vec![ExecutedAmmModel {
                sell_token: t1,
                buy_token: t0,
                exec_sell_amount: U256::from(2),
                exec_buy_amount: U256::from(1),
                exec_plan: ExecutionPlan {
                    coordinates: ExecutionPlanCoordinatesModel {
                        sequence: 2,
                        position: 0,
                    },
                    internal: false,
                },
            }],
            cost: Default::default(),
        };
        let updated_balancer_stable = UpdatedAmmModel {
            execution: vec![ExecutedAmmModel {
                sell_token: t1,
                buy_token: t0,
                exec_sell_amount: U256::from(6),
                exec_buy_amount: U256::from(4),
                exec_plan: ExecutionPlan {
                    coordinates: ExecutionPlanCoordinatesModel {
                        sequence: 3,
                        position: 0,
                    },
                    internal: false,
                },
            }],
            cost: Default::default(),
        };
        let settled = SettledBatchAuctionModel {
            orders: hashmap! { 0 => executed_order },
            foreign_liquidity_orders: vec![foreign_liquidity_order],
            amms: hashmap! {
                H160::from_low_u64_be(0) => updated_uniswap,
                H160::from_low_u64_be(1) => internal_uniswap,
                H160::from_low_u64_be(2) => updated_balancer_weighted,
                H160::from_low_u64_be(3) => updated_balancer_stable,
            },
            ref_token: Some(t0),
            prices: hashmap! { t0 => 10.into(), t1 => 11.into() },
            ..Default::default()
        };

        let prepared = SettlementContext { orders, liquidity };

        let settlement = convert_settlement(
            settled,
            &prepared,
            Arc::new(MockAllowanceManaging::new()),
            Arc::new(OrderConverter::test(weth)),
            SlippageContext::default(),
            &Default::default(),
        )
        .await
        .unwrap();
        assert_eq!(
            settlement.clearing_prices(),
            &hashmap! { t0 => 10.into(), t1 => 11.into() }
        );

        assert_eq!(
            settlement.encoder.all_trades().collect::<Vec<_>>(),
            [PricedTrade {
                data: &Trade {
                    order: Order {
                        metadata: OrderMetadata {
                            owner: H160([99; 20]),
                            full_fee_amount: 42.into(),
                            class: OrderClass::Liquidity,
                            uid: foreign_liquidity_order_uid,
                            ..Default::default()
                        },
                        data: OrderData {
                            sell_token: t1,
                            buy_token: t0,
                            sell_amount: 101.into(),
                            buy_amount: 102.into(),
                            fee_amount: 42.into(),
                            valid_to: u32::MAX,
                            kind: OrderKind::Sell,
                            ..Default::default()
                        },
                        signature: Signature::PreSign,
                        ..Default::default()
                    },
                    executed_amount: 101.into(),
                    solver_fee: 0.into(),
                },
                sell_token_price: 102.into(),
                buy_token_price: 101.into(),
            }]
        );

        assert_eq!(limit_handler.calls(), vec![7.into()]);
        assert_eq!(
            cp_amm_handler.calls(),
            vec![AmmOrderExecution {
                input_max: TokenAmount {
                    token: t0,
                    amount: 9.into()
                },
                output: TokenAmount {
                    token: t1,
                    amount: 9.into()
                },
                internalizable: false
            }]
        );
        assert_eq!(
            internal_amm_handler.calls(),
            vec![AmmOrderExecution {
                input_max: TokenAmount {
                    token: t0,
                    amount: 2.into()
                },
                output: TokenAmount {
                    token: t1,
                    amount: 1.into()
                },
                internalizable: true
            }]
        );
        assert_eq!(
            wp_amm_handler.calls(),
            vec![AmmOrderExecution {
                input_max: TokenAmount::new(t0, 2),
                output: TokenAmount::new(t1, 2),
                internalizable: false
            }]
        );
        assert_eq!(
            sp_amm_handler.calls(),
            vec![AmmOrderExecution {
                input_max: TokenAmount::new(t0, 5),
                output: TokenAmount::new(t1, 6),
                internalizable: false
            }]
        );
    }

    #[test]
    fn match_prepared_and_settled_amms_() {
        let token_a = H160::from_slice(&hex!("a7d1c04faf998f9161fc9f800a99a809b84cfc9d"));
        let token_b = H160::from_slice(&hex!("c778417e063141139fce010982780140aa0cd5ab"));
        let token_c = H160::from_slice(&hex!("e4b9895e638f54c3bee2a3a78d6a297cc03e0353"));

        let cpo_0 = ConstantProductOrder {
            address: H160::from_low_u64_be(0),
            tokens: TokenPair::new(token_a, token_b).unwrap(),
            reserves: (597249810824827988770940, 225724246562756585230),
            fee: Ratio::new(3, 1000),
            settlement_handling: CapturingSettlementHandler::arc(),
        };
        let cpo_1 = ConstantProductOrder {
            address: H160::from_low_u64_be(1),
            tokens: TokenPair::new(token_b, token_c).unwrap(),
            reserves: (8488677530563931705, 75408146511005299032),
            fee: Ratio::new(3, 1000),
            settlement_handling: CapturingSettlementHandler::arc(),
        };

        let lo_1 = LimitOrder {
            id: crate::liquidity::LimitOrderId::Liquidity(LiquidityOrderId::Protocol(
                OrderUid::from_integer(1),
            )),
            sell_token: token_a,
            buy_token: token_a,
            sell_amount: U256::from(996570293625199060u128),
            buy_amount: U256::from(289046068204476404625u128),
            kind: OrderKind::Buy,
            partially_fillable: false,
            settlement_handling: CapturingSettlementHandler::arc(),
            exchange: crate::liquidity::Exchange::ZeroEx,
            ..Default::default()
        };

        let wpo = WeightedProductOrder {
            address: H160::from_low_u64_be(2),
            reserves: hashmap! {
                token_c => WeightedTokenState {
                    common: TokenState {
                        balance: U256::from(1251682293173877359u128),
                        scaling_exponent: 0,
                    },
                    weight: Bfp::from(500_000_000_000_000_000),
                },
                token_b => WeightedTokenState {
                    common: TokenState {
                        balance: U256::from(799086982149629058u128),
                        scaling_exponent: 0,
                    },
                    weight: Bfp::from(500_000_000_000_000_000),
                }
            },
            fee: "0.001".parse().unwrap(),
            settlement_handling: CapturingSettlementHandler::arc(),
        };

        let spo = StablePoolOrder {
            address: H160::from_low_u64_be(3),
            reserves: hashmap! {
                token_c => TokenState {
                    balance: U256::from(1234u128),
                    scaling_exponent: 0
                },
                token_b => TokenState {
                    balance: U256::from(5678u128),
                    scaling_exponent: 0
                },
            },
            fee: BigRational::new(1.into(), 1000.into()),
            amplification_parameter: AmplificationParameter::new(1.into(), 1.into()).unwrap(),
            settlement_handling: CapturingSettlementHandler::arc(),
        };

        let liquidity = vec![
            Liquidity::ConstantProduct(cpo_0.clone()),
            Liquidity::ConstantProduct(cpo_1.clone()),
            Liquidity::LimitOrder(lo_1),
            Liquidity::BalancerWeighted(wpo.clone()),
            Liquidity::BalancerStable(spo.clone()),
        ];
        let solution_response = serde_json::from_str::<SettledBatchAuctionModel>(
            r#"{
            "ref_token": "0xc778417e063141139fce010982780140aa0cd5ab",
            "tokens": {
                "0xa7d1c04faf998f9161fc9f800a99a809b84cfc9d": {
                    "decimals": 18,
                    "estimated_price": "377939419103409",
                    "normalize_priority": "0"
                },
                "0xc778417e063141139fce010982780140aa0cd5ab": {
                    "decimals": 18,
                    "estimated_price": "1000000000000000000",
                    "normalize_priority": "1"
                },
                "0xe4b9895e638f54c3bee2a3a78d6a297cc03e0353": {
                    "decimals": 18,
                    "estimated_price": "112874952666826941",
                    "normalize_priority": "0"
                }
            },
            "prices": {
                "0xa7d1c04faf998f9161fc9f800a99a809b84cfc9d": "379669381779741",
                "0xc778417e063141139fce010982780140aa0cd5ab": "1000000000000000000",
                "0xe4b9895e638f54c3bee2a3a78d6a297cc03e0353": "355227837551346618"
            },
            "orders": {
                "0": {
                    "sell_token": "0xe4b9895e638f54c3bee2a3a78d6a297cc03e0353",
                    "buy_token": "0xa7d1c04faf998f9161fc9f800a99a809b84cfc9d",
                    "sell_amount": "996570293625199060",
                    "buy_amount": "289046068204476404625",
                    "allow_partial_fill": false,
                    "is_sell_order": true,
                    "fee": {
                        "token": "0xe4b9895e638f54c3bee2a3a78d6a297cc03e0353",
                        "amount": "3429706374800940"
                    },
                    "cost": {
                        "token": "0xc778417e063141139fce010982780140aa0cd5ab",
                        "amount": "98173121900550"
                    },
                    "exec_sell_amount": "996570293625199060",
                    "exec_buy_amount": "932415220613609833982"
                }
            },
            "amms": {
                "0x0000000000000000000000000000000000000000": {
                    "kind": "ConstantProduct",
                    "reserves": {
                        "0xa7d1c04faf998f9161fc9f800a99a809b84cfc9d": "597249810824827988770940",
                        "0xc778417e063141139fce010982780140aa0cd5ab": "225724246562756585230"
                    },
                    "fee": "0.003",
                    "cost": {
                        "token": "0xc778417e063141139fce010982780140aa0cd5ab",
                        "amount": "140188523735120"
                    },
                    "execution": [
                        {
                            "sell_token": "0xa7d1c04faf998f9161fc9f800a99a809b84cfc9d",
                            "buy_token": "0xc778417e063141139fce010982780140aa0cd5ab",
                            "exec_sell_amount": "932415220613609833982",
                            "exec_buy_amount": "354009510372389956",
                            "exec_plan": {
                                "sequence": 0,
                                "position": 1,
                                "internal": false
                            }
                        }
                    ]
                },
                "0x0000000000000000000000000000000000000001": {
                    "execution": [
                        {
                            "sell_token": "0xc778417e063141139fce010982780140aa0cd5ab",
                            "buy_token": "0xe4b9895e638f54c3bee2a3a78d6a297cc03e0353",
                            "exec_sell_amount": "1",
                            "exec_buy_amount": "2",
                            "exec_plan": {
                                "sequence": 0,
                                "position": 2,
                                "internal": false
                            }
                        }
                    ]
                },
                "0x0000000000000000000000000000000000000002": {
                    "kind": "WeightedProduct",
                    "reserves": {
                        "0xe4b9895e638f54c3bee2a3a78d6a297cc03e0353": {
                            "balance": "1251682293173877359",
                            "weight": "0.5"
                        },
                        "0xc778417e063141139fce010982780140aa0cd5ab": {
                            "balance": "799086982149629058",
                            "weight": "0.5"
                        }
                    },
                    "fee": "0.001",
                    "cost": {
                        "token": "0xc778417e063141139fce010982780140aa0cd5ab",
                        "amount": "177648716400000"
                    },
                    "execution": [
                        {
                            "sell_token": "0xc778417e063141139fce010982780140aa0cd5ab",
                            "buy_token": "0xe4b9895e638f54c3bee2a3a78d6a297cc03e0353",
                            "exec_sell_amount": "354009510372384890",
                            "exec_buy_amount": "996570293625184642",
                            "exec_plan": {
                                "sequence": 0,
                                "position": 0,
                                "internal": false
                            }
                        }
                    ]
                },
                "0x0000000000000000000000000000000000000003": {
                    "kind": "Stable",
                    "reserves": {
                        "0xe4b9895e638f54c3bee2a3a78d6a297cc03e0353": "1234",
                        "0xc778417e063141139fce010982780140aa0cd5ab": "5678"
                    },
                    "fee": "0.001",
                    "cost": {
                        "token": "0xc778417e063141139fce010982780140aa0cd5ab",
                        "amount": "1771"
                    },
                    "execution": [
                        {
                            "sell_token": "0xc778417e063141139fce010982780140aa0cd5ab",
                            "buy_token": "0xe4b9895e638f54c3bee2a3a78d6a297cc03e0353",
                            "exec_sell_amount": "3",
                            "exec_buy_amount": "4",
                            "exec_plan": {
                                "sequence": 0,
                                "position": 3,
                                "internal": false
                            }
                        }
                    ]
                }
            },
            "solver": {
                "name": "standard",
                "args": [
                    "--write_auxiliary_files",
                    "--solver",
                    "SCIP",
                    "--output_dir",
                    "/app/results"
                ],
                "runtime": 0.0,
                "runtime_preprocessing": 17.097073793411255,
                "runtime_solving": 123.31747031211853,
                "runtime_ring_finding": 0.0,
                "runtime_validation": 0.14400219917297363,
                "nr_variables": 24,
                "nr_bool_variables": 8,
                "optimality_gap": null,
                "solver_status": "ok",
                "termination_condition": "optimal",
                "exit_status": "completed"
            }
            }"#,
        )
        .unwrap();

        let amms = match_prepared_and_settled_amms(&liquidity, solution_response.amms).unwrap();
        let executions = merge_and_order_executions(amms, vec![], vec![]);
        assert_eq!(
            executions,
            vec![
                Execution::Amm(Box::new(ExecutedAmm {
                    order: Liquidity::BalancerWeighted(wpo),
                    input: TokenAmount::new(token_c, 996570293625184642u128),
                    output: TokenAmount::new(token_b, 354009510372384890u128),
                    exec_plan: ExecutionPlan {
                        coordinates: ExecutionPlanCoordinatesModel {
                            sequence: 0,
                            position: 0,
                        },
                        internal: false,
                    }
                })),
                Execution::Amm(Box::new(ExecutedAmm {
                    order: Liquidity::ConstantProduct(cpo_0),
                    input: TokenAmount::new(token_b, 354009510372389956u128),
                    output: TokenAmount::new(token_a, 932415220613609833982u128),
                    exec_plan: ExecutionPlan {
                        coordinates: ExecutionPlanCoordinatesModel {
                            sequence: 0,
                            position: 1,
                        },
                        internal: false,
                    }
                })),
                Execution::Amm(Box::new(ExecutedAmm {
                    order: Liquidity::ConstantProduct(cpo_1),
                    input: TokenAmount::new(token_c, 2),
                    output: TokenAmount::new(token_b, 1),
                    exec_plan: ExecutionPlan {
                        coordinates: ExecutionPlanCoordinatesModel {
                            sequence: 0,
                            position: 2,
                        },
                        internal: false,
                    }
                })),
                Execution::Amm(Box::new(ExecutedAmm {
                    order: Liquidity::BalancerStable(spo),
                    input: TokenAmount::new(token_c, 4),
                    output: TokenAmount::new(token_b, 3),
                    exec_plan: ExecutionPlan {
                        coordinates: ExecutionPlanCoordinatesModel {
                            sequence: 0,
                            position: 3,
                        },
                        internal: false,
                    }
                })),
            ],
        );
    }

    #[test]
    fn merge_and_order_executions_() {
        let token_a = H160::from_slice(&hex!("a7d1c04faf998f9161fc9f800a99a809b84cfc9d"));
        let token_b = H160::from_slice(&hex!("c778417e063141139fce010982780140aa0cd5ab"));

        let cpo_1 = ConstantProductOrder {
            address: H160::from_low_u64_be(1),
            tokens: TokenPair::new(token_a, token_b).unwrap(),
            reserves: (8488677530563931705, 75408146511005299032),
            fee: Ratio::new(3, 1000),
            settlement_handling: CapturingSettlementHandler::arc(),
        };
        let executions_amms = vec![ExecutedAmm {
            order: Liquidity::ConstantProduct(cpo_1),
            input: TokenAmount::new(token_a, 2),
            output: TokenAmount::new(token_b, 1),
            exec_plan: ExecutionPlan {
                coordinates: ExecutionPlanCoordinatesModel {
                    sequence: 1u32,
                    position: 2u32,
                },
                internal: false,
            },
        }];
        let interactions = vec![InteractionData {
            target: H160::zero(),
            value: U256::zero(),
            call_data: Vec::new(),
            inputs: vec![],
            outputs: vec![],
            exec_plan: Some(ExecutionPlan {
                coordinates: ExecutionPlanCoordinatesModel {
                    sequence: 1u32,
                    position: 1u32,
                },
                internal: false,
            }),
            cost: None,
        }];
        let orders = vec![ExecutedLimitOrder {
            order: Default::default(),
            executed_buy_amount: U256::zero(),
            executed_sell_amount: U256::zero(),
            exec_plan: None,
        }];
        let merged_executions = merge_and_order_executions(
            executions_amms.clone(),
            interactions.clone(),
            orders.clone(),
        );
        assert_eq!(3, merged_executions.len());
        assert!(
            matches!(&merged_executions[0], Execution::LimitOrder(order) if order.as_ref() == &orders[0])
        );
        assert!(
            matches!(&merged_executions[1], Execution::CustomInteraction(interaction) if interaction.as_ref() == &interactions[0])
        );
        assert!(
            matches!(&merged_executions[2], Execution::Amm(amm) if amm.as_ref() == &executions_amms[0])
        );
    }

    #[tokio::test]
    pub async fn compute_approvals_groups_approvals_by_spender_and_token() {
        let mut allowance_manager = MockAllowanceManaging::new();
        allowance_manager
            .expect_get_approvals()
            .withf(|requests| {
                // deal with underterministic ordering because of grouping
                // implementation.
                let grouped = requests
                    .iter()
                    .map(|request| ((request.token, request.spender), request.amount))
                    .collect::<HashMap<_, _>>();

                requests.len() == grouped.len()
                    && grouped
                        == hashmap! {
                            (H160([1; 20]), H160([0xf1; 20])) => U256::from(12),
                            (H160([1; 20]), H160([0xf2; 20])) => U256::from(3),
                            (H160([2; 20]), H160([0xf1; 20])) => U256::from(4),
                            (H160([2; 20]), H160([0xf2; 20])) => U256::from(5),
                        }
            })
            .returning(|_| Ok(Vec::new()));

        assert_eq!(
            compute_approvals(
                Arc::new(allowance_manager),
                vec![
                    ApprovalModel {
                        token: H160([1; 20]),
                        spender: H160([0xf1; 20]),
                        amount: 10.into()
                    },
                    ApprovalModel {
                        token: H160([1; 20]),
                        spender: H160([0xf2; 20]),
                        amount: 3.into(),
                    },
                    ApprovalModel {
                        token: H160([1; 20]),
                        spender: H160([0xf1; 20]),
                        amount: 2.into(),
                    },
                    ApprovalModel {
                        token: H160([2; 20]),
                        spender: H160([0xf1; 20]),
                        amount: 4.into(),
                    },
                    ApprovalModel {
                        token: H160([2; 20]),
                        spender: H160([0xf2; 20]),
                        amount: 5.into(),
                    },
                ],
            )
            .await
            .unwrap(),
            Vec::new(),
        );
    }

    #[tokio::test]
    pub async fn compute_approvals_errors_on_overflow() {
        assert!(compute_approvals(
            Arc::new(MockAllowanceManaging::new()),
            vec![
                ApprovalModel {
                    token: H160([1; 20]),
                    spender: H160([2; 20]),
                    amount: U256::MAX,
                },
                ApprovalModel {
                    token: H160([1; 20]),
                    spender: H160([2; 20]),
                    amount: 1.into(),
                },
            ],
        )
        .await
        .is_err());
    }

    fn interaction_with_coordinate(
        coordinates: Option<ExecutionPlanCoordinatesModel>,
    ) -> Execution {
        Execution::CustomInteraction(Box::new(InteractionData {
            exec_plan: coordinates.map(|coordinates| ExecutionPlan {
                coordinates,
                ..Default::default()
            }),
            ..Default::default()
        }))
    }

    #[test]
    pub fn duplicate_coordinates_false() {
        let executions = vec![
            interaction_with_coordinate(None),
            interaction_with_coordinate(Some(ExecutionPlanCoordinatesModel {
                sequence: 0,
                position: 0,
            })),
            interaction_with_coordinate(Some(ExecutionPlanCoordinatesModel {
                sequence: 0,
                position: 1,
            })),
        ];
        assert!(!duplicate_coordinates(&executions));
    }

    #[test]
    pub fn duplicate_coordinates_true() {
        let executions = vec![
            interaction_with_coordinate(None),
            interaction_with_coordinate(Some(ExecutionPlanCoordinatesModel {
                sequence: 0,
                position: 0,
            })),
            interaction_with_coordinate(Some(ExecutionPlanCoordinatesModel {
                sequence: 0,
                position: 0,
            })),
        ];
        assert!(duplicate_coordinates(&executions));
    }
}
