//! Client for the drivers' quote routes.

use {
    futures::{StreamExt, stream::FuturesUnordered},
    reqwest::Client,
    serde::{Deserialize, Serialize},
    serde_with::{DisplayFromStr, base64::Base64, serde_as},
    solana_sdk::{pubkey::Pubkey, transaction::VersionedTransaction},
    std::{
        num::NonZero,
        time::{Duration, Instant},
    },
    url::Url,
};

/// Time reserved out of the budget for the driver to convert a solution and
/// for the response to travel back. The driver spends everything up to the
/// deadline it is given, so without a reserve a near-deadline answer races
/// this client's own timeout.
const RESPONSE_RESERVE: Duration = Duration::from_millis(500);

/// Asks every configured driver to quote an order.
#[derive(Clone, Debug)]
pub struct Quoter {
    client: Client,
    endpoints: Vec<Url>,
    timeout: Duration,
}

/// The order to quote.
#[derive(Debug)]
pub struct Order {
    pub sell_token: Pubkey,
    pub buy_token: Pubkey,
    pub amount: u64,
    pub kind: Kind,
    /// The wallet the quoted order belongs to, for the driver to build its
    /// settlement against. `None` asks for no settlement.
    pub owner: Option<Pubkey>,
}

/// Which amount the order fixes.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Sell,
    Buy,
}

/// What a driver quoted.
#[derive(Clone, Debug)]
pub struct Quote {
    pub sell_amount: u64,
    pub buy_amount: u64,
    /// The solver that produced the quote.
    pub solver: Pubkey,
    /// The network fee of settling the quoted route, in lamports. Zero when
    /// the driver reports none.
    pub network_fee: u64,
    /// The settlement of the quoted route against the order's wallet,
    /// unsigned. `None` when the driver answered none or a malformed one.
    pub transaction: Option<VersionedTransaction>,
    /// Whether the verification confirmed the amounts.
    pub verified: bool,
    /// The driver that answered, as labelled in metrics.
    pub driver: String,
    /// How long the driver took to answer.
    pub elapsed: Duration,
}

impl Quote {
    /// Counts this quote as the winner of its quote competition.
    pub fn count_win(&self, kind: Kind) {
        let kind = match kind {
            Kind::Sell => "sell",
            Kind::Buy => "buy",
        };
        metrics()
            .competition_price_estimator_queries_won
            .with_label_values(&[self.driver.as_str(), kind])
            .inc();
    }
}

/// The `price_estimates` result of a driver without a route.
const NO_LIQUIDITY: &str = "no_liquidity";

/// Why no driver quoted.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("no driver has a route")]
    NoLiquidity,
    #[error("a driver failed to quote")]
    DriverFailed,
}

impl Error {
    /// The error of a competition whose drivers failed with these
    /// `price_estimates` results: no liquidity only when every driver has no
    /// route, as the EVM quote competition ranks any other estimator error
    /// above a missing route.
    fn of(failures: &[&'static str]) -> Self {
        if failures.iter().all(|failure| *failure == NO_LIQUIDITY) {
            Self::NoLiquidity
        } else {
            Self::DriverFailed
        }
    }
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RequestBody {
    #[serde_as(as = "DisplayFromStr")]
    sell_token: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    buy_token: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    amount: u64,
    kind: Kind,
    deadline: chrono::DateTime<chrono::Utc>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[serde(skip_serializing_if = "Option::is_none")]
    owner: Option<Pubkey>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResponseBody {
    #[serde_as(as = "DisplayFromStr")]
    sell_amount: u64,
    #[serde_as(as = "DisplayFromStr")]
    buy_amount: u64,
    #[serde_as(as = "DisplayFromStr")]
    solver: Pubkey,
    /// Absent from a driver that does not report it.
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[serde(default)]
    network_fee: Option<u64>,
    /// The settlement, bincode-serialized or in the wire format (the two
    /// differ for a v1 message). Absent from a driver that answers none.
    #[serde_as(as = "Option<Base64>")]
    #[serde(default)]
    transaction: Option<Vec<u8>>,
}

#[derive(Deserialize)]
struct ErrorBody {
    kind: String,
}

impl Quoter {
    pub fn new(endpoints: Vec<Url>, timeout: Duration) -> Self {
        Self {
            client: Client::new(),
            endpoints,
            timeout,
        }
    }

    /// Quote `order` on every driver concurrently and return every answer.
    /// With `early_return`, return as soon as that many drivers answered and
    /// cancel the requests still running.
    pub async fn quotes(
        &self,
        order: &Order,
        early_return: Option<NonZero<usize>>,
    ) -> Result<Vec<Quote>, Error> {
        let mut pending: FuturesUnordered<_> = self
            .endpoints
            .iter()
            .map(|endpoint| self.quote_one(endpoint, order))
            .collect();
        let mut quotes = Vec::new();
        let mut failures = Vec::new();
        while let Some(quoted) = pending.next().await {
            match quoted {
                Ok(quote) => quotes.push(quote),
                Err(failure) => failures.push(failure),
            }
            if early_return.is_some_and(|required| quotes.len() >= required.get()) {
                break;
            }
        }
        if quotes.is_empty() {
            return Err(Error::of(&failures));
        }
        Ok(quotes)
    }

    /// Quote `order` on one driver. A failure is logged, counted and returned
    /// as its `price_estimates` result: a `QuotingFailed` reply means no
    /// route, which is a routine outcome, while anything else is that driver
    /// misbehaving.
    async fn quote_one(&self, endpoint: &Url, order: &Order) -> Result<Quote, &'static str> {
        let driver = driver_name(endpoint);
        let start = Instant::now();
        let quoted = self.ask(endpoint, order).await;
        let elapsed = start.elapsed();
        let metrics = metrics();
        metrics
            .price_estimation_times
            .with_label_values(&[driver.as_str()])
            .observe(elapsed.as_secs_f64());
        let result = match &quoted {
            Ok(_) => "success",
            Err(result) => result,
        };
        metrics
            .price_estimates
            .with_label_values(&[driver.as_str(), result])
            .inc();
        // Every driver that finishes, answered or failed: a driver that fails
        // never reaches the quote competition, and a request where they all
        // fail logs no competition at all.
        tracing::debug!(
            driver = %driver,
            result,
            elapsed_ms = elapsed.as_millis() as u64,
            ?order,
            "new price estimate"
        );
        let quoted = quoted?;
        let transaction = quoted.transaction.as_deref().and_then(|bytes| {
            bincode::deserialize(bytes)
                .or_else(|_| wincode::deserialize(bytes))
                .inspect_err(|err| {
                    tracing::warn!(driver, ?err, "driver quote transaction malformed")
                })
                .ok()
        });
        Ok(Quote {
            sell_amount: quoted.sell_amount,
            buy_amount: quoted.buy_amount,
            solver: quoted.solver,
            network_fee: quoted.network_fee.unwrap_or(0),
            transaction,
            verified: false,
            driver,
            elapsed,
        })
    }

    /// Ask one driver to quote `order`. An error is the `price_estimates`
    /// result the failure counts as.
    async fn ask(&self, endpoint: &Url, order: &Order) -> Result<ResponseBody, &'static str> {
        let url = endpoint.join("quote").expect("valid /quote path");
        let body = RequestBody {
            sell_token: order.sell_token,
            buy_token: order.buy_token,
            amount: order.amount,
            kind: order.kind,
            deadline: chrono::Utc::now() + self.timeout.saturating_sub(RESPONSE_RESERVE),
            owner: order.owner,
        };
        let response = self
            .client
            .post(url)
            .json(&body)
            .timeout(self.timeout)
            .send()
            .await
            .inspect_err(|err| tracing::warn!(%endpoint, ?err, "driver quote request failed"))
            .map_err(|_| "estimator_internal_error")?;
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        // A driver may answer its error under a success status, so the body
        // tells a quote from a failure, as on the EVM orderbook.
        if status.is_success()
            && let Ok(quote) = serde_json::from_str(&body)
        {
            return Ok(quote);
        }
        let result = failure(status, &body);
        if result == NO_LIQUIDITY {
            tracing::debug!(%endpoint, body, "driver found no quote");
        } else {
            tracing::warn!(%endpoint, %status, body, "driver quote failed");
        }
        Err(result)
    }
}

/// A driver's metric label: the last segment of its URL path, which names the
/// solver behind it, or the host for a URL without a path.
fn driver_name(endpoint: &Url) -> String {
    endpoint
        .path_segments()
        .and_then(|mut segments| segments.rfind(|segment| !segment.is_empty()))
        .or(endpoint.host_str())
        .unwrap_or_default()
        .to_owned()
}

/// The `price_estimates` result of a driver's answer that is no quote:
/// `QuotingFailed` means no route, 429 rate limiting, anything else a driver
/// fault.
fn failure(status: reqwest::StatusCode, body: &str) -> &'static str {
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return "rate_limited";
    }
    match serde_json::from_str::<ErrorBody>(body) {
        Ok(error) if error.kind == "QuotingFailed" => NO_LIQUIDITY,
        _ => "estimator_internal_error",
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
struct Metrics {
    /// Driver quotes by driver and result.
    #[metric(labels("estimator_type", "result"))]
    price_estimates: prometheus::IntCounterVec,
    /// Duration of each driver quote request.
    #[metric(labels("estimator_type"))]
    price_estimation_times: prometheus::HistogramVec,
    /// Quotes won by driver and order kind.
    #[metric(labels("estimator_type", "order_kind"))]
    competition_price_estimator_queries_won: prometheus::IntCounterVec,
}

fn metrics() -> &'static Metrics {
    Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        base64::{Engine, prelude::BASE64_STANDARD},
        reqwest::StatusCode,
        solana_sdk::{
            hash::Hash,
            instruction::{AccountMeta, Instruction},
            message::{VersionedMessage, v1},
            signature::Signature,
        },
    };

    #[test]
    fn labels_drivers_by_their_last_path_segment() {
        let name = |url: &str| driver_name(&url.parse().unwrap());
        assert_eq!(
            name("http://driver.svc/jupiter-quote-native"),
            "jupiter-quote-native"
        );
        assert_eq!(
            name("http://quoter.example:15090/SolverName/"),
            "SolverName"
        );
        assert_eq!(name("http://quoter.example:15090"), "quoter.example");
    }

    #[test]
    fn counts_only_quoting_failed_as_no_liquidity() {
        let error = |kind: &str| format!(r#"{{"kind":"{kind}","description":"test"}}"#);
        assert_eq!(
            failure(StatusCode::BAD_REQUEST, &error("QuotingFailed")),
            "no_liquidity"
        );
        assert_eq!(
            failure(StatusCode::BAD_REQUEST, &error("SolverFailed")),
            "estimator_internal_error"
        );
        assert_eq!(failure(StatusCode::TOO_MANY_REQUESTS, ""), "rate_limited");
        assert_eq!(
            failure(StatusCode::INTERNAL_SERVER_ERROR, "upstream down"),
            "estimator_internal_error"
        );
    }

    /// A driver stand-in answering every `/quote` with `status` and `body`.
    async fn driver(status: StatusCode, body: &'static str) -> Url {
        let app = axum::Router::new().route(
            "/quote",
            axum::routing::post(move || async move { (status, body) }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{addr}/").parse().unwrap()
    }

    async fn quotes(drivers: Vec<Url>) -> Result<Vec<Quote>, Error> {
        let order = Order {
            sell_token: Pubkey::new_unique(),
            buy_token: Pubkey::new_unique(),
            amount: 1_000,
            kind: Kind::Sell,
            owner: None,
        };
        Quoter::new(drivers, Duration::from_secs(1))
            .quotes(&order, None)
            .await
    }

    /// A driver's `QuotingFailed` is no route under any status, as the EVM
    /// orderbook reads a driver's error from the body alone. A success
    /// status with any other body is a driver fault.
    #[tokio::test]
    async fn reads_a_drivers_missing_route_whatever_the_status() {
        let no_route = r#"{"kind":"QuotingFailed","description":"No valid quote found"}"#;
        for status in [StatusCode::BAD_REQUEST, StatusCode::OK] {
            let failed = quotes(vec![driver(status, no_route).await]).await;
            assert_eq!(failed.unwrap_err(), Error::NoLiquidity, "{status}");
        }
        let malformed = quotes(vec![driver(StatusCode::OK, r#"{"amount":"1"}"#).await]).await;
        assert_eq!(malformed.unwrap_err(), Error::DriverFailed);
    }

    /// Bincode and the wire format encode a v1 message differently, and
    /// drivers send either.
    #[tokio::test]
    async fn reads_a_drivers_settlement_in_bincode_or_wire_format() {
        let payer = Pubkey::new_unique();
        let instruction = Instruction::new_with_bytes(
            Pubkey::new_unique(),
            &[1, 2, 3],
            vec![AccountMeta::new(payer, true)],
        );
        let config = v1::TransactionConfig {
            priority_fee: Some(5_000),
            compute_unit_limit: Some(1_400_000),
            loaded_accounts_data_size_limit: Some(64 * 1024 * 1024),
            heap_size: None,
        };
        let message = v1::Message::try_compile_with_config(
            &payer,
            &[instruction],
            Hash::new_unique(),
            config,
        )
        .unwrap();
        let settlement = VersionedTransaction {
            signatures: vec![Signature::default()],
            message: VersionedMessage::V1(message),
        };
        for (encoding, bytes) in [
            ("bincode", bincode::serialize(&settlement).unwrap()),
            ("wire", wincode::serialize(&settlement).unwrap()),
        ] {
            let body = serde_json::json!({
                "sellAmount": "1000",
                "buyAmount": "900",
                "solver": payer.to_string(),
                "transaction": BASE64_STANDARD.encode(bytes),
            });
            let quoted = quotes(vec![driver(StatusCode::OK, body.to_string().leak()).await]).await;
            assert_eq!(
                quoted.unwrap()[0].transaction,
                Some(settlement.clone()),
                "{encoding}"
            );
        }
    }

    #[test]
    fn no_quote_is_no_liquidity_only_when_every_driver_lacks_a_route() {
        assert_eq!(Error::of(&[NO_LIQUIDITY, NO_LIQUIDITY]), Error::NoLiquidity);
        assert_eq!(
            Error::of(&[NO_LIQUIDITY, "rate_limited"]),
            Error::DriverFailed
        );
        assert_eq!(
            Error::of(&["estimator_internal_error"]),
            Error::DriverFailed
        );
        // No driver to ask has no route either.
        assert_eq!(Error::of(&[]), Error::NoLiquidity);
    }
}
