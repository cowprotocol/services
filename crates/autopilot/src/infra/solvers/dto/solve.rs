use {
    crate::{
        boundary,
        domain::{self, eth},
        infra::{
            persistence::dto::{self, order::Order},
            solvers::InjectIntoHttpRequest,
        },
    },
    alloy::primitives::{Address, U256},
    bytes::Bytes,
    chrono::{DateTime, Utc},
    futures::Stream,
    itertools::Itertools,
    number::serialization::HexOrDecimalU256,
    reqwest::RequestBuilder,
    serde::{Deserialize, Serialize},
    serde_with::{DisplayFromStr, serde_as},
    std::{
        borrow::Cow,
        collections::{HashMap, HashSet},
        task::Poll,
        time::{Duration, Instant},
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
}

impl Request {
    pub async fn new(
        auction: &domain::Auction,
        trusted_tokens: &HashSet<Address>,
        time_limit: Duration,
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

        let body = tokio::task::spawn_blocking(move || {
            let serialized = serde_json::to_vec(&helper).expect("type should be JSON serializable");
            Bytes::from(serialized)
        })
        .await
        .expect("inner task should not panic as serialization should work for the given type");

        Self { body, auction_id }
    }
}

impl InjectIntoHttpRequest for Request {
    fn inject(&self, request: RequestBuilder) -> RequestBuilder {
        request
            .body(reqwest::Body::wrap_stream(ByteStream::new(self.body.clone())))
            // announce which auction this request is for in the
            // headers to help the driver detect duplicated
            // `/solve` requests before streaming the body
            .header("X-Auction-Id", self.auction_id)
            // manually set the content type header for JSON since
            // we can't use `request.json(self)`
            .header(
                hyper::header::CONTENT_TYPE,
                hyper::header::HeaderValue::from_static("application/json")
            )
    }

    fn body_to_string(&self) -> Cow<'_, str> {
        let string = str::from_utf8(self.body.as_ref()).unwrap();
        Cow::Borrowed(string)
    }
}

/// Thin wrapper around a payload that already exists fully serialized
/// in-memory. The purpose of converting that into a stream is to allow
/// measuring the data transfer of the request body (for debugging and
/// optimization purposes).
///
/// Note that this measurement is only an approximation of the truth.
/// The reason is that this only measures how long it takes `hyper`
/// to load the last byte from the body into the buffer of the network
/// stack. However, given how big `/solve` requests are in practice
/// `hyper` should have to flush the buffer a couple of times so the
/// measured time should be reasonably accurate.
struct ByteStream {
    data: Bytes,
    created_at: Instant,
    first_polled_at: Option<Instant>,
    span: tracing::Span,
}

impl ByteStream {
    fn new(data: Bytes) -> Self {
        Self {
            data,
            created_at: Instant::now(),
            first_polled_at: None,
            span: tracing::Span::current(),
        }
    }
}

impl Stream for ByteStream {
    type Item = Result<Bytes, std::io::Error>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let this = self.as_mut().get_mut();
        let _span = this.span.enter();

        if this.first_polled_at.is_none() {
            this.first_polled_at = Some(Instant::now());
        }

        const CHUNK_SIZE: usize = 10 * 1024; // 10 KB
        if this.data.is_empty() {
            let first_poll = this.first_polled_at.expect("initialized at first poll");
            tracing::debug!(
                until_first_poll = ?first_poll.duration_since(this.created_at),
                transmission = ?first_poll.elapsed(),
                "finished streaming http request body"
            );
            Poll::Ready(None)
        } else {
            let end_index = std::cmp::min(CHUNK_SIZE, this.data.len());
            // splits off bytes to transmit and only leaves the remaining bytes in
            // `self.data`
            let chunk = this.data.split_to(end_index);
            Poll::Ready(Some(Ok(chunk)))
        }
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
