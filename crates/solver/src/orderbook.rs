use anyhow::{Context, Result};
use model::{auction::AuctionWithId, solver_competition::SolverCompetition};
use reqwest::{Client, Url};

pub struct OrderBookApi {
    base: Url,
    client: Client,
    competition_auth: Option<String>,
}

impl OrderBookApi {
    /// base: protocol and host of the url. example: `https://example.com`
    pub fn new(base: Url, client: Client, competition_auth: Option<String>) -> Self {
        Self {
            base,
            client,
            competition_auth,
        }
    }

    pub async fn get_auction(&self) -> Result<AuctionWithId> {
        let url = self.base.join("api/v1/auction")?;
        let auction = self.client.get(url).send().await?.json().await?;
        Ok(auction)
    }

    pub async fn send_solver_competition(&self, body: &SolverCompetition) -> Result<()> {
        let url = self.base.join("api/v1/solver_competition")?;
        let mut request = self.client.post(url);
        if let Some(auth) = &self.competition_auth {
            request = request.header("Authorization", auth)
        };
        let response = request.json(&body).send().await.context("send")?;
        if let Err(err) = response.error_for_status_ref() {
            let body = response.text().await;
            return Err(anyhow::Error::new(err).context(format!("body: {:?}", body)));
        }
        Ok(())
    }
}

#[cfg(test)]
pub mod test_util {
    use super::*;

    // cargo test local_orderbook -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn local_orderbook() {
        let api = OrderBookApi::new(
            Url::parse("http://localhost:8080").unwrap(),
            Client::new(),
            None,
        );
        let auction = api.get_auction().await.unwrap();
        println!("{:#?}", auction);
    }

    // cargo test real_orderbook -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn real_orderbook() {
        let api = OrderBookApi::new(
            Url::parse("https://barn.api.cow.fi/mainnet/").unwrap(),
            Client::new(),
            None,
        );
        let auction = api.get_auction().await.unwrap();
        println!("{:#?}", auction);
    }
}
