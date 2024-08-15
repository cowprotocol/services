use {
    crate::domain::eth,
    ethrpc::block_stream::CurrentBlockWatcher,
    reqwest::ClientBuilder,
    std::time::Duration,
    thiserror::Error,
};

mod dto;

const GAS_LIMIT: u64 = 30_000_000;

#[derive(Debug, Clone)]
pub(super) struct Enso {
    url: reqwest::Url,
    chain_id: eth::ChainId,
    current_block: CurrentBlockWatcher,
    network_block_interval: Option<Duration>,
}

#[derive(Debug, Clone)]
pub struct Config {
    /// The URL of the Transaction Simulator API.
    pub url: reqwest::Url,
    /// The time between new blocks in the network.
    pub network_block_interval: Option<Duration>,
}

impl Enso {
    pub(super) fn new(
        config: Config,
        chain_id: eth::ChainId,
        current_block: CurrentBlockWatcher,
    ) -> Self {
        Self {
            url: reqwest::Url::parse(&format!("{}api/v1/simulate", config.url)).unwrap(),
            chain_id,
            current_block,
            network_block_interval: config.network_block_interval,
        }
    }

    pub(super) async fn simulate(&self, tx: eth::Tx) -> Result<eth::Gas, Error> {
        let current_block = *self.current_block.borrow();

        let (block_number, block_timestamp) = match self.network_block_interval {
            None => (None, None), // use default values which result in simulation on `latest`
            Some(duration) => {
                // We would like to simulate on the `pending` block instead of the `latest`
                // block. Unfortunately `enso` does not support that so to get closer to
                // the actual behavior of the `pending` block we use the block number of
                // the `latest` block but the timestamp of the `pending` block.
                let block_number = current_block.number;
                let next_timestamp = current_block.timestamp + duration.as_secs();
                (Some(block_number), Some(next_timestamp))
            }
        };

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
                block_number,
                block_timestamp,
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
