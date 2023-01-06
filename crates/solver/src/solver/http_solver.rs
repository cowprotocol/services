pub mod buffers;
pub mod settlement;

use self::settlement::SettlementContext;
use crate::{
    interactions::allowances::AllowanceManaging,
    liquidity::{
        order_converter::OrderConverter, slippage::SlippageCalculator, Exchange, LimitOrder,
        Liquidity,
    },
    s3_instance_upload::S3InstanceUploader,
    settlement::{external_prices::ExternalPrices, Settlement},
    solver::{http_solver::settlement::ConversionError, Auction, Solver},
};
use anyhow::{Context, Result};
use buffers::{BufferRetrievalError, BufferRetrieving};
use ethcontract::{errors::ExecutionError, Account, U256};
use futures::{join, lock::Mutex};
use itertools::{Either, Itertools as _};
use maplit::{btreemap, hashset};
use model::{auction::AuctionId, order::OrderKind, DomainSeparator};
use num::{BigInt, BigRational};
use primitive_types::H160;
use shared::{
    http_solver::{gas_model::GasModel, model::*, DefaultHttpSolverApi, HttpSolverApi},
    measure_time,
    sources::balancer_v2::pools::common::compute_scaling_rate,
    token_info::{TokenInfo, TokenInfoFetching},
    token_list::AutoUpdatingTokenList,
};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    iter::FromIterator as _,
    sync::Arc,
    time::Instant,
};

use super::AuctionResult;

// TODO: special rounding for the prices we get from the solver?

/// Data shared between multiple instances of the http solver for the same driver run.
pub struct InstanceData {
    run_id: u64,
    model: BatchAuctionModel,
    context: SettlementContext,
}

/// We keep a cache of per solve instance data because it is the same for all http solver
/// invocations. Without the cache we would duplicate most of the requests to the node.
pub type InstanceCache = Arc<Mutex<Option<InstanceData>>>;

pub struct HttpSolver {
    solver: DefaultHttpSolverApi,
    account: Account,
    allowance_manager: Arc<dyn AllowanceManaging>,
    order_converter: Arc<OrderConverter>,
    instance_cache: InstanceCache,
    filter_non_fee_connected_orders: bool,
    slippage_calculator: SlippageCalculator,
    market_makable_token_list: AutoUpdatingTokenList,
    domain: DomainSeparator,
    instance_uploader: Option<Arc<S3InstanceUploader>>,
}

impl HttpSolver {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        solver: DefaultHttpSolverApi,
        account: Account,
        allowance_manager: Arc<dyn AllowanceManaging>,
        order_converter: Arc<OrderConverter>,
        instance_cache: InstanceCache,
        filter_non_fee_connected_orders: bool,
        slippage_calculator: SlippageCalculator,
        market_makable_token_list: AutoUpdatingTokenList,
        domain: DomainSeparator,
        instance_uploader: Option<S3InstanceUploader>,
    ) -> Self {
        let instance_uploader = instance_uploader.map(Arc::new);
        Self {
            solver,
            account,
            allowance_manager,
            order_converter,
            instance_cache,
            filter_non_fee_connected_orders,
            slippage_calculator,
            market_makable_token_list,
            domain,
            instance_uploader,
        }
    }

        }
    }
}

fn non_bufferable_tokens_used(
    interactions: &[InteractionData],
    market_makable_token_list: &HashSet<H160>,
) -> BTreeSet<H160> {
    interactions
        .iter()
        .filter(|interaction| {
            interaction
                .exec_plan
                .as_ref()
                .map(|plan| plan.internal)
                .unwrap_or_default()
        })
        .flat_map(|interaction| &interaction.inputs)
        .filter(|input| !market_makable_token_list.contains(&input.token))
        .map(|input| input.token)
        .collect()
}

#[async_trait::async_trait]
impl Solver for HttpSolver {
    async fn solve(
        &self,
        Auction {
            id,
            run,
            orders,
            liquidity,
            gas_price,
            deadline,
            external_prices,
            ..
        }: Auction,
    ) -> Result<Vec<Settlement>> {
        if orders.is_empty() {
            return Ok(Vec::new());
        };

        let (model, context) = {
            let mut guard = self.instance_cache.lock().await;
            match guard.as_mut() {
                Some(data) if data.run_id == run => (data.model.clone(), data.context.clone()),
                _ => {
                    let (model, context) = self
                        .prepare_model(
                            id,
                            run,
                            orders,
                            liquidity,
                            gas_price,
                            external_prices.clone(),
                        )
                        .await?;
                    // This can be a large log message so we don't want to log it by default.
                    tracing::trace!(
                        "Problem sent to http solvers (json):\n{}",
                        serde_json::to_string_pretty(&model).unwrap()
                    );
                    *guard = Some(InstanceData {
                        run_id: run,
                        model: model.clone(),
                        context: context.clone(),
                    });
                    (model, context)
                }
            }
        };

        self.upload_instance_in_background(id, &model);

        let timeout = deadline
            .checked_duration_since(Instant::now())
            .context("no time left to send request")?;
        let mut settled = self.solver.solve(&model, timeout).await?;
        settled.add_missing_execution_plans();

        tracing::debug!(
            "Solution received from http solver {} (json):\n{:}",
            self.solver.name,
            serde_json::to_string_pretty(&settled).unwrap()
        );

        // verify solution is not empty
        if settled.orders.is_empty() {
            return Ok(vec![]);
        }

        // verify internal custom interactions return only bufferable tokens to settlement contract
        let non_bufferable_tokens = non_bufferable_tokens_used(
            &settled.interaction_data,
            &self.market_makable_token_list.addresses(),
        );
        if !non_bufferable_tokens.is_empty() {
            tracing::warn!(
                "Solution filtered out for using non bufferable output tokens for solver {}, tokens: {:?}",
                self.solver.name,
                non_bufferable_tokens
            );
            self.notify_auction_result(
                id,
                AuctionResult::Rejected(SolverRejectionReason::NonBufferableTokensUsed(
                    non_bufferable_tokens,
                )),
            );
            return Ok(vec![]);
        }

        let slippage = self.slippage_calculator.context(&external_prices);
        match settlement::convert_settlement(
            settled.clone(),
            context,
            self.allowance_manager.clone(),
            self.order_converter.clone(),
            slippage,
            &self.domain,
        )
        .await
        {
            Ok(settlement) => Ok(vec![settlement]),
            Err(err) => {
                tracing::debug!(
                    name = %self.name(), ?settled, ?err,
                    "failed to process HTTP solver result",
                );
                if matches!(err, ConversionError::InvalidExecutionPlans(_)) {
                    self.notify_auction_result(
                        id,
                        AuctionResult::Rejected(SolverRejectionReason::InvalidExecutionPlans),
                    );
                }
                Err(err.into())
            }
        }
    }

    fn notify_auction_result(&self, auction_id: AuctionId, result: AuctionResult) {
        self.solver.notify_auction_result(auction_id, result);
    }

    fn account(&self) -> &Account {
        &self.account
    }

    fn name(&self) -> &str {
        &self.solver.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        interactions::allowances::MockAllowanceManaging,
        liquidity::{tests::CapturingSettlementHandler, ConstantProductOrder, LimitOrder},
        settlement::external_prices::externalprices,
        solver::http_solver::buffers::MockBufferRetrieving,
    };
    use ::model::TokenPair;
    use ethcontract::Address;
    use maplit::hashmap;
    use num::rational::Ratio;
    use reqwest::Client;
    use shared::{
        http_solver::SolverConfig,
        token_info::{MockTokenInfoFetching, TokenInfo},
    };
    use std::{sync::Arc, time::Duration};

    // cargo test real_solver -- --ignored --nocapture
    // set the env variable GP_V2_OPTIMIZER_URL to use a non localhost optimizer
    #[tokio::test]
    #[ignore]
    async fn real_solver() {
        tracing_subscriber::fmt::fmt()
            .with_env_filter("solver=trace")
            .init();
        let url = std::env::var("GP_V2_OPTIMIZER_URL")
            .unwrap_or_else(|_| "http://localhost:8000".to_string());

        let buy_token = H160::from_low_u64_be(1337);
        let sell_token = H160::from_low_u64_be(43110);

        let mut mock_token_info_fetcher = MockTokenInfoFetching::new();
        mock_token_info_fetcher
            .expect_get_token_infos()
            .return_once(move |_| {
                hashmap! {
                    buy_token => TokenInfo { decimals: Some(18), symbol: Some("CAT".to_string()) },
                    sell_token => TokenInfo { decimals: Some(18), symbol: Some("CAT".to_string()) },
                }
            });

        let mut mock_buffer_retriever = MockBufferRetrieving::new();
        mock_buffer_retriever
            .expect_get_buffers()
            .return_once(move |_| {
                hashmap! {
                    buy_token => Ok(U256::from(42)),
                    sell_token => Ok(U256::from(1337)),
                }
            });

        let gas_price = 100.;

        let solver = HttpSolver::new(
            DefaultHttpSolverApi {
                name: "Test Solver".to_string(),
                network_name: "mock_network_id".to_string(),
                chain_id: 0,
                base: url.parse().unwrap(),
                client: Client::new(),
                config: SolverConfig::default(),
            },
            Account::Local(Address::default(), None),
            H160::zero(),
            Arc::new(mock_token_info_fetcher),
            Arc::new(mock_buffer_retriever),
            Arc::new(MockAllowanceManaging::new()),
            Arc::new(OrderConverter::test(H160([0x42; 20]))),
            Default::default(),
            true,
            SlippageCalculator::default(),
            Default::default(),
            Default::default(),
            None,
        );
        let base = |x: u128| x * 10u128.pow(18);
        let limit_orders = vec![LimitOrder {
            buy_token,
            sell_token,
            buy_amount: base(1).into(),
            sell_amount: base(2).into(),
            kind: OrderKind::Sell,
            id: 0.into(),
            ..Default::default()
        }];
        let liquidity = vec![Liquidity::ConstantProduct(ConstantProductOrder {
            address: H160::from_low_u64_be(1),
            tokens: TokenPair::new(buy_token, sell_token).unwrap(),
            reserves: (base(100), base(100)),
            fee: Ratio::new(0, 1),
            settlement_handling: CapturingSettlementHandler::arc(),
        })];
        let (model, _context) = solver
            .prepare_model(0, 1, limit_orders, liquidity, gas_price, Default::default())
            .await
            .unwrap();
        let settled = solver
            .solver
            .solve(&model, Duration::from_secs(1000))
            .await
            .unwrap();
        dbg!(&settled);

        let exec_order = settled.orders.values().next().unwrap();
        assert_eq!(exec_order.exec_sell_amount.as_u128(), base(2));
        assert!(exec_order.exec_buy_amount.as_u128() > 0);

        let uniswap = settled.amms.values().next().unwrap();
        let execution = &uniswap.execution[0];
        assert!(execution.exec_buy_amount.gt(&U256::zero()));
        assert_eq!(execution.exec_sell_amount, U256::from(base(2)));
        assert_eq!(execution.exec_plan, ExecutionPlan::default());

        assert_eq!(settled.prices.len(), 2);
    }

    #[test]
    fn decode_response() {
        let example_response = r#"
            {
              "extra_crap": ["Hello"],
              "orders": {
                "0": {
                  "sell_token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                  "buy_token": "0xba100000625a3754423978a60c9317c58a424e3d",
                  "sell_amount": "195160000000000000",
                  "buy_amount": "18529625032931383084",
                  "allow_partial_fill": false,
                  "is_sell_order": true,
                  "fee": {
                    "amount": "4840000000000000",
                    "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                  },
                  "cost": {
                    "amount": "1604823000000000",
                    "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                  },
                  "exec_buy_amount": "18689825362370811941",
                  "exec_sell_amount": "195160000000000000"
                },
                "1": {
                  "sell_token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                  "buy_token": "0xba100000625a3754423978a60c9317c58a424e3d",
                  "sell_amount": "395160000000000000",
                  "buy_amount": "37314737669229514851",
                  "allow_partial_fill": false,
                  "is_sell_order": true,
                  "fee": {
                    "amount": "4840000000000000",
                    "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                  },
                  "cost": {
                    "amount": "1604823000000000",
                    "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                  },
                  "exec_buy_amount": "37843161458262200293",
                  "exec_sell_amount": "395160000000000000"
                }
              },
              "ref_token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
              "prices": {
                "0xba100000625a3754423978a60c9317c58a424e3d": "10442045135045813",
                "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": "1000000000000000000"
              },
              "amms": {
                "0x0000000000000000000000000000000000000000": {
                  "kind": "WeightedProduct",
                  "reserves": {
                    "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": {
                      "balance": "99572200495363891220",
                      "weight": "0.5"
                    },
                    "0xba100000625a3754423978a60c9317c58a424e3d": {
                      "balance": "9605600791222732320384",
                      "weight": "0.5"
                    }
                  },
                  "fee": "0.0014",
                  "cost": {
                    "amount": "2904000000000000",
                    "token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                  },
                  "execution": [
                    {
                      "sell_token": "0xba100000625a3754423978a60c9317c58a424e3d",
                      "buy_token": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                      "exec_sell_amount": "56532986820633012234",
                      "exec_buy_amount": "590320000000000032",
                      "exec_plan": {
                        "sequence": 0,
                        "position": 0,
                        "internal": false
                      }
                    }
                  ]
                }
              }
            }
        "#;
        let parsed_response = serde_json::from_str::<SettledBatchAuctionModel>(example_response);
        assert!(parsed_response.is_ok());
    }

    #[test]
    fn non_bufferable_tokens_used_test_all_empty() {
        let interactions = vec![];
        let market_makable_token_list = HashSet::<H160>::new();
        assert_eq!(
            non_bufferable_tokens_used(&interactions, &market_makable_token_list),
            BTreeSet::new()
        );
    }

    // Interaction is internal and it contains only bufferable tokens
    #[test]
    fn non_bufferable_tokens_used_test_ok() {
        let bufferable_token = H160::from_low_u64_be(1);
        let market_makable_token_list = HashSet::from([bufferable_token]);

        let token_amount = TokenAmount {
            token: bufferable_token,
            ..Default::default()
        };

        let interactions = vec![InteractionData {
            inputs: vec![token_amount],
            exec_plan: Some(ExecutionPlan {
                internal: true,
                ..Default::default()
            }),
            ..Default::default()
        }];

        assert_eq!(
            non_bufferable_tokens_used(&interactions, &market_makable_token_list),
            BTreeSet::new()
        );
    }

    // Interaction is internal but it contains non bufferable tokens
    #[test]
    fn non_bufferable_tokens_used_test_not_ok() {
        let non_bufferable_token = H160::from_low_u64_be(1);
        let market_makable_token_list = HashSet::from([]);

        let token_amount = TokenAmount {
            token: non_bufferable_token,
            ..Default::default()
        };

        let interactions = vec![InteractionData {
            inputs: vec![token_amount],
            exec_plan: Some(ExecutionPlan {
                internal: true,
                ..Default::default()
            }),
            ..Default::default()
        }];

        assert_eq!(
            non_bufferable_tokens_used(&interactions, &market_makable_token_list),
            BTreeSet::from([non_bufferable_token])
        );
    }

    // Interaction is **not** internal and it contains non bufferable tokens
    #[test]
    fn non_bufferable_tokens_used_test_ok2() {
        let non_bufferable_token = H160::from_low_u64_be(1);
        let market_makable_token_list = HashSet::from([]);

        let token_amount = TokenAmount {
            token: non_bufferable_token,
            ..Default::default()
        };

        let interactions = vec![InteractionData {
            inputs: vec![token_amount],
            exec_plan: Some(ExecutionPlan {
                internal: false,
                ..Default::default()
            }),
            ..Default::default()
        }];

        assert_eq!(
            non_bufferable_tokens_used(&interactions, &market_makable_token_list),
            BTreeSet::new()
        );
    }
}
