use ethcontract::H160;
use model::{
    ratio_as_decimal,
    u256_decimal::{self, DecimalU256},
};
use num::BigRational;
use primitive_types::U256;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::collections::{BTreeMap, HashMap};

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
    pub fee: FeeModel,
    pub cost: CostModel,
    pub is_liquidity_order: bool,
    #[serde(default)]
    pub mandatory: bool,
    /// Signals if the order will be executed as an atomic unit. In that case the order's
    /// preconditions have to be met for it to be executed successfully. This is different from the
    /// usual user provided orders because those can be batched together and it's only relevant if
    /// the pre- and post conditions are met after the complete batch got executed.
    pub has_atomic_execution: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AmmModel {
    #[serde(flatten)]
    pub parameters: AmmParameters,
    #[serde(with = "ratio_as_decimal")]
    pub fee: BigRational,
    pub cost: CostModel,
    pub mandatory: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum AmmParameters {
    ConstantProduct(ConstantProductPoolParameters),
    WeightedProduct(WeightedProductPoolParameters),
    Stable(StablePoolParameters),
}

#[serde_as]
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WeightedProductPoolParameters {
    pub reserves: BTreeMap<H160, WeightedPoolTokenData>,
}

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
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
pub struct TokenInfoModel {
    pub decimals: Option<u8>,
    pub alias: Option<String>,
    pub external_price: Option<f64>,
    pub normalize_priority: Option<u64>,
    #[serde_as(as = "Option<DecimalU256>")]
    pub internal_buffer: Option<U256>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CostModel {
    #[serde(with = "u256_decimal")]
    pub amount: U256,
    pub token: H160,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FeeModel {
    #[serde(with = "u256_decimal")]
    pub amount: U256,
    pub token: H160,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ApprovalModel {
    pub token: H160,
    pub spender: H160,
    #[serde(with = "u256_decimal")]
    pub amount: U256,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct InteractionData {
    pub target: H160,
    pub value: U256,
    pub call_data: Vec<u8>,
    pub exec_plan: Option<ExecutionPlanCoordinatesModel>,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
pub struct SettledBatchAuctionModel {
    pub orders: HashMap<usize, ExecutedOrderModel>,
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
        self.amms
            .values()
            .flat_map(|u| &u.execution)
            .all(|u| u.exec_plan.is_some())
    }
}

#[derive(Clone, Debug, Serialize, Default)]
pub struct MetadataModel {
    pub environment: Option<String>,
    pub auction_id: Option<u64>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SettledBatchAuctionMetadataModel {
    pub has_solution: Option<bool>,
    pub result: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ExecutedOrderModel {
    #[serde(with = "u256_decimal")]
    pub exec_sell_amount: U256,
    #[serde(with = "u256_decimal")]
    pub exec_buy_amount: U256,
    pub cost: Option<CostModel>,
    pub fee: Option<FeeModel>,
    // Orders which need to be executed in a specific order have an `exec_plan` (e.g. 0x limit orders)
    pub exec_plan: Option<ExecutionPlanCoordinatesModel>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct UpdatedAmmModel {
    /// We ignore additional incoming amm fields we don't need.
    pub execution: Vec<ExecutedAmmModel>,
    pub cost: Option<CostModel>,
}

#[derive(Clone, Debug, Default, Deserialize)]
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
    pub exec_plan: Option<ExecutionPlanCoordinatesModel>,
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

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExecutionPlanCoordinatesModel {
    pub sequence: u32,
    pub position: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use maplit::btreemap;
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
            exec_plan: Some(ExecutionPlanCoordinatesModel {
                sequence: 0,
                position: 0,
            }),
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
            fee: FeeModel {
                amount: U256::from(2),
                token: sell_token,
            },
            cost: CostModel {
                amount: U256::from(1),
                token: native_token,
            },
            is_liquidity_order: false,
            mandatory: false,
            has_atomic_execution: false,
        };
        let constant_product_pool_model = AmmModel {
            parameters: AmmParameters::ConstantProduct(ConstantProductPoolParameters {
                reserves: btreemap! {
                    buy_token => U256::from(100),
                    sell_token => U256::from(200),
                },
            }),
            fee: BigRational::new(3.into(), 1000.into()),
            cost: CostModel {
                amount: U256::from(3),
                token: native_token,
            },
            mandatory: false,
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
            cost: CostModel {
                amount: U256::from(2),
                token: native_token,
            },
            mandatory: true,
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
            cost: CostModel {
                amount: U256::from(3),
                token: native_token,
            },
            mandatory: true,
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
            },
            metadata: Some(MetadataModel {
                environment: Some(String::from("Such Meta")),
                auction_id: None,
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
              "has_atomic_execution": false
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
            },
          },
          "metadata": {
            "environment": "Such Meta",
            "auction_id": null,
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
}
