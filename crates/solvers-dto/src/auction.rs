use {
    alloy_primitives::{Address, B256, U256},
    app_data::AppDataHash,
    bigdecimal::BigDecimal,
    eth_domain_types::Receiver,
    number::serialization::HexOrDecimalU256,
    serde::{Deserialize, Serialize},
    serde_with::{DisplayFromStr, serde_as},
    std::{
        collections::{BTreeMap, HashMap},
        sync::Arc,
    },
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "RawAuction", into = "RawAuction")]
pub struct Auction {
    pub id: Id,
    pub tokens: HashMap<Address, Token>,
    pub orders: Vec<Order>,
    pub liquidity: Vec<Liquidity>,
    pub effective_gas_price: U256,
    pub deadline: chrono::DateTime<chrono::Utc>,
    pub surplus_capturing_jit_order_owners: Vec<Address>,
}

/// Whether a `/solve` request is a competition auction or a quote. The variants
/// are mutually exclusive, so a request can never be both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Id {
    /// A competition auction.
    Auction(i64),
    /// A quote. `fast_path` marks a fast-path quote; `price_quality` is the
    /// quality the orderbook was asked for.
    Quote {
        id: i64,
        fast_path: bool,
        price_quality: PriceQuality,
    },
}

/// How the orderbook picks the winning quote among the solvers' answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PriceQuality {
    /// The best of the first few quotes to arrive wins; nothing is simulated.
    Fast,
    /// The best promised price wins; nothing is simulated.
    Optimal,
    /// Quotes are simulated where possible and a quote that passes simulation
    /// is preferred over one that doesn't, even at a worse price.
    Verified,
}

impl Id {
    /// The auction or quote id.
    pub fn id(&self) -> i64 {
        match *self {
            Id::Auction(id) | Id::Quote { id, .. } => id,
        }
    }
}

/// The wire form of [`Auction`]. `id` is `null` for quotes, which is how
/// solvers tell a quote from an auction.
#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawAuction {
    #[serde_as(as = "Option<DisplayFromStr>")]
    id: Option<i64>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    quote_id: Option<i64>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    fast_path: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    price_quality: Option<PriceQuality>,
    tokens: HashMap<Address, Token>,
    orders: Vec<Order>,
    liquidity: Vec<Liquidity>,
    #[serde_as(as = "HexOrDecimalU256")]
    effective_gas_price: U256,
    deadline: chrono::DateTime<chrono::Utc>,
    surplus_capturing_jit_order_owners: Vec<Address>,
}

impl TryFrom<RawAuction> for Auction {
    type Error = &'static str;

    fn try_from(raw: RawAuction) -> Result<Self, Self::Error> {
        let id = match (raw.id, raw.quote_id) {
            (Some(_), Some(_)) => return Err("`id` and `quoteId` are mutually exclusive"),
            (None, None) => return Err("either `id` or `quoteId` must be set"),
            (Some(_), None) if raw.fast_path => return Err("`fastPath` is only valid for quotes"),
            (Some(_), None) if raw.price_quality.is_some() => {
                return Err("`priceQuality` is only valid for quotes");
            }
            (Some(id), None) => Id::Auction(id),
            (None, Some(id)) => Id::Quote {
                id,
                fast_path: raw.fast_path,
                price_quality: raw.price_quality.ok_or("quotes carry a `priceQuality`")?,
            },
        };
        Ok(Auction {
            id,
            tokens: raw.tokens,
            orders: raw.orders,
            liquidity: raw.liquidity,
            effective_gas_price: raw.effective_gas_price,
            deadline: raw.deadline,
            surplus_capturing_jit_order_owners: raw.surplus_capturing_jit_order_owners,
        })
    }
}

impl From<Auction> for RawAuction {
    fn from(auction: Auction) -> Self {
        let (id, quote_id, fast_path, price_quality) = match auction.id {
            Id::Auction(id) => (Some(id), None, false, None),
            Id::Quote {
                id,
                fast_path,
                price_quality,
            } => (None, Some(id), fast_path, Some(price_quality)),
        };
        RawAuction {
            id,
            quote_id,
            fast_path,
            price_quality,
            tokens: auction.tokens,
            orders: auction.orders,
            liquidity: auction.liquidity,
            effective_gas_price: auction.effective_gas_price,
            deadline: auction.deadline,
            surplus_capturing_jit_order_owners: auction.surplus_capturing_jit_order_owners,
        }
    }
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    #[serde_as(as = "serde_ext::Hex")]
    pub uid: [u8; 56],
    pub sell_token: Address,
    pub buy_token: Address,
    #[serde_as(as = "HexOrDecimalU256")]
    pub sell_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub full_sell_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub buy_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub full_buy_amount: U256,
    /// Cap on the penalty a solver can incur for winning this order but
    /// failing to execute it, in native token.
    #[serde_as(as = "Option<HexOrDecimalU256>")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub penalty_cap_native: Option<U256>,
    pub valid_to: u32,
    pub kind: Kind,
    #[serde(default)]
    pub receiver: Receiver,
    pub owner: Address,
    pub partially_fillable: bool,
    pub pre_interactions: Vec<InteractionData>,
    pub post_interactions: Vec<InteractionData>,
    pub sell_token_source: SellTokenSource,
    pub buy_token_destination: BuyTokenDestination,
    /// Deprecated: every order is a limit order. Solvers should not rely on
    /// this field, it is kept for backwards compatibility and will be removed.
    #[serde(default)]
    pub class: Class,
    pub app_data: AppDataHash,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flashloan_hint: Option<FlashloanHint>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub wrappers: Vec<WrapperCall>,
    pub signing_scheme: SigningScheme,
    #[serde(with = "bytes_hex")]
    pub signature: Vec<u8>,
}

/// Destination for which the buyAmount should be transferred to order's
/// receiver to upon fulfillment
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuyTokenDestination {
    /// Pay trade proceeds as an ERC20 token transfer
    Erc20,
    /// Pay trade proceeds as a Vault internal balance transfer
    #[deprecated(
        note = "Balancer Vault token sources are deprecated and no longer appear in auctions; \
                only erc20 is used"
    )]
    Internal,
}

/// Source from which the sellAmount should be drawn upon order fulfillment
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SellTokenSource {
    /// Direct ERC20 allowances to the Vault relayer contract
    Erc20,
    /// Internal balances to the Vault with GPv2 relayer approval
    #[deprecated(
        note = "Balancer Vault token sources are deprecated and no longer appear in auctions; \
                only erc20 is used"
    )]
    External,
    /// ERC20 allowances to the Vault with GPv2 relayer approval
    #[deprecated(
        note = "Balancer Vault token sources are deprecated and no longer appear in auctions; \
                only erc20 is used"
    )]
    Internal,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractionData {
    pub target: Address,
    #[serde_as(as = "HexOrDecimalU256")]
    pub value: U256,
    #[serde(with = "bytes_hex")]
    pub call_data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SigningScheme {
    Eip712,
    EthSign,
    Eip1271,
    PreSign,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Sell,
    Buy,
}

/// Deprecated: every order is a limit order.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Class {
    Market,
    #[default]
    Limit,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Token {
    pub decimals: Option<u8>,
    pub symbol: Option<String>,
    #[serde_as(as = "Option<HexOrDecimalU256>")]
    pub reference_price: Option<U256>,
    #[serde_as(as = "HexOrDecimalU256")]
    pub available_balance: U256,
    pub trusted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Liquidity {
    ConstantProduct(ConstantProductPool),
    WeightedProduct(WeightedProductPool),
    Stable(StablePool),
    ConcentratedLiquidity(ConcentratedLiquidityPool),
    LimitOrder(ForeignLimitOrder),
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConstantProductPool {
    pub id: String,
    pub address: Address,
    pub router: Address,
    #[serde_as(as = "HexOrDecimalU256")]
    pub gas_estimate: U256,
    pub tokens: HashMap<Address, ConstantProductReserve>,
    pub fee: BigDecimal,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConstantProductReserve {
    #[serde_as(as = "HexOrDecimalU256")]
    pub balance: U256,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeightedProductPool {
    pub id: String,
    pub address: Address,
    pub balancer_pool_id: B256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub gas_estimate: U256,
    pub tokens: HashMap<Address, WeightedProductReserve>,
    pub fee: BigDecimal,
    pub version: WeightedProductVersion,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeightedProductReserve {
    #[serde_as(as = "HexOrDecimalU256")]
    pub balance: U256,
    pub scaling_factor: BigDecimal,
    pub weight: BigDecimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WeightedProductVersion {
    V0,
    V3Plus,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StablePool {
    pub id: String,
    pub address: Address,
    pub balancer_pool_id: B256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub gas_estimate: U256,
    pub tokens: HashMap<Address, StableReserve>,
    pub amplification_parameter: BigDecimal,
    pub fee: BigDecimal,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StableReserve {
    #[serde_as(as = "HexOrDecimalU256")]
    pub balance: U256,
    pub scaling_factor: BigDecimal,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConcentratedLiquidityPool {
    pub id: String,
    pub address: Address,
    pub router: Address,
    #[serde_as(as = "HexOrDecimalU256")]
    pub gas_estimate: U256,
    pub tokens: Vec<Address>,
    #[serde_as(as = "HexOrDecimalU256")]
    pub sqrt_price: U256,
    #[serde_as(as = "DisplayFromStr")]
    pub liquidity: u128,
    pub tick: i32,
    // `Arc`-shared with the driver's domain pool to avoid deep-copying the tick
    // map when building the solver request on every `/solve`.
    #[serde_as(as = "Arc<BTreeMap<DisplayFromStr, DisplayFromStr>>")]
    pub liquidity_net: Arc<BTreeMap<i32, i128>>,
    pub fee: BigDecimal,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForeignLimitOrder {
    pub id: String,
    pub address: Address,
    #[serde_as(as = "HexOrDecimalU256")]
    pub gas_estimate: U256,
    #[serde_as(as = "serde_ext::Hex")]
    pub hash: [u8; 32],
    pub maker_token: Address,
    pub taker_token: Address,
    #[serde_as(as = "HexOrDecimalU256")]
    pub maker_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub taker_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub taker_token_fee_amount: U256,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlashloanHint {
    pub liquidity_provider: Address,
    pub protocol_adapter: Address,
    pub receiver: Address,
    pub token: Address,
    #[serde_as(as = "HexOrDecimalU256")]
    pub amount: U256,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WrapperCall {
    pub address: Address,
    #[serde(with = "bytes_hex")]
    pub data: Vec<u8>,
    /// Declares whether this wrapper (and its data) needs to be included
    /// unmodified in a solution containing this order.
    #[serde(default)]
    pub is_omittable: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn auction(id: Id) -> Auction {
        Auction {
            id,
            tokens: Default::default(),
            orders: Default::default(),
            liquidity: Default::default(),
            effective_gas_price: U256::from(1),
            deadline: chrono::DateTime::from_timestamp(0, 0).unwrap(),
            surplus_capturing_jit_order_owners: Default::default(),
        }
    }

    // `id` is always present (`null` for quotes, which external solvers rely on
    // to tell a quote from an auction); `quoteId` and `priceQuality` appear for
    // every quote, `fastPath` only for fast-path quotes. Pins the wire shape
    // and the round-trip so the typed enum can never silently change either.
    #[test]
    fn wire_format_and_roundtrip() {
        for (id, id_json, quote_id, fast_path, price_quality) in [
            (Id::Auction(1), serde_json::json!("1"), None, false, None),
            (
                Id::Quote {
                    id: 2,
                    fast_path: false,
                    price_quality: PriceQuality::Verified,
                },
                serde_json::Value::Null,
                Some("2"),
                false,
                Some("verified"),
            ),
            (
                Id::Quote {
                    id: 3,
                    fast_path: true,
                    price_quality: PriceQuality::Optimal,
                },
                serde_json::Value::Null,
                Some("3"),
                true,
                Some("optimal"),
            ),
        ] {
            let v = serde_json::to_value(auction(id)).unwrap();
            assert_eq!(v["id"], id_json);
            assert_eq!(
                v.get("quoteId"),
                quote_id.map(|q| serde_json::json!(q)).as_ref()
            );
            assert_eq!(
                v.get("fastPath"),
                fast_path.then_some(serde_json::json!(true)).as_ref()
            );
            assert_eq!(
                v.get("priceQuality"),
                price_quality.map(|q| serde_json::json!(q)).as_ref()
            );
            assert_eq!(serde_json::from_value::<Auction>(v).unwrap().id, id);
        }
    }

    #[test]
    fn rejects_invalid_states() {
        let rest = r#""tokens":{},"orders":[],"liquidity":[],"effectiveGasPrice":"1","deadline":"1970-01-01T00:00:00Z","surplusCapturingJitOrderOwners":[]"#;
        for head in [
            r#""id":"1","quoteId":"2""#,             // auction and quote
            r#""id":"1","fastPath":true"#,           // auction with fast path
            r#""id":"1","priceQuality":"verified""#, // auction with price quality
            r#""quoteId":"2""#,                      // quote without price quality
            "",                                      // neither id nor quoteId
        ] {
            let json = format!("{{{head}{}{rest}}}", if head.is_empty() { "" } else { "," });
            assert!(serde_json::from_str::<Auction>(&json).is_err(), "{json}");
        }
    }
}
