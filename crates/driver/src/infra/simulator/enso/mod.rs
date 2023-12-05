use {crate::domain::eth, reqwest::ClientBuilder, thiserror::Error};

mod dto;

const GAS_LIMIT: u64 = 30_000_000;

#[derive(Debug, Clone)]
pub(super) struct Enso {
    url: reqwest::Url,
    chain_id: eth::ChainId,
}

#[derive(Debug, Clone)]
pub struct Config {
    /// The URL of the Transaction Simulator API.
    pub url: reqwest::Url,
}

impl Enso {
    pub(super) fn new(config: Config, chain_id: eth::ChainId) -> Self {
        Self {
            url: reqwest::Url::parse(&format!("{}api/v1/simulate", config.url)).unwrap(),
            chain_id,
        }
    }

    pub(super) async fn simulate(&self, tx: eth::Tx) -> Result<eth::Gas, Error> {
        let res: dto::Response = ClientBuilder::new()
            .build()
            .unwrap()
            .post(self.url.clone())
            .json(&dto::Request {
                chain_id: self.chain_id.into(),
                from: tx.from.into(),
                to: tx.to.into(),
                data: tx.input.into(),
                value: tx.value.into(),
                gas_limit: GAS_LIMIT,
                block_number: None,
                access_list: if tx.access_list.is_empty() {
                    None
                } else {
                    Some(tx.access_list.into())
                },
            })
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        res.into()
    }
}

#[derive(Debug, Error)]
#[error("Enso tx simulation error")]
pub enum Error {
    Http(#[from] reqwest::Error),
    Revert(String),
}

impl From<dto::Response> for Result<eth::Gas, Error> {
    fn from(response: dto::Response) -> Self {
        if !response.success {
            return Err(Error::Revert(format!(
                "{}: {}",
                response.exit_reason,
                hex::encode(&response.return_data)
            )));
        }
        Ok(response.gas_used.into())
    }
}
