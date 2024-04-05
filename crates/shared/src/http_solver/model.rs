use {
    crate::{
        interaction::{EncodedInteraction, Interaction},
        sources::uniswap_v3::pool_fetching::PoolInfo,
    },
    derivative::Derivative,
    ethcontract::{Bytes, H160},
    model::{
        auction::AuctionId,
        order::{OrderData, OrderUid},
        ratio_as_decimal,
        signature::Signature,
    },
    num::BigRational,
    number::serialization::HexOrDecimalU256,
    primitive_types::{H256, U256},
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
    std::collections::{BTreeMap, BTreeSet, HashMap},
    web3::types::AccessList,
};

#[derive(Clone, Debug, Default, Serialize)]
pub struct BatchAuctionModel {
    pub tokens: BTreeMap<H160, TokenInfoModel>,
    pub orders: BTreeMap<usize, OrderModel>,
    pub amms: BTreeMap<H160, AmmModel>,
    pub metadata: Option<MetadataModel>,
}

#[serde_as]
#[derive(Clone, Debug, Serialize)]
pub struct OrderModel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<OrderUid>,
    pub sell_token: H160,
    pub buy_token: H160,
    #[serde_as(as = "HexOrDecimalU256")]
    pub sell_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub buy_amount: U256,
    pub allow_partial_fill: bool,
    pub is_sell_order: bool,
    /// Represents user_fee. Which is 0 for limit orders.
    pub fee: TokenAmount,
    pub cost: TokenAmount,
    pub is_liquidity_order: bool,
    /// [DEPRECATED] All orders are always mature.
    pub is_mature: bool,
    /// [DEPRECATED] Mandatory flag is not useful enough to warrant keeping
    /// around.
    #[serde(default)]
    pub mandatory: bool,
    /// Signals if the order will be executed as an atomic unit. In that case
    /// the order's preconditions have to be met for it to be executed
    /// successfully. This is different from the usual user provided orders
    /// because those can be batched together and it's only relevant if
    /// the pre- and post conditions are met after the complete batch got
    /// executed.
    pub has_atomic_execution: bool,
    /// [DEPRECATED] CIP-14 risk adjusted solver reward is no longer used
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
    #[serde_as(as = "BTreeMap<_, HexOrDecimalU256>")]
    pub reserves: BTreeMap<H160, U256>,
}

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WeightedPoolTokenData {
    #[serde_as(as = "HexOrDecimalU256")]
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
    #[serde_as(as = "BTreeMap<_, HexOrDecimalU256>")]
    pub reserves: BTreeMap<H160, U256>,
    #[serde_as(as = "BTreeMap<_, HexOrDecimalU256>")]
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
    #[serde_as(as = "Option<HexOrDecimalU256>")]
    pub internal_buffer: Option<U256>,
    /// Is token in the external list containing only safe tokens
    pub accepted_for_internalization: bool,
}

#[serde_as]
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct TokenAmount {
    #[serde_as(as = "HexOrDecimalU256")]
    pub amount: U256,
    pub token: H160,
}

impl TokenAmount {
    pub fn new<T: Into<U256>>(token: H160, amount: T) -> Self {
        Self {
            amount: amount.into(),
            token,
        }
    }
}

#[serde_as]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ApprovalModel {
    pub token: H160,
    pub spender: H160,
    #[serde_as(as = "HexOrDecimalU256")]
    pub amount: U256,
}

#[serde_as]
#[derive(Clone, Derivative, Default, Deserialize, Eq, PartialEq, Serialize)]
#[derivative(Debug)]
pub struct InteractionData {
    pub target: H160,
    #[serde_as(as = "HexOrDecimalU256")]
    pub value: U256,
    #[derivative(Debug(format_with = "crate::debug_bytes"))]
    #[serde(with = "bytes_hex")]
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
    // TODO remove Option once all external solvers conform to sending exec plans instead of null.
    pub exec_plan: Option<ExecutionPlan>,
    pub cost: Option<TokenAmount>,
}

impl Interaction for InteractionData {
    fn encode(&self) -> EncodedInteraction {
        (self.target, self.value, Bytes(self.call_data.clone()))
    }
}

#[serde_as]
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct SettledBatchAuctionModel {
    pub orders: HashMap<usize, ExecutedOrderModel>,
    #[serde(default)]
    pub foreign_liquidity_orders: Vec<ExecutedLiquidityOrderModel>,
    #[serde(default)]
    pub amms: HashMap<H160, UpdatedAmmModel>,
    pub ref_token: Option<H160>,
    #[serde_as(as = "HashMap<_, HexOrDecimalU256>")]
    pub prices: HashMap<H160, U256>,
    #[serde(default)]
    pub approvals: Vec<ApprovalModel>,
    #[serde(default)]
    pub interaction_data: Vec<InteractionData>,
    pub metadata: Option<SettledBatchAuctionMetadataModel>,
}

impl SettledBatchAuctionModel {
    // TODO remove this function once all solvers conform to sending the
    // execution plan for their custom interactions!
    pub fn add_missing_execution_plans(&mut self) {
        for (index, interaction) in self.interaction_data.iter_mut().enumerate() {
            if interaction.exec_plan.is_none() {
                // if no exec plan is provided, convert to the default exec plan by setting the
                // position coordinate to the position of the exec plan in the interactions
                // vector
                interaction.exec_plan = Some(ExecutionPlan {
                    coordinates: ExecutionPlanCoordinatesModel {
                        sequence: u32::MAX,
                        position: index as u32,
                    },
                    internal: false,
                })
            }
        }
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

#[serde_as]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ExecutedOrderModel {
    #[serde_as(as = "HexOrDecimalU256")]
    pub exec_sell_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub exec_buy_amount: U256,
    #[serde_as(as = "Option<HexOrDecimalU256>")]
    pub exec_fee_amount: Option<U256>,
    pub cost: Option<TokenAmount>,
    pub fee: Option<TokenAmount>,
    // Orders which need to be executed in a specific order have an `exec_plan` (e.g. 0x limit
    // orders)
    pub exec_plan: Option<ExecutionPlan>,
}

#[serde_as]
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct ExecutedLiquidityOrderModel {
    pub order: NativeLiquidityOrder,
    #[serde_as(as = "HexOrDecimalU256")]
    pub exec_sell_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
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

#[serde_as]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UpdatedAmmModel {
    /// We ignore additional incoming amm fields we don't need.
    pub execution: Vec<ExecutedAmmModel>,
    pub cost: Option<TokenAmount>,
}

#[serde_as]
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ExecutedAmmModel {
    pub sell_token: H160,
    pub buy_token: H160,
    #[serde_as(as = "HexOrDecimalU256")]
    pub exec_sell_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub exec_buy_amount: U256,
    pub exec_plan: ExecutionPlan,
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

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct ExecutionPlan {
    #[serde(flatten)]
    pub coordinates: ExecutionPlanCoordinatesModel,
    pub internal: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, PartialOrd, Ord, Serialize, Hash)]
pub struct ExecutionPlanCoordinatesModel {
    pub sequence: u32,
    pub position: u32,
}

/// The result of a submission process for a winning solver
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SubmissionResult {
    Success(H256),
    Revert(H256),
    SimulationRevert,
    Fail,
}

/// The result a given solver achieved in the auction
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AuctionResult {
    /// Solution was valid and was ranked at the given place
    /// Rank 1 means the solver won the competition
    Ranked(usize),

    /// Solution was invalid for some reason
    Rejected(SolverRejectionReason),

    /// For winners, additional notify is sent after submission onchain is
    /// finalized
    SubmittedOnchain(SubmissionResult),
}

type SimulationSucceededAtLeastOnce = bool;

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SolverRejectionReason {
    /// The solver didn't return a successful response
    RunError(SolverRunError),

    /// The solver returned an empty solution
    EmptySolution,

    /// The solution candidate didn't include any user orders
    NoUserOrders,

    /// The solution violated a price constraint (ie. max deviation to external
    /// price vector)
    PriceViolation,

    /// The solution contains custom interation/s using the token/s not
    /// contained in the allowed bufferable list Returns the list of not
    /// allowed tokens
    NonBufferableTokensUsed(BTreeSet<H160>),

    /// The solution contains non unique execution plans (duplicated
    /// coordinates)
    InvalidExecutionPlans,

    /// The solution didn't pass simulation. Includes all data needed to
    /// re-create simulation locally
    SimulationFailure(TransactionWithError, SimulationSucceededAtLeastOnce),

    /// Not all trades have clearing prices
    InvalidClearingPrices,

    /// Solver balance too low to cover the execution costs.
    SolverAccountInsufficientBalance(U256),

    /// Solution received from solver engine don't have unique id.
    DuplicatedSolutionId(u64),

    /// Some aspect of the driver logic failed.
    Driver(String),

    /// On-chain solution postprocessing timed out.
    PostprocessingTimedOut,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SolverRunError {
    Timeout,
    Solving(String),
}

/// Contains all information about a failing settlement simulation
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionWithError {
    /// Transaction data used for simulation of the settlement
    #[serde(flatten)]
    pub transaction: SimulatedTransaction,
    /// Error message from the simulator
    pub error: String,
}

/// Transaction data used for simulation of the settlement
#[serde_as]
#[derive(Clone, Serialize, Derivative)]
#[derivative(Debug)]
#[serde(rename_all = "camelCase")]
pub struct SimulatedTransaction {
    /// The simulation was done at the beginning of the block
    pub block_number: u64,
    /// Index of the transaction inside the block the transaction was simulated
    /// on
    pub tx_index: u64,
    /// Is transaction simulated with internalized interactions or without
    /// TODO: remove field once the colocation is enabled.
    pub internalization: InternalizationStrategy,
    /// Which storage the settlement tries to access. Contains `None` if some
    /// error happened while estimating the access list.
    pub access_list: Option<AccessList>,
    /// Solver address
    pub from: H160,
    /// GPv2 settlement contract address
    pub to: H160,
    /// Transaction input data
    #[derivative(Debug(format_with = "crate::debug_bytes"))]
    #[serde(with = "bytes_hex")]
    pub data: Vec<u8>,
    /// Gas price can influence the success of simulation if sender balance
    /// is not enough for paying the costs of executing the transaction onchain
    #[serde_as(as = "HexOrDecimalU256")]
    pub max_fee_per_gas: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub max_priority_fee_per_gas: U256,
}

/// Whether or not internalizable interactions should be encoded as calldata
#[derive(Debug, Copy, Clone, Serialize)]
pub enum InternalizationStrategy {
    #[serde(rename = "Disabled")]
    EncodeAllInteractions,
    #[serde(rename = "Enabled")]
    SkipInternalizableInteraction,
    Unknown,
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::sources::uniswap_v3::graph_api::Token,
        app_data::AppDataHash,
        ethcontract::H256,
        maplit::btreemap,
        model::order::{OrderKind, SellTokenSource},
        serde_json::json,
        std::str::FromStr,
        web3::types::AccessListItem,
    };

    #[test]
    fn updated_amm_model_is_non_trivial() {
        assert!(!UpdatedAmmModel {
            execution: vec![],
            cost: Default::default(),
        }
        .is_non_trivial());

        let trivial_execution = ExecutedAmmModel::default();

        assert!(!UpdatedAmmModel {
            execution: vec![trivial_execution.clone()],
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
            execution: vec![trivial_execution, execution_with_buy],
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
            id: Some(OrderUid::default()),
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
            is_mature: false,
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
                    accepted_for_internalization: true,
                },
                sell_token => TokenInfoModel {
                    decimals: Some(18),
                    alias: Some("DOG".to_string()),
                    external_price: Some(2345.0),
                    normalize_priority: Some(0),
                    internal_buffer: Some(U256::from(42)),
                    accepted_for_internalization: true,
                }
            },
            orders: btreemap! { 0 => order_model },
            amms: btreemap! {
                H160::from_low_u64_be(0) => constant_product_pool_model,
                H160::from_low_u64_be(1) => weighted_product_pool_model,
                H160::from_low_u64_be(2) => stable_pool_model,
                H160::from_low_u64_be(3) => concentrated_pool_model,
            },
            metadata: Some(MetadataModel {
                environment: Some(String::from("Such Meta")),
                ..Default::default()
            }),
        };

        let result = serde_json::to_value(model).unwrap();

        let expected = json!({
          "tokens": {
            "0x0000000000000000000000000000000000000539": {
              "decimals": 6,
              "alias": "CAT",
              "external_price": 1.2,
              "normalize_priority": 1,
              "internal_buffer": "1337",
              "accepted_for_internalization": true,
            },
            "0x000000000000000000000000000000000000a866": {
              "decimals": 18,
              "alias": "DOG",
              "external_price": 2345.0,
              "normalize_priority": 0,
              "internal_buffer": "42",
              "accepted_for_internalization": true,
            },
          },
          "orders": {
            "0": {
              "id": "0x0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
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
              "reward": 3.0,
              "is_mature": false,
            },
          },
          "amms": {
            "0x0000000000000000000000000000000000000000": {
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
            "0x0000000000000000000000000000000000000001": {
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
            "0x0000000000000000000000000000000000000002": {
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
            "0x0000000000000000000000000000000000000003": {
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
        assert_eq!(
            serde_json::from_str::<ExecutionPlan>(
                r#"
                    {
                        "sequence": 42,
                        "position": 1337,
                        "internal": true
                    }
                "#,
            )
            .unwrap(),
            ExecutionPlan {
                coordinates: ExecutionPlanCoordinatesModel {
                    sequence: 42,
                    position: 1337,
                },
                internal: true
            }
        );
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
                        "exec_plan": {
                            "sequence": 0,
                            "position": 0,
                            "internal": true
                        },
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
                exec_plan: Some(ExecutionPlan {
                    coordinates: ExecutionPlanCoordinatesModel {
                        sequence: 0,
                        position: 0,
                    },
                    internal: true,
                }),
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
                        "class": "market",
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
                        app_data: AppDataHash([3; 32]),
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

    #[test]
    fn serialize_simulated_transaction() {
        assert_eq!(
            serde_json::to_value(SimulatedTransaction {
                access_list: Some(vec![AccessListItem {
                    address: H160::from_low_u64_be(1),
                    storage_keys: vec![H256::from_low_u64_be(2)]
                }]),
                block_number: 15848799,
                tx_index: 0,
                from: H160::from_str("0x9008D19f58AAbD9eD0D60971565AA8510560ab41").unwrap(),
                to: H160::from_str("0x9008D19f58AAbD9eD0D60971565AA8510560ab41").unwrap(),
                data: vec![19, 250, 73],
                internalization: InternalizationStrategy::SkipInternalizableInteraction,
                max_fee_per_gas: U256::from(100),
                max_priority_fee_per_gas: U256::from(10),
            })
            .unwrap(),
            json!({
                "accessList": [{
                    "address": "0x0000000000000000000000000000000000000001",
                    "storageKeys": [
                        "0x0000000000000000000000000000000000000000000000000000000000000002"
                    ]
                }],
                "blockNumber": 15848799,
                "txIndex": 0,
                "data": "0x13fa49",
                "from": "0x9008d19f58aabd9ed0d60971565aa8510560ab41",
                "to": "0x9008d19f58aabd9ed0d60971565aa8510560ab41",
                "internalization": "Enabled",
                "maxFeePerGas": "100",
                "maxPriorityFeePerGas": "10",
            }),
        );
    }

    #[test]
    fn serialize_rejection_non_bufferable_tokens_used() {
        assert_eq!(
            serde_json::to_value(SolverRejectionReason::NonBufferableTokensUsed(
                [H160::from_low_u64_be(1), H160::from_low_u64_be(2)]
                    .into_iter()
                    .collect()
            ))
            .unwrap(),
            json!({
                "nonBufferableTokensUsed": ["0x0000000000000000000000000000000000000001", "0x0000000000000000000000000000000000000002"],
            }),
        );
    }
}
