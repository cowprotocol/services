use {
    crate::{
        boundary,
        domain,
        infra::{
            persistence::dto::{self, order::Order},
            solvers::InjectIntoHttpRequest,
        },
    },
    alloy::primitives::{Address, U256},
    brotli::enc::writer::CompressorWriter,
    bytes::Bytes,
    chrono::{DateTime, Utc},
    eth_domain_types as eth,
    itertools::Itertools,
    number::serialization::HexOrDecimalU256,
    observe::http_body::Measured,
    reqwest::{RequestBuilder, header::HeaderValue},
    serde::{Deserialize, Serialize},
    serde_with::{DisplayFromStr, serde_as},
    std::{
        borrow::Cow,
        collections::{HashMap, HashSet},
        convert::Infallible,
        io::Write,
        time::Duration,
    },
};

/// Cheaply clonable handle to an already JSON serialized
/// request. The purpose of this is to make it ergonomic
/// to serialize a request once and reuse the resulting
/// string in multiple HTTP requests.
#[derive(Clone, Debug)]
pub struct Request {
    auction_id: i64,
    body: bytes::Bytes,
    content_encoding: Option<HeaderValue>,
    deadline: chrono::DateTime<chrono::Utc>,
}

impl Request {
    pub async fn new(
        auction: &domain::Auction,
        trusted_tokens: &HashSet<Address>,
        deadline: chrono::DateTime<chrono::Utc>,
        compress: bool,
    ) -> Self {
        let helper = FullRequestHelper {
            id: auction.id,
            orders: auction.orders.iter().map(dto::order::from_domain).collect(),
            tokens: tokens(auction, trusted_tokens),
            deadline,
            surplus_capturing_jit_order_owners: auction.surplus_capturing_jit_order_owners.to_vec(),
        };
        Self::from_body(RequestBody::Full(helper), compress).await
    }

    /// Builds a request containing the delta to the previously sent auction.
    pub async fn new_delta(
        previous: &domain::Auction,
        current: &domain::Auction,
        trusted_tokens: &HashSet<Address>,
        deadline: chrono::DateTime<chrono::Utc>,
        compress: bool,
    ) -> Self {
        let helper = DeltaRequestHelper {
            id: current.id,
            tokens: tokens(current, trusted_tokens),
            orders: OrderDelta::compute(&previous.orders, &current.orders),
            deadline,
            surplus_capturing_jit_order_owners: current.surplus_capturing_jit_order_owners.to_vec(),
        };

        Self::from_body(RequestBody::Delta(helper), compress).await
    }

    async fn from_body(body: RequestBody, compress: bool) -> Self {
        let _timer =
            observe::metrics::metrics().on_auction_overhead_start("autopilot", "serialize_request");
        let (auction_id, deadline) = (body.id(), body.deadline());

        let (body, content_encoding) = tokio::task::spawn_blocking(move || {
            let serialized = serde_json::to_vec(&body).expect("auction is JSON serializable");

            if !compress {
                return (Bytes::from(serialized), None);
            }

            // quality 1: fastest brotli level. Already beats gzip-3 on both
            // ratio and speed for our JSON payloads.
            //
            // lgwin 22: LZ77 window = 2^22 - 16 ≈ 4 MB. How far back the
            // compressor looks for repeated patterns. The decompressor must
            // allocate up to this much memory. Aligns with our current auction size
            // (~3-4mb).
            //
            // 4096: internal I/O buffer for flushing to the output Vec.
            // Doesn't affect compression ratio. Tested 512 B to 256 KB with
            // no meaningful difference; 4 KB is a standard default.
            let mut encoder = CompressorWriter::new(Vec::new(), 4096, 1, 22);
            match encoder.write_all(&serialized).and_then(|_| encoder.flush()) {
                Ok(()) => (
                    Bytes::from(encoder.into_inner()),
                    Some(HeaderValue::from_static("br")),
                ),
                Err(err) => {
                    tracing::error!(
                        ?err,
                        "brotli compression failed, falling back to uncompressed"
                    );
                    (Bytes::from(serialized), None)
                }
            }
        })
        .await
        .expect("inner task should not panic as serialization should work for the given type");

        Self {
            body,
            auction_id,
            content_encoding,
            deadline,
        }
    }

    pub fn body_size(&self) -> usize {
        self.body.len()
    }

    pub fn time_until_deadline(&self) -> Duration {
        self.deadline
            .signed_duration_since(Utc::now())
            .to_std()
            .unwrap_or(Duration::ZERO)
    }
}

/// The two kinds of `/solve` request bodies, distinguished by an internal
/// `kind` tag: the full auction or - for drivers that opted in - only the
/// difference to a previously sent auction.
///
/// The tag is also set on full auctions. That is a purely additive change for
/// drivers that predate delta requests: they ignore the unknown field.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum RequestBody {
    Full(FullRequestHelper),
    Delta(DeltaRequestHelper),
}

impl RequestBody {
    fn id(&self) -> i64 {
        match self {
            RequestBody::Full(FullRequestHelper { id, .. }) => *id,
            RequestBody::Delta(DeltaRequestHelper { id, .. }) => *id,
        }
    }

    fn deadline(&self) -> DateTime<Utc> {
        match self {
            RequestBody::Full(FullRequestHelper { deadline, .. }) => *deadline,
            RequestBody::Delta(DeltaRequestHelper { deadline, .. }) => *deadline,
        }
    }
}

/// Builds the token list of an auction: all tokens with a known price plus
/// all trusted tokens.
fn tokens(auction: &domain::Auction, trusted_tokens: &HashSet<Address>) -> Vec<Token> {
    auction
        .prices
        .iter()
        .map(|(address, price)| Token {
            address: *address.to_owned(),
            price: Some(price.get().0),
            trusted: trusted_tokens.contains(&Address::from(*address)),
        })
        .chain(trusted_tokens.iter().map(|&address| Token {
            address,
            price: None,
            trusted: true,
        }))
        .unique_by(|token| token.address)
        .collect()
}

impl InjectIntoHttpRequest for Request {
    fn inject(&self, request: RequestBuilder) -> RequestBuilder {
        let body = futures::stream::iter([Ok::<_, Infallible>(self.body.clone())]);
        let request = request
            .body(reqwest::Body::wrap_stream(Measured::new(body)))
            // announce which auction this request is for in the
            // headers to help the driver detect duplicated
            // `/solve` requests before streaming the body
            .header("X-Auction-Id", self.auction_id)
            // manually set the content type header for JSON since
            // we can't use `request.json(self)`
            .header(
                reqwest::header::CONTENT_TYPE,
                reqwest::header::HeaderValue::from_static("application/json"),
            );
        if let Some(encoding) = &self.content_encoding {
            request.header(reqwest::header::CONTENT_ENCODING, encoding)
        } else {
            request
        }
    }

    fn body_to_string(&self) -> Cow<'_, str> {
        if self.content_encoding.is_some() {
            return Cow::Borrowed("<compressed>");
        }
        let string = str::from_utf8(self.body.as_ref()).unwrap();
        Cow::Borrowed(string)
    }
}

impl Response {
    pub fn into_domain(self) -> Vec<domain::competition::Solution> {
        if self
            .solutions
            .iter()
            .any(|solution| !solution.clearing_prices.is_empty())
        {
            tracing::debug!("driver sent deprecated clearingPrices field");
        }
        self.solutions
            .into_iter()
            .map(Solution::into_domain)
            .collect()
    }
}

#[serde_as]
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct FullRequestHelper {
    #[serde_as(as = "DisplayFromStr")]
    pub id: i64,
    pub tokens: Vec<Token>,
    pub orders: Vec<Order>,
    pub deadline: DateTime<Utc>,
    pub surplus_capturing_jit_order_owners: Vec<Address>,
}

/// Difference of an auction relative to the previous (i.e. `id - 1`).
/// Only orders are diffed; tokens and all scalar fields are sent whole.
#[serde_as]
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeltaRequestHelper {
    #[serde_as(as = "DisplayFromStr")]
    id: i64,
    tokens: Vec<Token>,
    #[serde(flatten)]
    orders: OrderDelta,
    deadline: DateTime<Utc>,
    surplus_capturing_jit_order_owners: Vec<Address>,
}

/// The orders of an auction expressed as the difference to a previous auction.
/// Flattened into [`DeltaHelper`], so the two fields sit next to `tokens` and
/// friends on the wire.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OrderDelta {
    /// Orders that were added or modified since the base auction.
    updated_orders: Vec<Order>,
    /// Uids of orders that were removed since the base auction.
    removed_orders: Vec<boundary::OrderUid>,
}

impl OrderDelta {
    /// Diffs the orders of two auctions, matching them by uid. An order counts
    /// as updated when it is new or when any of its fields changed.
    fn compute(previous: &[domain::Order], current: &[domain::Order]) -> Self {
        // Matched orders are taken out of the map, so whatever is left at the
        // end is exactly the set of removed orders. That saves both a second
        // hash set holding the uids of `current` and a second pass over
        // `previous`.
        let mut unmatched: HashMap<_, _> =
            previous.iter().map(|order| (order.uid, order)).collect();

        let mut updated_orders = Vec::new();
        for order in current {
            let changed = unmatched
                .remove(&order.uid)
                .is_none_or(|previous| previous != order);
            if changed {
                updated_orders.push(dto::order::from_domain(order));
            }
        }

        let removed_orders: Vec<boundary::OrderUid> =
            unmatched.into_keys().map(Into::into).collect();

        Self {
            updated_orders,
            removed_orders,
        }
    }
}

#[serde_as]
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Token {
    pub address: Address,
    #[serde_as(as = "Option<HexOrDecimalU256>")]
    pub price: Option<U256>,
    pub trusted: bool,
}

impl Solution {
    pub fn into_domain(self) -> domain::competition::Solution {
        domain::competition::Solution::new(
            self.solution_id,
            self.submission_address,
            self.orders
                .into_iter()
                .map(|(o, amounts)| (o.into(), amounts.into_domain()))
                .collect(),
        )
    }
}

/// Contains basic order information and the executed amounts. Basic order
/// information are required because of JIT orders which are not part of an
/// auction, so autopilot can be aware of them before the solution is
/// settled on-chain.
#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradedOrder {
    side: Side,
    sell_token: Address,
    buy_token: Address,
    /// Sell limit order amount.
    #[serde_as(as = "HexOrDecimalU256")]
    limit_sell: U256,
    /// Buy limit order amount.
    #[serde_as(as = "HexOrDecimalU256")]
    limit_buy: U256,
    /// The effective amount that left the user's wallet including all fees.
    #[serde_as(as = "HexOrDecimalU256")]
    executed_sell: U256,
    /// The effective amount the user received after all fees.
    #[serde_as(as = "HexOrDecimalU256")]
    executed_buy: U256,
}

impl TradedOrder {
    pub fn into_domain(self) -> domain::competition::TradedOrder {
        domain::competition::TradedOrder {
            sell: eth::Asset {
                token: self.sell_token.into(),
                amount: self.limit_sell.into(),
            },
            buy: eth::Asset {
                token: self.buy_token.into(),
                amount: self.limit_buy.into(),
            },
            side: match self.side {
                Side::Buy => domain::auction::order::Side::Buy,
                Side::Sell => domain::auction::order::Side::Sell,
            },
            executed_sell: self.executed_sell.into(),
            executed_buy: self.executed_buy.into(),
        }
    }
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Side {
    Buy,
    Sell,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Solution {
    /// Unique ID of the solution (per driver competition), used to identify
    /// it in subsequent requests (reveal, settle).
    pub solution_id: u64,
    /// Address used by the driver to submit the settlement onchain.
    pub submission_address: Address,
    pub orders: HashMap<boundary::OrderUid, TradedOrder>,
    /// Deprecated: uniform clearing prices are no longer used by the
    /// autopilot. Kept here purely so we can detect and log drivers that
    /// still send them, in order to chase them down before the field is
    /// removed entirely.
    #[serde(default)]
    #[serde_as(as = "HashMap<_, HexOrDecimalU256>")]
    pub clearing_prices: HashMap<Address, U256>,
    pub gas: Option<u64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    pub solutions: Vec<Solution>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_json() -> Vec<u8> {
        let json_value = serde_json::json!({
            "id": "1",
            "tokens": (0..100).map(|i| {
                serde_json::json!({
                    "address": format!("0x{:040x}", i),
                    "price": format!("{}", i * 1000),
                    "trusted": i % 2 == 0
                })
            }).collect::<Vec<_>>(),
            "orders": [],
            "deadline": "2025-01-01T00:00:00Z",
            "surplusCapturingJitOrderOwners": []
        });
        serde_json::to_vec(&json_value).unwrap()
    }

    fn uncompressed_request(json: Vec<u8>) -> Request {
        Request {
            auction_id: 1,
            body: Bytes::from(json),
            content_encoding: None,
            deadline: Utc::now(),
        }
    }

    fn compressed_request(json: &[u8]) -> Request {
        use brotli::enc::writer::CompressorWriter;

        let mut encoder = CompressorWriter::new(Vec::new(), 4096, 1, 22);
        encoder.write_all(json).unwrap();
        encoder.flush().unwrap();
        let compressed = encoder.into_inner();
        Request {
            auction_id: 1,
            body: Bytes::from(compressed),
            content_encoding: Some(HeaderValue::from_static("br")),
            deadline: Utc::now(),
        }
    }

    #[test]
    fn compressed_request_round_trips() {
        let json = make_test_json();

        let request = compressed_request(&json);
        assert_eq!(
            request.content_encoding.as_ref().map(|v| v.as_bytes()),
            Some("br".as_bytes())
        );
        assert!(
            request.body.len() < json.len(),
            "compressed body {} should be smaller than original {}",
            request.body.len(),
            json.len(),
        );

        let mut decompressed = Vec::new();
        brotli::BrotliDecompress(&mut request.body.as_ref(), &mut decompressed).unwrap();
        assert_eq!(decompressed, json);
    }

    #[test]
    fn uncompressed_request_preserves_json() {
        let json = make_test_json();
        let request = uncompressed_request(json.clone());

        assert_eq!(request.content_encoding, None);
        assert_eq!(request.body.as_ref(), json.as_slice());
    }

    /// Uid with all 56 bytes set to `uid_byte`.
    fn uid(uid_byte: u8) -> boundary::OrderUid {
        boundary::OrderUid([uid_byte; 56])
    }

    fn uid_json(uid_byte: u8) -> String {
        format!("0x{}", format!("{uid_byte:02x}").repeat(56))
    }

    /// JSON of a minimal order as serialized by [`RequestHelper`], with all
    /// bytes of the uid set to `uid_byte`.
    fn order_json(uid_byte: u8, executed: u64) -> serde_json::Value {
        serde_json::json!({
            "uid": uid_json(uid_byte),
            "sellToken": "0x2222222222222222222222222222222222222222",
            "buyToken": "0x3333333333333333333333333333333333333333",
            "sellAmount": "1000",
            "buyAmount": "2000",
            "protocolFees": [],
            "created": 1,
            "validTo": 2,
            "kind": "sell",
            "receiver": null,
            "owner": "0x4444444444444444444444444444444444444444",
            "partiallyFillable": true,
            "executed": executed.to_string(),
            "preInteractions": [],
            "postInteractions": [],
            "sellTokenBalance": "erc20",
            "buyTokenBalance": "erc20",
            "class": "limit",
            "appData": format!("0x{}", "00".repeat(32)),
            "signingScheme": "eip712",
            "signature": format!("0x{}1c", "01".repeat(64)),
            "quote": null,
        })
    }

    fn test_order(uid_byte: u8, executed: u64) -> domain::Order {
        dto::order::to_domain(serde_json::from_value(order_json(uid_byte, executed)).unwrap())
    }

    fn test_auction(id: i64, orders: Vec<domain::Order>) -> domain::Auction {
        domain::Auction {
            id,
            block: 1,
            orders,
            prices: Default::default(),
            surplus_capturing_jit_order_owners: vec![],
        }
    }

    fn test_deadline() -> DateTime<Utc> {
        DateTime::from_timestamp(1_700_000_000, 0).unwrap()
    }

    #[tokio::test]
    async fn full_request_is_tagged() {
        let auction = test_auction(1, vec![test_order(0x11, 0)]);
        let request = Request::new(&auction, &HashSet::new(), test_deadline(), false).await;
        let body: serde_json::Value = serde_json::from_str(&request.body_to_string()).unwrap();

        // The `kind` tag is the only addition compared to the body drivers
        // received before delta requests existed; every other field keeps its
        // name and place, so drivers that ignore unknown fields are unaffected.
        assert_eq!(body.get("kind"), Some(&serde_json::json!("full")));
        assert_eq!(body.get("id"), Some(&serde_json::json!("1")));
        assert_eq!(
            body.get("orders"),
            Some(&serde_json::json!([order_json(0x11, 0)]))
        );
    }

    /// The wire shape of a delta body, mirrored in the driver's tests
    /// (crates/driver/src/infra/api/routes/solve/dto/solve_request.rs) to
    /// pin the format both sides agree on. Note that the two [`OrderDelta`]
    /// fields are flattened into the body instead of nested.
    #[tokio::test]
    async fn delta_request_wire_format() {
        let previous = test_auction(1, vec![test_order(0x33, 0)]);
        let current = test_auction(2, vec![test_order(0x44, 0)]);
        let request =
            Request::new_delta(&previous, &current, &HashSet::new(), test_deadline(), false).await;

        let actual: serde_json::Value = serde_json::from_str(&request.body_to_string()).unwrap();
        let expected = serde_json::json!({
            "kind": "delta",
            "id": "2",
            "tokens": [],
            "updatedOrders": [order_json(0x44, 0)],
            "removedOrders": [uid_json(0x33)],
            "deadline": "2023-11-14T22:13:20Z",
            "surplusCapturingJitOrderOwners": [],
        });
        assert_eq!(actual, expected);
    }

    #[test]
    fn order_delta_splits_updated_from_removed() {
        let unchanged = test_order(0x11, 0);
        let previous = [unchanged.clone(), test_order(0x22, 0), test_order(0x33, 0)];
        let current = [
            unchanged,
            // same uid, but partially filled by now
            test_order(0x22, 100),
            // brand new order
            test_order(0x44, 0),
        ];

        let delta = OrderDelta::compute(&previous, &current);

        let updated: Vec<_> = delta
            .updated_orders
            .iter()
            .map(|order| (order.uid, order.executed))
            .collect();
        assert_eq!(
            updated,
            [(uid(0x22), U256::from(100)), (uid(0x44), U256::ZERO)]
        );
        assert_eq!(delta.removed_orders, [uid(0x33)]);
    }

    #[test]
    fn order_delta_is_empty_for_identical_orders() {
        let orders = [test_order(0x11, 0), test_order(0x22, 0)];

        let delta = OrderDelta::compute(&orders, &orders);

        assert!(delta.updated_orders.is_empty());
        assert!(delta.removed_orders.is_empty());
    }
}
