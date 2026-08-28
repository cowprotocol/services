//! Client for a driver's quote route.

use {
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

/// Asks a driver to quote one order.
#[derive(Clone, Debug)]
pub struct Quoter {
    client: Client,
    endpoint: Url,
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

/// What the driver quoted.
#[derive(Debug)]
pub struct Quote {
    pub sell_amount: u64,
    pub buy_amount: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The driver found no route for the pair.
    #[error("no route for the requested pair")]
    NoRoute,
    #[error("driver request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("driver answered {status}: {body}")]
    Status { status: u16, body: String },
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
}

impl Quoter {
    pub fn new(endpoint: Url, timeout: Duration) -> Self {
        Self {
            client: Client::new(),
            endpoint,
            timeout,
        }
    }

    /// Quote `order`, giving the driver until `timeout` to answer.
    pub async fn quote(&self, order: &Order) -> Result<Quote, Error> {
        let url = self.endpoint.join("quote").expect("valid /quote path");
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
            .await?;
        // A driver with no route answers 404, which is a normal outcome for an
        // untradeable pair rather than a failure to report.
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(Error::NoRoute);
        }
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::Status { status, body });
        }
        let quoted: ResponseBody = response.json().await?;
        Ok(Quote {
            sell_amount: quoted.sell_amount,
            buy_amount: quoted.buy_amount,
        })
    }
}
