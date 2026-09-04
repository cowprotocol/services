use {
    alloy_primitives::{Address, U256},
    number::serialization::HexOrDecimalU256,
    serde::{Deserialize, Deserializer, Serialize, de},
    serde_with::serde_as,
    std::collections::{HashMap, HashSet},
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SolverError {
    pub code: SolverErrorCode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SolverErrorCode {
    /// Token can only be traded during specific time windows (e.g., RWA tokens)
    TradingOutsideAllowedWindow,
    /// Token is temporarily suspended from trading
    TokenTemporarilySuspended,
    /// Insufficient liquidity for the requested trade size
    InsufficientLiquidity,
    /// Generic solver error with custom message
    Other,
}

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum SolverResponse {
    Solutions { solutions: Vec<Solution> },
    Error { error: SolverError },
}

/// Hand written because `#[serde(untagged)]` discards the error of every
/// variant it tries and reports only that none matched, which hides why a
/// solution was rejected from the solver we notify about it.
impl<'de> Deserialize<'de> for SolverResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Raw {
            solutions: Option<Vec<Solution>>,
            error: Option<SolverError>,
        }

        match Raw::deserialize(deserializer)? {
            Raw {
                solutions: Some(solutions),
                ..
            } => Ok(Self::Solutions { solutions }),
            Raw {
                error: Some(error), ..
            } => Ok(Self::Error { error }),
            Raw { .. } => Err(de::Error::custom(
                "expected either a `solutions` or an `error` field",
            )),
        }
    }
}

impl Default for SolverResponse {
    fn default() -> Self {
        Self::Solutions {
            solutions: Vec::new(),
        }
    }
}

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Solution {
    pub id: u64,
    #[serde_as(as = "HashMap<_, HexOrDecimalU256>")]
    pub prices: HashMap<Address, U256>,
    #[serde(deserialize_with = "deserialize_trades")]
    pub trades: Vec<Trade>,
    #[serde(default)]
    pub pre_interactions: Vec<Call>,
    pub interactions: Vec<Interaction>,
    #[serde(default)]
    pub post_interactions: Vec<Call>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas: Option<u64>,
    #[serde(flatten)]
    pub gas_fee_override: Option<GasFeeOverride>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub flashloans: Option<HashMap<OrderUid, Flashloan>>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub wrappers: Vec<WrapperCall>,
}

/// A partially fillable order may be split across several solutions, but a
/// single solution settles each order exactly once.
fn deserialize_trades<'de, D>(deserializer: D) -> Result<Vec<Trade>, D::Error>
where
    D: Deserializer<'de>,
{
    let trades = Vec::<Trade>::deserialize(deserializer)?;

    let mut settled = HashSet::with_capacity(trades.len());
    for trade in &trades {
        let Trade::Fulfillment(fulfillment) = trade else {
            continue;
        };
        if !settled.insert(&fulfillment.order) {
            let uid = const_hex::encode_prefixed(fulfillment.order.0);
            return Err(de::Error::custom(format!(
                "order {uid} is settled by more than one trade"
            )));
        }
    }

    Ok(trades)
}

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct OrderUid(#[serde_as(as = "serde_ext::Hex")] pub [u8; 56]);

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Trade {
    Fulfillment(Fulfillment),
    Jit(JitTrade),
}

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fulfillment {
    pub order: OrderUid,
    #[serde_as(as = "HexOrDecimalU256")]
    pub executed_amount: U256,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde_as(as = "Option<HexOrDecimalU256>")]
    pub fee: Option<U256>,
}

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JitTrade {
    pub order: JitOrder,
    #[serde_as(as = "HexOrDecimalU256")]
    pub executed_amount: U256,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde_as(as = "Option<HexOrDecimalU256>")]
    pub fee: Option<U256>,
}

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JitOrder {
    pub sell_token: Address,
    pub buy_token: Address,
    pub receiver: Address,
    #[serde_as(as = "HexOrDecimalU256")]
    pub sell_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub buy_amount: U256,
    #[serde(default)]
    pub partially_fillable: bool,
    pub valid_to: u32,
    #[serde_as(as = "serde_ext::Hex")]
    pub app_data: [u8; 32],
    pub kind: Kind,
    pub sell_token_balance: SellTokenBalance,
    pub buy_token_balance: BuyTokenBalance,
    pub signing_scheme: SigningScheme,
    #[serde_as(as = "serde_ext::Hex")]
    pub signature: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Sell,
    Buy,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Interaction {
    Liquidity(LiquidityInteraction),
    Custom(CustomInteraction),
}

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub struct Call {
    pub target: Address,
    pub value: U256,
    #[serde(rename = "callData")]
    #[serde_as(as = "serde_ext::Hex")]
    pub calldata: Vec<u8>,
}

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiquidityInteraction {
    pub internalize: bool,
    pub id: String,
    pub input_token: Address,
    pub output_token: Address,
    #[serde_as(as = "HexOrDecimalU256")]
    pub input_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub output_amount: U256,
}

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomInteraction {
    pub internalize: bool,
    pub target: Address,
    #[serde_as(as = "HexOrDecimalU256")]
    pub value: U256,
    #[serde(rename = "callData")]
    #[serde_as(as = "serde_ext::Hex")]
    pub calldata: Vec<u8>,
    pub allowances: Vec<Allowance>,
    pub inputs: Vec<Asset>,
    pub outputs: Vec<Asset>,
}

/// An interaction that can be executed as part of an order's pre- or
/// post-interactions.
#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderInteraction {
    pub target: Address,
    #[serde_as(as = "HexOrDecimalU256")]
    pub value: U256,
    #[serde(rename = "callData")]
    #[serde_as(as = "serde_ext::Hex")]
    pub calldata: Vec<u8>,
}

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub token: Address,
    #[serde_as(as = "HexOrDecimalU256")]
    pub amount: U256,
}

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Allowance {
    pub token: Address,
    pub spender: Address,
    #[serde_as(as = "HexOrDecimalU256")]
    pub amount: U256,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SellTokenBalance {
    #[default]
    Erc20,
    #[deprecated(
        note = "Balancer Vault token sources are deprecated and no longer appear in auctions; \
                only erc20 is used"
    )]
    Internal,
    #[deprecated(
        note = "Balancer Vault token sources are deprecated and no longer appear in auctions; \
                only erc20 is used"
    )]
    External,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BuyTokenBalance {
    #[default]
    Erc20,
    #[deprecated(
        note = "Balancer Vault token sources are deprecated and no longer appear in auctions; \
                only erc20 is used"
    )]
    Internal,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SigningScheme {
    Eip712,
    EthSign,
    PreSign,
    Eip1271,
}

/// Solver-provided gas fee overrides for the settlement transaction.
#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GasFeeOverride {
    #[serde_as(as = "HexOrDecimalU256")]
    pub max_fee_per_gas: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub max_priority_fee_per_gas: U256,
}

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Flashloan {
    pub liquidity_provider: Address,
    pub protocol_adapter: Address,
    pub receiver: Address,
    pub token: Address,
    #[serde_as(as = "HexOrDecimalU256")]
    pub amount: U256,
}

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WrapperCall {
    pub address: Address,
    #[serde_as(as = "serde_ext::Hex")]
    #[serde(default)]
    pub data: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use {super::*, serde_json::json};

    #[test]
    fn serializes_empty_solutions_response() {
        let response = SolverResponse::default();

        let value = serde_json::to_value(response).unwrap();
        assert_eq!(
            value,
            json!({
                "solutions": [],
            })
        );
    }

    fn order_uid(byte: u8) -> String {
        const_hex::encode_prefixed([byte; 56])
    }

    fn solution_with_fulfillments(orders: &[String]) -> serde_json::Value {
        json!({
            "id": 0,
            "prices": {},
            "trades": orders.iter().map(|order| json!({
                "kind": "fulfillment",
                "order": order,
                "executedAmount": "1000",
            })).collect::<Vec<_>>(),
            "interactions": [],
        })
    }

    #[test]
    fn rejects_solutions_settling_the_same_order_twice() {
        let order = order_uid(1);
        let solution = solution_with_fulfillments(&[order.clone(), order.clone()]);

        let err = serde_json::from_value::<Solution>(solution).unwrap_err();
        assert_eq!(
            err.to_string(),
            format!("order {order} is settled by more than one trade")
        );
    }

    #[test]
    fn accepts_one_trade_per_order() {
        let solution = solution_with_fulfillments(&[order_uid(1), order_uid(2)]);

        let solution = serde_json::from_value::<Solution>(solution).unwrap();
        assert_eq!(solution.trades.len(), 2);
    }

    #[test]
    fn duplicate_trade_error_survives_the_response_wrapper() {
        let order = order_uid(1);
        let response = json!({
            "solutions": [solution_with_fulfillments(&[order.clone(), order.clone()])],
        });

        let err = serde_json::from_value::<SolverResponse>(response).unwrap_err();
        assert!(
            err.to_string()
                .contains(&format!("order {order} is settled by more than one trade")),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn serializes_and_deserializes_error_responses() {
        let cases = vec![
            (
                SolverErrorCode::TradingOutsideAllowedWindow,
                "tradingOutsideAllowedWindow",
            ),
            (
                SolverErrorCode::TokenTemporarilySuspended,
                "tokenTemporarilySuspended",
            ),
            (
                SolverErrorCode::InsufficientLiquidity,
                "insufficientLiquidity",
            ),
            (SolverErrorCode::Other, "other"),
        ];

        for (code, expected_code) in cases {
            let response = SolverResponse::Error {
                error: SolverError {
                    code: code.clone(),
                    message: Some("custom message".to_string()),
                },
            };

            let value = serde_json::to_value(response).unwrap();
            assert_eq!(
                value,
                json!({
                    "error": {
                        "code": expected_code,
                        "message": "custom message",
                    },
                })
            );

            let decoded: SolverResponse = serde_json::from_value(value).unwrap();
            assert!(matches!(
                decoded,
                SolverResponse::Error { error }
                if error.code == code && error.message.as_deref() == Some("custom message")
            ));
        }
    }
}
