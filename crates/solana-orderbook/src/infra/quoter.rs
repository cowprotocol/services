//! Client for the drivers' quote routes.

use {
    futures::future::join_all,
    reqwest::Client,
    serde::{Deserialize, Serialize},
    serde_with::{DisplayFromStr, serde_as},
    solana_sdk::pubkey::Pubkey,
    std::time::{Duration, Instant},
    url::Url,
};

/// Time reserved out of the budget for the driver to convert a solution and
/// for the response to travel back. The driver spends everything up to the
/// deadline it is given, so without a reserve a near-deadline answer races
/// this client's own timeout.
const RESPONSE_RESERVE: Duration = Duration::from_millis(500);

/// Asks every configured driver to quote an order and keeps the best answer.
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
}

/// Which amount the order fixes.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Sell,
    Buy,
}

/// What the competition was asked for. Mirrors the EVM `QuoteRequest` so both
/// chains describe a quote request the same way.
#[serde_as]
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteRequest {
    #[serde_as(as = "DisplayFromStr")]
    pub sell_token: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub buy_token: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub amount: u64,
    pub kind: Kind,
}

/// What one driver quoted. Mirrors the EVM `QuoteResponse`; `driver` is the
/// Solana analogue of the EVM's `estimator`, naming who produced the quote.
#[serde_as]
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteResponse {
    pub driver: String,
    #[serde_as(as = "DisplayFromStr")]
    pub solver: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub sell_amount: u64,
    #[serde_as(as = "DisplayFromStr")]
    pub buy_amount: u64,
    pub elapsed_ms: u64,
}

/// Every driver that answered, best first. Mirrors the EVM `QuoteCompetition`:
/// the losing quotes are kept rather than discarded, so the competition that
/// produced a quote stays visible after the fact.
///
/// Guaranteed non-empty by construction.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Competition {
    pub request: QuoteRequest,
    quotes: Vec<QuoteResponse>,
}

/// Order the quotes best first: the largest buy for a sell order, the
/// smallest sell for a buy order.
fn rank(quotes: &mut [QuoteResponse], kind: Kind) {
    match kind {
        Kind::Sell => quotes.sort_by_key(|quote| std::cmp::Reverse(quote.buy_amount)),
        Kind::Buy => quotes.sort_by_key(|quote| quote.sell_amount),
    }
}

impl Competition {
    /// The winning quote: the largest buy for a sell order, the smallest sell
    /// for a buy order.
    pub fn winner(&self) -> &QuoteResponse {
        self.quotes.first().expect("non-empty by construction")
    }

    /// All quotes, best first. Guaranteed non-empty.
    pub fn quotes(&self) -> &[QuoteResponse] {
        &self.quotes
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
}

impl Quoter {
    pub fn new(endpoints: Vec<Url>, timeout: Duration) -> Self {
        Self {
            client: Client::new(),
            endpoints,
            timeout,
        }
    }

    /// Quote `order` on every driver concurrently and return the whole
    /// competition, ranked best first: the largest buy for a sell order, the
    /// smallest sell for a buy order.
    ///
    /// The competition is logged as one JSON object under the same message
    /// the EVM orderbook uses, so a single parser serves both chains.
    pub async fn quote(&self, order: &Order) -> Result<Competition, Error> {
        let quotes = join_all(
            self.endpoints
                .iter()
                .map(|endpoint| self.quote_one(endpoint, order)),
        )
        .await;
        // Rank rather than pick, so the drivers that lost stay on the record.
        let mut quotes: Vec<_> = quotes.into_iter().flatten().collect();
        rank(&mut quotes, order.kind);
        if quotes.is_empty() {
            return Err(Error::NoQuotes);
        }
        let competition = Competition {
            request: QuoteRequest {
                sell_token: order.sell_token,
                buy_token: order.buy_token,
                amount: order.amount,
                kind: order.kind,
            },
            quotes,
        };
        match serde_json::to_string(&competition) {
            Ok(json) => tracing::debug!(competition = %json, "computed quote"),
            Err(err) => tracing::warn!(?err, "failed to serialize quote competition"),
        }
        Ok(competition)
    }

    /// Quote `order` on one driver. Failures are logged and swallowed: a
    /// driver rejecting the quote found no route, which is a routine outcome,
    /// while anything else is that driver misbehaving.
    async fn quote_one(&self, endpoint: &Url, order: &Order) -> Option<QuoteResponse> {
        let url = endpoint.join("quote").expect("valid /quote path");
        let body = RequestBody {
            sell_token: order.sell_token,
            buy_token: order.buy_token,
            amount: order.amount,
            kind: order.kind,
            deadline: chrono::Utc::now() + self.timeout.saturating_sub(RESPONSE_RESERVE),
        };
        let start = Instant::now();
        let response = self
            .client
            .post(url)
            .json(&body)
            .timeout(self.timeout)
            .send()
            .await
            .inspect_err(|err| tracing::warn!(%endpoint, ?err, "driver quote request failed"))
            .ok()?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            if status == reqwest::StatusCode::BAD_REQUEST {
                tracing::debug!(%endpoint, body, "driver found no quote");
            } else {
                tracing::warn!(%endpoint, %status, body, "driver quote failed");
            }
            return None;
        }
        let quoted: ResponseBody = response
            .json()
            .await
            .inspect_err(|err| tracing::warn!(%endpoint, ?err, "driver quote response malformed"))
            .ok()?;
        Some(QuoteResponse {
            driver: endpoint.to_string(),
            solver: quoted.solver,
            sell_amount: quoted.sell_amount,
            buy_amount: quoted.buy_amount,
            elapsed_ms: start.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(driver: &str, sell_amount: u64, buy_amount: u64) -> QuoteResponse {
        QuoteResponse {
            driver: driver.to_owned(),
            solver: Pubkey::new_from_array([7; 32]),
            sell_amount,
            buy_amount,
            elapsed_ms: 1,
        }
    }

    fn competition(kind: Kind, mut quotes: Vec<QuoteResponse>) -> Competition {
        rank(&mut quotes, kind);
        Competition {
            request: QuoteRequest {
                sell_token: Pubkey::new_from_array([1; 32]),
                buy_token: Pubkey::new_from_array([2; 32]),
                amount: 100,
                kind,
            },
            quotes,
        }
    }

    /// A sell order wants the most buy; a buy order the least sell. Either
    /// way every driver that answered stays on the record, so the losers can
    /// be seen after the fact.
    #[test]
    fn the_competition_ranks_best_first_and_keeps_the_losers() {
        let sell = competition(
            Kind::Sell,
            vec![
                response("mid", 100, 200),
                response("best", 100, 300),
                response("worst", 100, 100),
            ],
        );
        assert_eq!(sell.winner().driver, "best");
        assert_eq!(
            sell.quotes().iter().map(|q| &*q.driver).collect::<Vec<_>>(),
            ["best", "mid", "worst"]
        );

        let buy = competition(
            Kind::Buy,
            vec![
                response("mid", 200, 100),
                response("best", 100, 100),
                response("worst", 300, 100),
            ],
        );
        assert_eq!(buy.winner().driver, "best");
        assert_eq!(
            buy.quotes().iter().map(|q| &*q.driver).collect::<Vec<_>>(),
            ["best", "mid", "worst"]
        );
    }

    /// The competition is logged as JSON so one parser serves both chains.
    /// Pubkeys are base58 and amounts are strings, matching the API's own
    /// encoding rather than Rust's `Debug`.
    #[test]
    fn the_competition_serializes_for_the_log_line() {
        let json =
            serde_json::to_value(competition(Kind::Sell, vec![response("driver", 100, 300)]))
                .unwrap();
        assert_eq!(json["request"]["kind"], "sell");
        assert_eq!(json["request"]["amount"], "100");
        assert_eq!(
            json["request"]["sellToken"],
            Pubkey::new_from_array([1; 32]).to_string()
        );
        assert_eq!(json["quotes"][0]["driver"], "driver");
        assert_eq!(json["quotes"][0]["buyAmount"], "300");
        assert_eq!(
            json["quotes"][0]["solver"],
            Pubkey::new_from_array([7; 32]).to_string()
        );
    }
}
