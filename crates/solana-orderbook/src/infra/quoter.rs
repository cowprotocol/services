//! Client for the drivers' quote routes.

use {
    futures::{StreamExt, stream::FuturesUnordered},
    reqwest::Client,
    serde::{Deserialize, Serialize},
    serde_with::{DisplayFromStr, base64::Base64, serde_as},
    solana_sdk::{pubkey::Pubkey, transaction::VersionedTransaction},
    std::{
        num::NonZero,
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    },
    url::Url,
};

/// Time reserved out of the budget for the driver to convert a solution and
/// for the response to travel back. The driver spends everything up to the
/// deadline it is given, so without a reserve a near-deadline answer races
/// this client's own timeout.
const RESPONSE_RESERVE: Duration = Duration::from_millis(500);

/// Failures in a row after which quotes skip a driver.
const FAILURES_TO_SKIP: u32 = 3;

/// How long quotes skip a failing driver before one quote probes it again.
const PROBE_INTERVAL: Duration = Duration::from_secs(30);

/// Asks every configured driver to quote an order.
#[derive(Clone, Debug)]
pub struct Quoter {
    client: Client,
    drivers: Arc<[Driver]>,
    timeout: Duration,
    probe_interval: Duration,
}

#[derive(Debug)]
struct Driver {
    endpoint: Url,
    name: String,
    health: Mutex<Health>,
}

#[derive(Debug, Default)]
struct Health {
    /// Failures in a row.
    failures: u32,
    skipped_until: Option<Instant>,
}

enum Action {
    Ask,
    Skip,
    /// Skip the driver, and probe it in the background.
    Probe,
}

impl Driver {
    fn up(&self) -> bool {
        self.health.lock().unwrap().skipped_until.is_none()
    }

    fn next_action(&self, probe_interval: Duration) -> Action {
        let mut health = self.health.lock().unwrap();
        let now = Instant::now();
        match health.skipped_until {
            None => Action::Ask,
            Some(until) if now < until => Action::Skip,
            Some(_) => {
                // Concurrent quotes keep skipping while this one probes.
                health.skipped_until = Some(now + probe_interval);
                Action::Probe
            }
        }
    }

    fn record(&self, answered: bool, probe_interval: Duration) {
        let mut health = self.health.lock().unwrap();
        if answered {
            health.failures = 0;
            if health.skipped_until.take().is_some() {
                tracing::info!(endpoint = %self.endpoint, "driver answers again, quoting with it");
            }
            return;
        }
        health.failures = health.failures.saturating_add(1);
        if health.failures >= FAILURES_TO_SKIP && health.skipped_until.is_none() {
            health.skipped_until = Some(Instant::now() + probe_interval);
            tracing::info!(
                endpoint = %self.endpoint,
                failures = health.failures,
                ?probe_interval,
                "driver keeps failing, skipping it"
            );
        }
    }
}

/// The order to quote.
#[derive(Clone, Debug)]
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

/// The `price_estimates` result of a driver skipped as failing.
const SKIPPED: &str = "skipped";

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
            drivers: endpoints
                .into_iter()
                .map(|endpoint| Driver {
                    name: driver_name(&endpoint),
                    endpoint,
                    health: Mutex::default(),
                })
                .collect(),
            timeout,
            probe_interval: PROBE_INTERVAL,
        }
    }

    /// Quote `order` on every driver concurrently and return every answer.
    /// With `early_return`, return as soon as that many drivers answered and
    /// cancel the requests still running. A driver that keeps failing is
    /// skipped until a background probe gets an answer from it, unless every
    /// driver is failing: then all are asked.
    pub async fn quotes(
        &self,
        order: &Order,
        early_return: Option<NonZero<usize>>,
    ) -> Result<Vec<Quote>, Error> {
        let mut pending = FuturesUnordered::new();
        let mut failures = Vec::new();
        let any_up = self.drivers.iter().any(Driver::up);
        for (index, driver) in self.drivers.iter().enumerate() {
            match driver.next_action(self.probe_interval) {
                Action::Skip if any_up => {}
                Action::Probe if any_up => {
                    let (quoter, order) = (self.clone(), order.clone());
                    tokio::spawn(
                        async move { quoter.quote_one(&quoter.drivers[index], &order).await },
                    );
                }
                _ => {
                    pending.push(self.quote_one(driver, order));
                    continue;
                }
            }
            metrics()
                .price_estimates
                .with_label_values(&[driver.name.as_str(), SKIPPED])
                .inc();
            failures.push(SKIPPED);
        }
        let mut quotes = Vec::new();
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
    async fn quote_one(&self, driver: &Driver, order: &Order) -> Result<Quote, &'static str> {
        let start = Instant::now();
        let quoted = self.ask(driver, order).await;
        let elapsed = start.elapsed();
        let driver = &driver.name;
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
            driver: driver.clone(),
            elapsed,
        })
    }

    /// Ask one driver to quote `order`. An error is the `price_estimates`
    /// result the failure counts as.
    async fn ask(&self, driver: &Driver, order: &Order) -> Result<ResponseBody, &'static str> {
        let endpoint = &driver.endpoint;
        let url = endpoint.join("quote").expect("valid /quote path");
        let body = RequestBody {
            sell_token: order.sell_token,
            buy_token: order.buy_token,
            amount: order.amount,
            kind: order.kind,
            deadline: chrono::Utc::now() + self.timeout.saturating_sub(RESPONSE_RESERVE),
            owner: order.owner,
        };
        let (status, body) = async {
            let response = self
                .client
                .post(url)
                .json(&body)
                .timeout(self.timeout)
                .send()
                .await?;
            let status = response.status();
            Ok::<_, reqwest::Error>((status, response.text().await?))
        }
        .await
        .inspect_err(|err| {
            tracing::warn!(%endpoint, ?err, "driver quote request failed");
            driver.record(false, self.probe_interval);
        })
        .map_err(|_| "estimator_internal_error")?;
        driver.record(
            !(status.is_server_error() || status == reqwest::StatusCode::TOO_MANY_REQUESTS),
            self.probe_interval,
        );
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
        reqwest::StatusCode,
        std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst},
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

    async fn serve(app: axum::Router) -> Url {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{addr}/").parse().unwrap()
    }

    /// A driver stand-in answering every `/quote` with `status` and `body`.
    async fn driver(status: StatusCode, body: &'static str) -> Url {
        serve(axum::Router::new().route(
            "/quote",
            axum::routing::post(move || async move { (status, body) }),
        ))
        .await
    }

    const QUOTE: &str =
        r#"{"sellAmount":"1000","buyAmount":"1","solver":"11111111111111111111111111111111"}"#;

    /// A driver stand-in that quotes while the returned flag is set, otherwise
    /// answers `failure` or hangs on `None`, and counts its requests.
    async fn flaky_driver(failure: Option<StatusCode>) -> (Url, Arc<AtomicBool>, Arc<AtomicUsize>) {
        let (up, hits) = (
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicUsize::new(0)),
        );
        let (is_up, hit) = (up.clone(), hits.clone());
        let app = axum::Router::new().route(
            "/quote",
            axum::routing::post(move || {
                hit.fetch_add(1, SeqCst);
                let up = is_up.load(SeqCst);
                async move {
                    match (up, failure) {
                        (true, _) => (StatusCode::OK, QUOTE),
                        (false, Some(status)) => (status, ""),
                        (false, None) => std::future::pending().await,
                    }
                }
            }),
        );
        (serve(app).await, up, hits)
    }

    fn order() -> Order {
        Order {
            sell_token: Pubkey::new_unique(),
            buy_token: Pubkey::new_unique(),
            amount: 1_000,
            kind: Kind::Sell,
            owner: None,
        }
    }

    async fn quotes(drivers: Vec<Url>) -> Result<Vec<Quote>, Error> {
        Quoter::new(drivers, Duration::from_secs(1))
            .quotes(&order(), None)
            .await
    }

    #[tokio::test]
    async fn skips_a_driver_that_keeps_failing() {
        let failures = [
            None,
            Some(StatusCode::SERVICE_UNAVAILABLE),
            Some(StatusCode::TOO_MANY_REQUESTS),
        ];
        for failure in failures {
            let (url, _, hits) = flaky_driver(failure).await;
            let healthy = driver(StatusCode::OK, QUOTE).await;
            let quoter = Quoter::new(vec![url, healthy], Duration::from_millis(200));
            for _ in 0..FAILURES_TO_SKIP + 2 {
                quoter.quotes(&order(), None).await.unwrap();
            }
            assert_eq!(hits.load(SeqCst), FAILURES_TO_SKIP as usize, "{failure:?}");
        }
    }

    #[tokio::test]
    async fn asks_a_recovered_driver_again() {
        let (url, up, _) = flaky_driver(None).await;
        let healthy = driver(StatusCode::OK, QUOTE).await;
        let mut quoter = Quoter::new(vec![url, healthy], Duration::from_millis(200));
        quoter.probe_interval = Duration::ZERO;
        for _ in 0..FAILURES_TO_SKIP {
            quoter.quotes(&order(), None).await.unwrap();
        }
        up.store(true, SeqCst);
        // The quote that probes the driver still skips it.
        assert_eq!(quoter.quotes(&order(), None).await.unwrap().len(), 1);
        tokio::time::timeout(Duration::from_secs(5), async {
            while quoter.quotes(&order(), None).await.unwrap().len() < 2 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("the recovered driver is asked again");
    }

    #[tokio::test]
    async fn never_skips_every_driver() {
        let (url, _, hits) = flaky_driver(Some(StatusCode::SERVICE_UNAVAILABLE)).await;
        let quoter = Quoter::new(vec![url], Duration::from_secs(1));
        for _ in 0..FAILURES_TO_SKIP + 2 {
            assert!(quoter.quotes(&order(), None).await.is_err());
        }
        assert_eq!(hits.load(SeqCst), FAILURES_TO_SKIP as usize + 2);
    }

    /// A driver that answers, even with an error, is not down.
    #[tokio::test]
    async fn answers_do_not_count_as_failures() {
        let no_route = r#"{"kind":"QuotingFailed","description":"No valid quote found"}"#;
        for body in [no_route, ""] {
            let quoter = Quoter::new(
                vec![driver(StatusCode::BAD_REQUEST, body).await],
                Duration::from_secs(1),
            );
            for _ in 0..FAILURES_TO_SKIP + 2 {
                assert!(quoter.quotes(&order(), None).await.is_err());
            }
            assert!(quoter.drivers[0].up(), "{body}");
        }
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
