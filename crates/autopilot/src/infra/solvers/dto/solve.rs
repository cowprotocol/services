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
        let helper = RequestHelper {
            id: auction.id,
            orders: auction.orders.iter().map(dto::order::from_domain).collect(),
            tokens: tokens(auction, trusted_tokens),
            deadline,
            surplus_capturing_jit_order_owners: auction.surplus_capturing_jit_order_owners.to_vec(),
        };
        Self::from_body(RequestBody::Full(helper), compress).await
    }

    /// Builds a request containing only the difference to a previously sent
    /// auction. Use [`DeltaBase::delta`] to compute the payload.
    pub async fn new_delta(delta: Delta, compress: bool) -> Self {
        Self::from_body(RequestBody::Delta(delta.0), compress).await
    }

    async fn from_body(body: RequestBody, compress: bool) -> Self {
        let _timer =
            observe::metrics::metrics().on_auction_overhead_start("autopilot", "serialize_request");
        let (auction_id, deadline) = match &body {
            RequestBody::Full(helper) => (helper.id, helper.deadline),
            RequestBody::Delta(helper) => (helper.id, helper.deadline),
        };

        let (body, content_encoding) = tokio::task::spawn_blocking(move || {
            let serialized = body.serialize();

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
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum RequestBody {
    Full(RequestHelper),
    Delta(DeltaHelper),
}

impl RequestBody {
    /// Full requests are serialized without the `kind` tag so their JSON
    /// stays byte-compatible with drivers that predate delta requests. Only
    /// new request kinds carry the tag.
    fn serialize(&self) -> Vec<u8> {
        match self {
            Self::Full(helper) => serde_json::to_vec(helper),
            Self::Delta(_) => serde_json::to_vec(self),
        }
        .expect("type should be JSON serializable")
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

/// Orders of the most recently sent auction. Used as the base to compute the
/// next auction's delta request against.
pub struct DeltaBase {
    auction_id: i64,
    orders: HashMap<domain::OrderUid, domain::Order>,
}

impl DeltaBase {
    pub fn new(auction: &domain::Auction) -> Self {
        Self {
            auction_id: auction.id,
            orders: auction
                .orders
                .iter()
                .map(|order| (order.uid, order.clone()))
                .collect(),
        }
    }

    /// Computes the payload of a delta request for `auction` against this
    /// base. Only orders are diffed since they make up ~99% of the auction's
    /// bytes; everything else is always sent whole. Returns `None` when so
    /// many orders changed that sending the full auction is cheaper.
    pub fn delta(
        &self,
        auction: &domain::Auction,
        trusted_tokens: &HashSet<Address>,
        deadline: chrono::DateTime<chrono::Utc>,
    ) -> Option<Delta> {
        let mut updated_orders = Vec::new();
        let mut current_uids = HashSet::with_capacity(auction.orders.len());
        for order in &auction.orders {
            current_uids.insert(order.uid);
            if self.orders.get(&order.uid) != Some(order) {
                updated_orders.push(dto::order::from_domain(order));
            }
        }
        let removed_orders: Vec<boundary::OrderUid> = self
            .orders
            .keys()
            .filter(|uid| !current_uids.contains(uid))
            .map(|&uid| uid.into())
            .collect();
        // Rough byte heuristic to detect that the delta would not be
        // meaningfully smaller than the full auction: an updated order
        // weighs like a full one and a removed order's uid ~1/10th of that.
        if updated_orders.len() + removed_orders.len() / 10 > auction.orders.len() / 2 {
            return None;
        }
        Some(Delta(DeltaHelper {
            id: auction.id,
            base_id: self.auction_id,
            tokens: tokens(auction, trusted_tokens),
            updated_orders,
            removed_orders,
            deadline,
            surplus_capturing_jit_order_owners: auction.surplus_capturing_jit_order_owners.to_vec(),
        }))
    }
}

/// Opaque payload of a delta request, built by [`DeltaBase::delta`].
#[derive(Clone)]
pub struct Delta(DeltaHelper);

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
struct RequestHelper {
    #[serde_as(as = "DisplayFromStr")]
    pub id: i64,
    pub tokens: Vec<Token>,
    pub orders: Vec<Order>,
    pub deadline: DateTime<Utc>,
    pub surplus_capturing_jit_order_owners: Vec<Address>,
}

/// Difference of an auction relative to the auction `base_id` which the
/// receiving driver is expected to still have. Only orders are diffed;
/// tokens and all scalar fields are sent whole.
#[serde_as]
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeltaHelper {
    #[serde_as(as = "DisplayFromStr")]
    pub id: i64,
    /// Id of the auction this delta applies to.
    #[serde_as(as = "DisplayFromStr")]
    pub base_id: i64,
    pub tokens: Vec<Token>,
    /// Orders that were added or modified since the base auction.
    pub updated_orders: Vec<Order>,
    /// Uids of orders that were removed since the base auction.
    pub removed_orders: Vec<boundary::OrderUid>,
    pub deadline: DateTime<Utc>,
    pub surplus_capturing_jit_order_owners: Vec<Address>,
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

    /// JSON of a minimal order as serialized by [`RequestHelper`], with all
    /// bytes of the uid set to `uid_byte`.
    fn order_json(uid_byte: u8, executed: u64) -> serde_json::Value {
        serde_json::json!({
            "uid": format!("0x{}", format!("{uid_byte:02x}").repeat(56)),
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
    async fn full_request_is_not_tagged() {
        let auction = test_auction(1, vec![test_order(0x11, 0)]);
        let request = Request::new(&auction, &HashSet::new(), test_deadline(), false).await;
        let body: serde_json::Value = serde_json::from_str(&request.body_to_string()).unwrap();

        // Drivers that predate delta requests must keep receiving the exact
        // JSON they got before.
        assert_eq!(body.get("kind"), None);
        assert_eq!(body.get("id"), Some(&serde_json::json!("1")));
    }

    /// The shape of this request is mirrored in the driver's tests
    /// (crates/driver/src/infra/api/routes/solve/dto/solve_request.rs) to
    /// pin the wire format both sides agree on.
    #[tokio::test]
    async fn delta_request_contains_order_diff_and_kind_tag() {
        let unchanged = [test_order(0x11, 0), test_order(0x55, 0)];
        let modified = test_order(0x22, 0);
        let removed = test_order(0x33, 0);

        let base_auction = test_auction(
            1,
            vec![
                unchanged[0].clone(),
                unchanged[1].clone(),
                modified,
                removed,
            ],
        );
        let auction = test_auction(
            2,
            vec![
                unchanged[0].clone(),
                unchanged[1].clone(),
                test_order(0x22, 100),
                test_order(0x44, 0),
            ],
        );

        let delta = DeltaBase::new(&base_auction)
            .delta(&auction, &HashSet::new(), test_deadline())
            .unwrap();
        let request = Request::new_delta(delta, false).await;

        let actual: serde_json::Value = serde_json::from_str(&request.body_to_string()).unwrap();
        let expected = serde_json::json!({
            "kind": "delta",
            "id": "2",
            "baseId": "1",
            "tokens": [],
            "updatedOrders": [order_json(0x22, 100), order_json(0x44, 0)],
            "removedOrders": [format!("0x{}", "33".repeat(56))],
            "deadline": "2023-11-14T22:13:20Z",
            "surplusCapturingJitOrderOwners": [],
        });
        assert_eq!(actual, expected);
    }

    #[test]
    fn no_delta_when_too_many_orders_changed() {
        let base_auction = test_auction(1, vec![test_order(0x11, 0), test_order(0x22, 0)]);
        // 2 out of 3 orders changed, more than half the auction.
        let auction = test_auction(
            2,
            vec![
                test_order(0x11, 0),
                test_order(0x22, 100),
                test_order(0x33, 0),
            ],
        );

        let base = DeltaBase::new(&base_auction);
        assert!(
            base.delta(&auction, &HashSet::new(), test_deadline())
                .is_none()
        );
    }

    #[test]
    fn no_delta_when_too_many_orders_removed() {
        // 28 of 30 orders removed: the delta (a long list of removed uids)
        // would not be meaningfully smaller than the tiny full auction.
        let base_auction = test_auction(1, (1..=30).map(|uid| test_order(uid, 0)).collect());
        let auction = test_auction(2, vec![test_order(1, 0), test_order(2, 0)]);

        let base = DeltaBase::new(&base_auction);
        assert!(
            base.delta(&auction, &HashSet::new(), test_deadline())
                .is_none()
        );
    }

    #[test]
    fn empty_delta_for_identical_auction() {
        let orders = vec![test_order(0x11, 0), test_order(0x22, 0)];
        let base_auction = test_auction(1, orders.clone());
        let auction = test_auction(2, orders);

        let base = DeltaBase::new(&base_auction);
        let delta = base
            .delta(&auction, &HashSet::new(), test_deadline())
            .unwrap();
        assert!(delta.0.updated_orders.is_empty());
        assert!(delta.0.removed_orders.is_empty());
        assert_eq!(delta.0.base_id, 1);
        assert_eq!(delta.0.id, 2);
    }

    /// Measures full vs delta request sizes over real consecutive auctions.
    ///
    /// Point `AUCTION_DATA_DIR` at a directory of `<auction_id>.json` files
    /// containing the `auctions.json` DB column of consecutive auctions
    /// (e.g. collected by polling the live table) and run with:
    ///
    /// AUCTION_DATA_DIR=... cargo nextest run -p autopilot \
    ///   delta_request_size_benchmark --run-ignored ignored-only \
    ///   --no-capture
    #[tokio::test]
    #[ignore]
    async fn delta_request_size_benchmark() {
        let dir = std::env::var("AUCTION_DATA_DIR")
            .expect("set AUCTION_DATA_DIR to a directory of <auction_id>.json files");
        let mut files: Vec<(i64, std::path::PathBuf)> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|entry| {
                let path = entry.unwrap().path();
                let id = path.file_stem()?.to_str()?.parse().ok()?;
                (path.extension()? == "json").then_some((id, path))
            })
            .collect();
        files.sort();
        assert!(files.len() >= 2, "need at least 2 auctions to diff");

        let auctions: Vec<domain::Auction> = files
            .iter()
            .map(|(id, path)| {
                let raw: dto::auction::RawAuctionData =
                    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
                dto::auction::Auction {
                    id: *id,
                    auction: raw,
                }
                .try_into_domain()
                .unwrap()
            })
            .collect();
        println!(
            "loaded {} auctions, ids {}..{}",
            auctions.len(),
            auctions.first().unwrap().id,
            auctions.last().unwrap().id
        );

        let trusted = HashSet::new();
        let deadline = test_deadline();
        let mut stats: Vec<[usize; 4]> = Vec::new();
        let mut churn = (0usize, 0usize);
        for pair in auctions.windows(2) {
            let [prev, curr] = pair else { unreachable!() };
            if curr.id != prev.id + 1 {
                println!("skipping non-consecutive pair {} -> {}", prev.id, curr.id);
                continue;
            }
            let full_raw = Request::new(curr, &trusted, deadline, false).await;
            let full_br = Request::new(curr, &trusted, deadline, true).await;
            let base = DeltaBase::new(prev);
            let Some(delta) = base.delta(curr, &trusted, deadline) else {
                println!(
                    "skipping heavy-churn pair {} -> {} (full auction would be sent)",
                    prev.id, curr.id
                );
                continue;
            };
            churn.0 += delta.0.updated_orders.len();
            churn.1 += delta.0.removed_orders.len();
            let delta_raw = Request::new_delta(delta.clone(), false).await;
            let delta_br = Request::new_delta(delta, true).await;
            stats.push([
                full_raw.body_size(),
                full_br.body_size(),
                delta_raw.body_size(),
                delta_br.body_size(),
            ]);
        }

        let n = stats.len();
        assert!(n > 0, "no consecutive auction pairs to measure");
        let col = |i: usize| {
            let mut values: Vec<usize> = stats.iter().map(|row| row[i]).collect();
            values.sort();
            (
                values.iter().sum::<usize>() / n,
                values[n / 2],
                values[n * 9 / 10],
                values[n - 1],
            )
        };
        println!(
            "pairs: {n}, orders updated/removed per pair: {:.1}/{:.1}",
            churn.0 as f64 / n as f64,
            churn.1 as f64 / n as f64
        );
        println!(
            "{:<14} {:>12} {:>12} {:>12} {:>12}",
            "", "mean", "p50", "p90", "max"
        );
        for (name, i) in [
            ("full raw", 0),
            ("full brotli", 1),
            ("delta raw", 2),
            ("delta brotli", 3),
        ] {
            let (mean, p50, p90, max) = col(i);
            println!("{name:<14} {mean:>12} {p50:>12} {p90:>12} {max:>12}");
        }
        let (full_br_mean, ..) = col(1);
        let (delta_br_mean, ..) = col(3);
        println!("\navg bytes/auction at checkpoint interval X (brotli):");
        for x in [5usize, 10, 20, 50, 100] {
            let avg = (full_br_mean + (x - 1) * delta_br_mean) / x;
            println!(
                "  X={x:<4} {avg:>12}  ({:.1}% of full)",
                100.0 * avg as f64 / full_br_mean as f64
            );
        }
    }
}
