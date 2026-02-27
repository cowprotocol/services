use {
    crate::{
        boundary,
        domain::{self, eth},
        infra::{
            persistence::dto::{self, order::Order},
            solvers::{InjectIntoHttpRequest, byte_stream::ByteStream},
        },
    },
    alloy::primitives::{Address, U256},
    brotli::enc::writer::CompressorWriter,
    bytes::Bytes,
    chrono::{DateTime, Utc},
    itertools::Itertools,
    number::serialization::HexOrDecimalU256,
    reqwest::{RequestBuilder, header::HeaderValue},
    serde::{Deserialize, Serialize},
    serde_with::{DisplayFromStr, serde_as},
    std::{
        borrow::Cow,
        collections::{HashMap, HashSet},
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
    raw_body: Bytes,
    compressed_body: Option<Bytes>,
    use_compressed: bool,
}

impl Request {
    pub async fn new(
        auction: &domain::Auction,
        trusted_tokens: &HashSet<Address>,
        time_limit: Duration,
        compress: bool,
    ) -> Self {
        let _timer =
            observe::metrics::metrics().on_auction_overhead_start("autopilot", "serialize_request");
        let helper = RequestHelper {
            id: auction.id,
            orders: auction
                .orders
                .clone()
                .into_iter()
                .map(dto::order::from_domain)
                .collect(),
            tokens: auction
                .prices
                .iter()
                .map(|(address, price)| Token {
                    address: address.to_owned().0,
                    price: Some(price.get().0),
                    trusted: trusted_tokens.contains(&(address.0)),
                })
                .chain(trusted_tokens.iter().map(|&address| Token {
                    address,
                    price: None,
                    trusted: true,
                }))
                .unique_by(|token| token.address)
                .collect(),
            deadline: Utc::now() + chrono::Duration::from_std(time_limit).unwrap(),
            surplus_capturing_jit_order_owners: auction.surplus_capturing_jit_order_owners.to_vec(),
        };
        let auction_id = auction.id;

        let (raw_body, compressed_body) = tokio::task::spawn_blocking(move || {
            let serialized = serde_json::to_vec(&helper).expect("type should be JSON serializable");

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
                    Bytes::from(serialized),
                    Some(Bytes::from(encoder.into_inner())),
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
            auction_id,
            raw_body,
            compressed_body,
            use_compressed: false,
        }
    }

    pub fn for_driver(&self, compress: bool) -> Self {
        Self {
            use_compressed: compress && self.compressed_body.is_some(),
            ..self.clone()
        }
    }

    pub fn body_size(&self) -> usize {
        self.raw_body.len()
    }
}

impl InjectIntoHttpRequest for Request {
    fn inject(&self, request: RequestBuilder) -> RequestBuilder {
        let (body, encoding) = if self.use_compressed {
            (
                self.compressed_body.clone().expect("checked in for_driver"),
                Some(HeaderValue::from_static("br")),
            )
        } else {
            (self.raw_body.clone(), None)
        };

        let request = request
            .body(reqwest::Body::wrap_stream(ByteStream::new(body)))
            .header("X-Auction-Id", self.auction_id)
            .header(
                reqwest::header::CONTENT_TYPE,
                reqwest::header::HeaderValue::from_static("application/json"),
            );
        if let Some(encoding) = encoding {
            request.header(reqwest::header::CONTENT_ENCODING, encoding)
        } else {
            request
        }
    }

    fn body_to_string(&self) -> Cow<'_, str> {
        if self.use_compressed {
            return Cow::Borrowed("<compressed>");
        }
        let string = str::from_utf8(self.raw_body.as_ref()).unwrap();
        Cow::Borrowed(string)
    }
}

impl Response {
    pub fn into_domain(
        self,
    ) -> Vec<Result<domain::competition::Solution, domain::competition::SolutionError>> {
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
    pub fn into_domain(
        self,
    ) -> Result<domain::competition::Solution, domain::competition::SolutionError> {
        Ok(domain::competition::Solution::new(
            self.solution_id,
            self.submission_address,
            self.orders
                .into_iter()
                .map(|(o, amounts)| (o.into(), amounts.into_domain()))
                .collect(),
            self.clearing_prices
                .into_iter()
                .map(|(token, price)| {
                    domain::auction::Price::try_new(price.into()).map(|price| (token.into(), price))
                })
                .collect::<Result<_, _>>()?,
        ))
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
            raw_body: Bytes::from(json),
            compressed_body: None,
            use_compressed: false,
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
            raw_body: Bytes::from(json.to_vec()),
            compressed_body: Some(Bytes::from(compressed)),
            use_compressed: true,
        }
    }

    #[test]
    fn compressed_request_round_trips() {
        let json = make_test_json();

        let request = compressed_request(&json);
        assert!(request.use_compressed);
        let compressed = request.compressed_body.as_ref().unwrap();
        assert!(
            compressed.len() < json.len(),
            "compressed body {} should be smaller than original {}",
            compressed.len(),
            json.len(),
        );

        let mut decompressed = Vec::new();
        brotli::BrotliDecompress(&mut compressed.as_ref(), &mut decompressed).unwrap();
        assert_eq!(decompressed, json);
    }

    #[test]
    fn uncompressed_request_preserves_json() {
        let json = make_test_json();
        let request = uncompressed_request(json.clone());

        assert!(!request.use_compressed);
        assert_eq!(request.raw_body.as_ref(), json.as_slice());
    }
}
