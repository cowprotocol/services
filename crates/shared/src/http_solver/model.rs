use derivative::Derivative;
use ethcontract::{Bytes, H160};
use model::{
    auction::AuctionId,
    order::OrderData,
    ratio_as_decimal,
    signature::Signature,
    u256_decimal::{self, DecimalU256},
};
use num::BigRational;
use primitive_types::U256;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::collections::{BTreeMap, HashMap};

use crate::{
    interaction::{EncodedInteraction, Interaction},
    sources::uniswap_v3::pool_fetching::PoolInfo,
};

#[derive(Clone, Debug, Default, Serialize)]
pub struct BatchAuctionModel {
    pub tokens: BTreeMap<H160, TokenInfoModel>,
    pub orders: BTreeMap<usize, OrderModel>,
    pub amms: BTreeMap<usize, AmmModel>,
    pub metadata: Option<MetadataModel>,
}

#[derive(Clone, Debug, Serialize)]
pub struct OrderModel {
    pub sell_token: H160,
    pub buy_token: H160,
    #[serde(with = "u256_decimal")]
    pub sell_amount: U256,
    #[serde(with = "u256_decimal")]
    pub buy_amount: U256,
    pub allow_partial_fill: bool,
    pub is_sell_order: bool,
    pub fee: TokenAmount,
    pub cost: TokenAmount,
    pub is_liquidity_order: bool,
    #[serde(default)]
    pub mandatory: bool,
    /// Signals if the order will be executed as an atomic unit. In that case the order's
    /// preconditions have to be met for it to be executed successfully. This is different from the
    /// usual user provided orders because those can be batched together and it's only relevant if
    /// the pre- and post conditions are met after the complete batch got executed.
    pub has_atomic_execution: bool,
    /// CIP-14 risk adjusted solver reward
    pub reward: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct AmmModel {
    #[serde(flatten)]
    pub parameters: AmmParameters,
    #[serde(with = "ratio_as_decimal")]
    pub fee: BigRational,
    pub cost: TokenAmount,
    pub mandatory: bool,
    pub address: H160,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind")]
pub enum AmmParameters {
    ConstantProduct(ConstantProductPoolParameters),
    WeightedProduct(WeightedProductPoolParameters),
    Stable(StablePoolParameters),
    Concentrated(ConcentratedPoolParameters),
}

#[serde_as]
#[derive(Clone, Debug, Default, Serialize)]
pub struct ConstantProductPoolParameters {
    #[serde_as(as = "BTreeMap<_, DecimalU256>")]
    pub reserves: BTreeMap<H160, U256>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WeightedPoolTokenData {
    #[serde(with = "u256_decimal")]
    pub balance: U256,
    #[serde(with = "ratio_as_decimal")]
    pub weight: BigRational,
}

#[derive(Clone, Debug, Serialize)]
pub struct WeightedProductPoolParameters {
    pub reserves: BTreeMap<H160, WeightedPoolTokenData>,
}

#[serde_as]
#[derive(Clone, Debug, Serialize)]
pub struct StablePoolParameters {
    #[serde_as(as = "BTreeMap<_, DecimalU256>")]
    pub reserves: BTreeMap<H160, U256>,
    #[serde_as(as = "BTreeMap<_, DecimalU256>")]
    pub scaling_rates: BTreeMap<H160, U256>,
    #[serde(with = "ratio_as_decimal")]
    pub amplification_parameter: BigRational,
}

#[serde_as]
#[derive(Clone, Debug, Default, Serialize)]
pub struct ConcentratedPoolParameters {
    pub pool: PoolInfo,
}

#[serde_as]
#[derive(Clone, Debug, Default, Serialize)]
pub struct TokenInfoModel {
    pub decimals: Option<u8>,
    pub alias: Option<String>,
    pub external_price: Option<f64>,
    pub normalize_priority: Option<u64>,
    #[serde_as(as = "Option<DecimalU256>")]
    pub internal_buffer: Option<U256>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct TokenAmount {
    #[serde(with = "u256_decimal")]
    pub amount: U256,
    pub token: H160,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ApprovalModel {
    pub token: H160,
    pub spender: H160,
    #[serde(with = "u256_decimal")]
    pub amount: U256,
}

#[derive(Clone, Derivative, Deserialize, Eq, PartialEq, Serialize)]
#[derivative(Debug)]
pub struct InteractionData {
    pub target: H160,
    pub value: U256,
    #[derivative(Debug(format_with = "crate::debug_bytes"))]
    #[serde(with = "model::bytes_hex")]
    pub call_data: Vec<u8>,
    /// The input amounts into the AMM interaction - i.e. the amount of tokens
    /// that are expected to be sent from the settlement contract into the AMM
    /// for this calldata.
    ///
    /// `GPv2Settlement -> AMM`
    pub inputs: Vec<TokenAmount>,
    /// The output amounts from the AMM interaction - i.e. the amount of tokens
    /// that are expected to be sent from the AMM into the settlement contract
    /// for this calldata.
    ///
    /// `AMM -> GPv2Settlement`
    pub outputs: Vec<TokenAmount>,
    pub exec_plan: Option<ExecutionPlan>,
    pub cost: Option<TokenAmount>,
}

impl Interaction for InteractionData {
    fn encode(&self) -> Vec<EncodedInteraction> {
        vec![(self.target, self.value, Bytes(self.call_data.clone()))]
    }
}

#[serde_as]
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct SettledBatchAuctionModel {
    pub orders: HashMap<usize, ExecutedOrderModel>,
    #[serde(default)]
    pub foreign_liquidity_orders: Vec<ExecutedLiquidityOrderModel>,
    #[serde(default)]
    pub amms: HashMap<usize, UpdatedAmmModel>,
    pub ref_token: Option<H160>,
    #[serde_as(as = "HashMap<_, DecimalU256>")]
    pub prices: HashMap<H160, U256>,
    #[serde(default)]
    pub approvals: Vec<ApprovalModel>,
    #[serde(default)]
    pub interaction_data: Vec<InteractionData>,
    pub metadata: Option<SettledBatchAuctionMetadataModel>,
}

impl SettledBatchAuctionModel {
    pub fn has_execution_plan(&self) -> bool {
        // Its a bit weird that we expect all entries to contain an execution plan. Could make
        // execution plan required and assert that the vector of execution updates is non-empty
        // - NOTE(nlordell): This was done as a way for the HTTP solvers to say "look, we found
        //   a solution but don't know how to order the AMMs to execute it". I think that we
        //   can, as the parent comment suggests, clean this up and just make the field required.

        // **Intentionally** allow interactions without execution plans.

        self.amms
            .values()
            .flat_map(|u| u.execution.iter().map(|e| &e.exec_plan))
            .all(|ex| ex.is_some())
    }
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct MetadataModel {
    pub environment: Option<String>,
    pub auction_id: Option<AuctionId>,
    pub run_id: Option<u64>,
    pub gas_price: Option<f64>,
    pub native_token: Option<H160>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SettledBatchAuctionMetadataModel {
    pub has_solution: Option<bool>,
    pub result: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ExecutedOrderModel {
    #[serde(with = "u256_decimal")]
    pub exec_sell_amount: U256,
    #[serde(with = "u256_decimal")]
    pub exec_buy_amount: U256,
    pub cost: Option<TokenAmount>,
    pub fee: Option<TokenAmount>,
    // Orders which need to be executed in a specific order have an `exec_plan` (e.g. 0x limit orders)
    pub exec_plan: Option<ExecutionPlan>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct ExecutedLiquidityOrderModel {
    pub order: NativeLiquidityOrder,
    #[serde(with = "u256_decimal")]
    pub exec_sell_amount: U256,
    #[serde(with = "u256_decimal")]
    pub exec_buy_amount: U256,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct NativeLiquidityOrder {
    pub from: H160,
    #[serde(flatten)]
    pub data: OrderData,
    #[serde(flatten)]
    pub signature: Signature,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UpdatedAmmModel {
    /// We ignore additional incoming amm fields we don't need.
    pub execution: Vec<ExecutedAmmModel>,
    pub cost: Option<TokenAmount>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ExecutedAmmModel {
    pub sell_token: H160,
    pub buy_token: H160,
    #[serde(with = "u256_decimal")]
    pub exec_sell_amount: U256,
    #[serde(with = "u256_decimal")]
    pub exec_buy_amount: U256,
    /// The exec plan is allowed to be optional because the http solver isn't always
    /// able to determine and order of execution. That is, solver may have a solution
    /// which it wants to share with the driver even if it couldn't derive an execution plan.
    pub exec_plan: Option<ExecutionPlan>,
}

impl UpdatedAmmModel {
    /// Returns true there is at least one non-zero update.
    pub fn is_non_trivial(&self) -> bool {
        let zero = &U256::zero();
        let has_non_trivial_execution = self
            .execution
            .iter()
            .any(|exec| exec.exec_sell_amount.gt(zero) || exec.exec_buy_amount.gt(zero));
        !self.execution.is_empty() && has_non_trivial_execution
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum ExecutionPlan {
    /// The coordinates at which the interaction should be included within a
    /// settlement.
    Coordinates(ExecutionPlanCoordinatesModel),

    /// The interaction should **not** be included in the settlement as
    /// internal buffers will be used instead.
    #[serde(with = "execution_plan_internal")]
    Internal,
}

/// A module for implementing `serde` (de)serialization for the execution plan
/// enum.
///
/// This is a work-around for untagged enum serialization not supporting empty
/// variants <https://github.com/serde-rs/serde/issues/1560>.
mod execution_plan_internal {
    use super::*;

    #[derive(Deserialize, Serialize)]
    enum Kind {
        #[serde(rename = "internal")]
        Internal,
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<(), D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Kind::deserialize(deserializer)?;
        Ok(())
    }

    pub fn serialize<S>(serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        Kind::Internal.serialize(serializer)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct ExecutionPlanCoordinatesModel {
    pub sequence: u32,
    pub position: u32,
}

#[cfg(test)]
mod tests {
    use crate::sources::uniswap_v3::graph_api::Token;

    use super::*;
    use maplit::btreemap;
    use model::{
        app_id::AppId,
        order::{OrderKind, SellTokenSource},
    };
    use serde_json::json;

    #[test]
    fn updated_amm_model_is_non_trivial() {
        assert!(!UpdatedAmmModel {
            execution: vec![],
            cost: Default::default(),
        }
        .is_non_trivial());

        let trivial_execution_without_plan = ExecutedAmmModel {
            exec_plan: None,
            ..Default::default()
        };

        let trivial_execution_with_plan = ExecutedAmmModel {
            exec_plan: Some(ExecutionPlan::Coordinates(ExecutionPlanCoordinatesModel {
                sequence: 0,
                position: 0,
            })),
            ..Default::default()
        };

        assert!(!UpdatedAmmModel {
            execution: vec![
                trivial_execution_with_plan.clone(),
                trivial_execution_without_plan
            ],
            cost: Default::default(),
        }
        .is_non_trivial());

        let execution_with_sell = ExecutedAmmModel {
            exec_sell_amount: U256::one(),
            ..Default::default()
        };

        let execution_with_buy = ExecutedAmmModel {
            exec_buy_amount: U256::one(),
            ..Default::default()
        };

        assert!(UpdatedAmmModel {
            execution: vec![execution_with_buy.clone()],
            cost: Default::default(),
        }
        .is_non_trivial());

        assert!(UpdatedAmmModel {
            execution: vec![execution_with_sell],
            cost: Default::default(),
        }
        .is_non_trivial());

        assert!(UpdatedAmmModel {
            // One trivial and one non-trivial -> non-trivial
            execution: vec![execution_with_buy, trivial_execution_with_plan],
            cost: Default::default(),
        }
        .is_non_trivial());
    }

    #[test]
    fn model_serialization() {
        let native_token = H160([0xee; 20]);
        let buy_token = H160::from_low_u64_be(1337);
        let sell_token = H160::from_low_u64_be(43110);
        let order_model = OrderModel {
            sell_token,
            buy_token,
            sell_amount: U256::from(1),
            buy_amount: U256::from(2),
            allow_partial_fill: false,
            is_sell_order: true,
            fee: TokenAmount {
                amount: U256::from(2),
                token: sell_token,
            },
            cost: TokenAmount {
                amount: U256::from(1),
                token: native_token,
            },
            is_liquidity_order: false,
            mandatory: false,
            has_atomic_execution: false,
            reward: 3.,
        };
        let constant_product_pool_model = AmmModel {
            parameters: AmmParameters::ConstantProduct(ConstantProductPoolParameters {
                reserves: btreemap! {
                    buy_token => U256::from(100),
                    sell_token => U256::from(200),
                },
            }),
            fee: BigRational::new(3.into(), 1000.into()),
            cost: TokenAmount {
                amount: U256::from(3),
                token: native_token,
            },
            mandatory: false,
            address: H160::from_low_u64_be(1),
        };
        let weighted_product_pool_model = AmmModel {
            parameters: AmmParameters::WeightedProduct(WeightedProductPoolParameters {
                reserves: btreemap! {
                    sell_token => WeightedPoolTokenData {
                        balance: U256::from(808),
                        weight: BigRational::new(2.into(), 10.into()),
                    },
                    buy_token => WeightedPoolTokenData {
                        balance: U256::from(64),
                        weight: BigRational::new(8.into(), 10.into()),
                    }
                },
            }),
            fee: BigRational::new(2.into(), 1000.into()),
            cost: TokenAmount {
                amount: U256::from(2),
                token: native_token,
            },
            mandatory: true,
            address: H160::from_low_u64_be(2),
        };
        let stable_pool_model = AmmModel {
            parameters: AmmParameters::Stable(StablePoolParameters {
                reserves: btreemap! {
                    sell_token => U256::from(1000),
                    buy_token => U256::from(1_001_000_000),
                },
                scaling_rates: btreemap! {
                    sell_token => U256::from(1),
                    buy_token => U256::from(1_000_000),
                },
                amplification_parameter: BigRational::new(1337.into(), 100.into()),
            }),
            fee: BigRational::new(3.into(), 1000.into()),
            cost: TokenAmount {
                amount: U256::from(3),
                token: native_token,
            },
            mandatory: true,
            address: H160::from_low_u64_be(3),
        };
        let concentrated_pool_model = AmmModel {
            parameters: AmmParameters::Concentrated(ConcentratedPoolParameters {
                pool: PoolInfo {
                    address: H160::from_low_u64_be(1),
                    tokens: vec![
                        Token {
                            id: buy_token,
                            decimals: 6,
                        },
                        Token {
                            id: sell_token,
                            decimals: 18,
                        },
                    ],
                    ..Default::default()
                },
            }),
            fee: BigRational::new(3.into(), 1000.into()),
            cost: TokenAmount {
                amount: U256::from(3),
                token: native_token,
            },
            mandatory: false,
            address: H160::from_low_u64_be(4),
        };
        let model = BatchAuctionModel {
            tokens: btreemap! {
                buy_token => TokenInfoModel {
                    decimals: Some(6),
                    alias: Some("CAT".to_string()),
                    external_price: Some(1.2),
                    normalize_priority: Some(1),
                    internal_buffer: Some(U256::from(1337)),
                },
                sell_token => TokenInfoModel {
                    decimals: Some(18),
                    alias: Some("DOG".to_string()),
                    external_price: Some(2345.0),
                    normalize_priority: Some(0),
                    internal_buffer: Some(U256::from(42)),
                }
            },
            orders: btreemap! { 0 => order_model },
            amms: btreemap! {
                0 => constant_product_pool_model,
                1 => weighted_product_pool_model,
                2 => stable_pool_model,
                3 => concentrated_pool_model,
            },
            metadata: Some(MetadataModel {
                environment: Some(String::from("Such Meta")),
                ..Default::default()
            }),
        };

        let result = serde_json::to_value(&model).unwrap();

        let expected = json!({
          "tokens": {
            "0x0000000000000000000000000000000000000539": {
              "decimals": 6,
              "alias": "CAT",
              "external_price": 1.2,
              "normalize_priority": 1,
              "internal_buffer": "1337",
            },
            "0x000000000000000000000000000000000000a866": {
              "decimals": 18,
              "alias": "DOG",
              "external_price": 2345.0,
              "normalize_priority": 0,
              "internal_buffer": "42",
            },
          },
          "orders": {
            "0": {
              "sell_token": "0x000000000000000000000000000000000000a866",
              "buy_token": "0x0000000000000000000000000000000000000539",
              "sell_amount": "1",
              "buy_amount": "2",
              "allow_partial_fill": false,
              "is_sell_order": true,
              "fee": {
                "amount": "2",
                "token": "0x000000000000000000000000000000000000a866",
              },
              "is_liquidity_order": false,
              "cost": {
                "amount": "1",
                "token": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
              },
              "mandatory": false,
              "has_atomic_execution": false,
              "reward": 3.0
            },
          },
          "amms": {
            "0": {
              "kind": "ConstantProduct",
              "reserves": {
                "0x000000000000000000000000000000000000a866": "200",
                "0x0000000000000000000000000000000000000539": "100"
              },
              "fee": "0.003",
              "cost": {
                "amount": "3",
                "token": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
              },
              "mandatory": false,
              "address": "0x0000000000000000000000000000000000000001",
            },
            "1": {
              "kind": "WeightedProduct",
              "reserves": {
                "0x000000000000000000000000000000000000a866": {
                    "balance": "808",
                    "weight": "0.2"
                },
                "0x0000000000000000000000000000000000000539": {
                    "balance": "64",
                    "weight": "0.8"
                },
              },
              "fee": "0.002",
              "cost": {
                "amount": "2",
                "token": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
              },
              "mandatory": true,
              "address": "0x0000000000000000000000000000000000000002",
            },
            "2": {
              "kind": "Stable",
              "reserves": {
                "0x000000000000000000000000000000000000a866": "1000",
                "0x0000000000000000000000000000000000000539": "1001000000",
              },
              "scaling_rates": {
                "0x000000000000000000000000000000000000a866": "1",
                "0x0000000000000000000000000000000000000539": "1000000",
              },
              "amplification_parameter": "13.37",
              "fee": "0.003",
              "cost": {
                "amount": "3",
                "token": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
              },
              "mandatory": true,
              "address": "0x0000000000000000000000000000000000000003",
            },
            "3": {
              "kind": "Concentrated",
              "pool": {
                 "tokens": [
                {
                  "id": "0x0000000000000000000000000000000000000539",
                  "decimals": "6",
                },
                {
                  "id": "0x000000000000000000000000000000000000a866",
                  "decimals": "18",
                }
                ],
              "state": {
                "sqrt_price": "0",
                "liquidity": "0",
                "tick": "0",
                "liquidity_net": {},
              },
              "gas_stats": {
                "mean": "0",
              }
              },
              "fee": "0.003",
              "cost": {
                "amount": "3",
                "token": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
              },
              "mandatory": false,
              "address": "0x0000000000000000000000000000000000000004",
            },
          },
          "metadata": {
            "environment": "Such Meta",
            "auction_id": null,
            "run_id": null,
            "gas_price": null,
            "native_token": null,
          },
        });
        assert_eq!(result, expected);
    }

    #[test]
    fn deserialize_approval_model() {
        let approval = r#"
            {
                "token": "0x7777777777777777777777777777777777777777",
                "spender": "0x5555555555555555555555555555555555555555",
                "amount": "1337"
            }
        "#;
        assert_eq!(
            serde_json::from_str::<ApprovalModel>(approval).unwrap(),
            ApprovalModel {
                token: addr!("7777777777777777777777777777777777777777"),
                spender: addr!("5555555555555555555555555555555555555555"),
                amount: 1337.into(),
            }
        );
    }

    #[test]
    fn decode_empty_solution() {
        let empty_solution = r#"
            {
                "tokens": {
                    "0xa7d1c04faf998f9161fc9f800a99a809b84cfc9d": {
                        "decimals": 18,
                        "alias": null,
                        "normalize_priority": 0
                    },
                    "0xc778417e063141139fce010982780140aa0cd5ab": {
                        "decimals": 18,
                        "alias": null,
                        "normalize_priority": 1
                    }
                },
                "orders": {},
                "metadata": {},
                "ref_token": "0xc778417e063141139fce010982780140aa0cd5ab",
                "prices": {
                    "0xa7d1c04faf998f9161fc9f800a99a809b84cfc9d": "1039670252129038",
                    "0xc778417e063141139fce010982780140aa0cd5ab": "1000000000000000000"
                }
            }
        "#;
        assert!(serde_json::from_str::<SettledBatchAuctionModel>(empty_solution).is_ok());
    }

    #[test]
    fn decode_trivial_solution_without_ref_token() {
        let x = r#"
            {
                "tokens": {},
                "orders": {},
                "metadata": {
                    "environment": null
                },
                "ref_token": null,
                "prices": {},
                "uniswaps": {}
            }
        "#;
        assert!(serde_json::from_str::<SettledBatchAuctionModel>(x).is_ok());
    }

    #[test]
    fn decode_execution_plan() {
        for (json, expected) in [
            (r#""internal""#, ExecutionPlan::Internal),
            (
                r#"{ "sequence": 42, "position": 1337 }"#,
                ExecutionPlan::Coordinates(ExecutionPlanCoordinatesModel {
                    sequence: 42,
                    position: 1337,
                }),
            ),
        ] {
            assert_eq!(
                serde_json::from_str::<ExecutionPlan>(json).unwrap(),
                expected,
            );
        }
    }

    #[test]
    fn decode_interaction_data() {
        assert_eq!(
            serde_json::from_str::<InteractionData>(
                r#"
                    {
                        "target": "0xffffffffffffffffffffffffffffffffffffffff",
                        "value": "0",
                        "call_data": "0x01020304",
                        "inputs": [
                            {
                                "token": "0x0101010101010101010101010101010101010101",
                                "amount": "9999"
                            }
                        ],
                        "outputs": [
                            {
                                "token": "0x0202020202020202020202020202020202020202",
                                "amount": "2000"
                            },
                            {
                                "token": "0x0303030303030303030303030303030303030303",
                                "amount": "3000"
                            }
                        ],
                        "exec_plan": "internal",
                        "cost": {
                            "amount": "1",
                            "token": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"
                        }
                    }
                "#,
            )
            .unwrap(),
            InteractionData {
                target: H160([0xff; 20]),
                value: 0.into(),
                call_data: vec![1, 2, 3, 4],
                inputs: vec![TokenAmount {
                    token: H160([1; 20]),
                    amount: 9999.into(),
                }],
                outputs: vec![
                    TokenAmount {
                        token: H160([2; 20]),
                        amount: 2000.into(),
                    },
                    TokenAmount {
                        token: H160([3; 20]),
                        amount: 3000.into(),
                    }
                ],
                exec_plan: Some(ExecutionPlan::Internal),
                cost: Some(TokenAmount {
                    amount: 1.into(),
                    token: addr!("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee")
                })
            },
        );
    }

    #[test]
    fn decode_foreign_liquidity_order() {
        assert_eq!(
            serde_json::from_str::<ExecutedLiquidityOrderModel>(
                r#"
                {
                    "order": {
                        "from": "0x4242424242424242424242424242424242424242",
                        "sellToken": "0x0101010101010101010101010101010101010101",
                        "buyToken": "0x0202020202020202020202020202020202020202",
                        "sellAmount": "101",
                        "buyAmount": "102",
                        "validTo": 3,
                        "appData":
                            "0x0303030303030303030303030303030303030303030303030303030303030303",
                        "feeAmount": "13",
                        "kind": "sell",
                        "partiallyFillable": true,
                        "sellTokenBalance": "external",
                        "signingScheme": "eip1271",
                        "signature": "0x01020304"
                    },
                    "exec_sell_amount": "50",
                    "exec_buy_amount": "51"
                }
                "#,
            )
            .unwrap(),
            ExecutedLiquidityOrderModel {
                order: NativeLiquidityOrder {
                    from: H160([0x42; 20]),
                    data: OrderData {
                        sell_token: H160([1; 20]),
                        buy_token: H160([2; 20]),
                        sell_amount: 101.into(),
                        buy_amount: 102.into(),
                        valid_to: 3,
                        app_data: AppId([3; 32]),
                        fee_amount: 13.into(),
                        kind: OrderKind::Sell,
                        partially_fillable: true,
                        sell_token_balance: SellTokenSource::External,
                        ..Default::default()
                    },
                    signature: Signature::Eip1271(vec![1, 2, 3, 4]),
                },
                exec_sell_amount: 50.into(),
                exec_buy_amount: 51.into(),
            },
        );
    }
}
