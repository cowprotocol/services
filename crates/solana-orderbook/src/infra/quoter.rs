//! Client for the drivers' quote routes.

use {
    futures::future::join_all,
    reqwest::Client,
    serde::{Deserialize, Serialize},
    serde_with::{DisplayFromStr, base64::Base64, serde_as},
    solana_sdk::{pubkey::Pubkey, transaction::VersionedTransaction},
    std::time::{Duration, Instant},
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
#[derive(Debug)]
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

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no driver returned a quote")]
    NoQuotes,
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
    /// The bincode-serialized settlement, absent from a driver that answers
    /// none.
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
    pub async fn quotes(&self, order: &Order) -> Result<Vec<Quote>, Error> {
        let quotes: Vec<Quote> = join_all(
            self.endpoints
                .iter()
                .map(|endpoint| self.quote_one(endpoint, order)),
        )
        .await
        .into_iter()
        .flatten()
        .collect();
        if quotes.is_empty() {
            return Err(Error::NoQuotes);
        }
        Ok(quotes)
    }

    /// Quote `order` on one driver. Failures are logged, counted and
    /// swallowed: a `QuotingFailed` reply means no route, which is a routine
    /// outcome, while anything else is that driver misbehaving.
    async fn quote_one(&self, endpoint: &Url, order: &Order) -> Option<Quote> {
        let driver = driver_name(endpoint);
        let start = Instant::now();
        let quoted = self.ask(endpoint, order).await;
        let metrics = metrics();
        metrics
            .price_estimation_times
            .with_label_values(&[driver.as_str()])
            .observe(start.elapsed().as_secs_f64());
        let result = match &quoted {
            Ok(_) => "success",
            Err(result) => result,
        };
        metrics
            .price_estimates
            .with_label_values(&[driver.as_str(), result])
            .inc();
        let quoted = quoted.ok()?;
        let transaction = quoted.transaction.as_deref().and_then(|bytes| {
            bincode::deserialize(bytes)
                .inspect_err(|err| {
                    tracing::warn!(driver, ?err, "driver quote transaction malformed")
                })
                .ok()
        });
        Some(Quote {
            sell_amount: quoted.sell_amount,
            buy_amount: quoted.buy_amount,
            solver: quoted.solver,
            network_fee: quoted.network_fee.unwrap_or(0),
            transaction,
            verified: false,
            driver,
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
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            let result = failure(status, &body);
            if result == "no_liquidity" {
                tracing::debug!(%endpoint, body, "driver found no quote");
            } else {
                tracing::warn!(%endpoint, %status, body, "driver quote failed");
            }
            return Err(result);
        }
        response
            .json()
            .await
            .inspect_err(|err| tracing::warn!(%endpoint, ?err, "driver quote response malformed"))
            .map_err(|_| "estimator_internal_error")
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

/// The `price_estimates` result of a driver's error reply: `QuotingFailed`
/// means no route, 429 rate limiting, anything else a driver fault.
fn failure(status: reqwest::StatusCode, body: &str) -> &'static str {
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return "rate_limited";
    }
    match serde_json::from_str::<ErrorBody>(body) {
        Ok(error) if error.kind == "QuotingFailed" => "no_liquidity",
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
    use {super::*, reqwest::StatusCode};

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
}
