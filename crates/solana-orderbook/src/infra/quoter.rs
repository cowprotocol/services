//! Client for the drivers' quote routes.

use {
    futures::future::join_all,
    reqwest::Client,
    serde::{Deserialize, Serialize},
    serde_with::{DisplayFromStr, serde_as},
    solana_sdk::pubkey::Pubkey,
    std::time::Duration,
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
    /// The network fee of settling the quoted route, in lamports.
    pub network_fee: u64,
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
    #[serde_as(as = "DisplayFromStr")]
    network_fee: u64,
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
        let quotes: Vec<_> = join_all(
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

    /// Quote `order` on one driver. Failures are logged and swallowed: a
    /// driver rejecting the quote found no route, which is a routine outcome,
    /// while anything else is that driver misbehaving.
    async fn quote_one(&self, endpoint: &Url, order: &Order) -> Option<Quote> {
        let url = endpoint.join("quote").expect("valid /quote path");
        let body = RequestBody {
            sell_token: order.sell_token,
            buy_token: order.buy_token,
            amount: order.amount,
            kind: order.kind,
            deadline: chrono::Utc::now() + self.timeout.saturating_sub(RESPONSE_RESERVE),
        };
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
        Some(Quote {
            sell_amount: quoted.sell_amount,
            buy_amount: quoted.buy_amount,
            solver: quoted.solver,
            network_fee: quoted.network_fee,
        })
    }
}
