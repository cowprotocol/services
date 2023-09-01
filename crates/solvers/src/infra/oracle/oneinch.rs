use crate::domain::{auction, eth};

/// 1Inch on-chain spot price aggregator.
pub struct OneInch {
    oracle: contracts::OneInchOffchainOracle,
}

pub struct Config {
    /// The Ethereum RPC URL to use.
    pub ethrpc: reqwest::Url,

    /// The address of the `OffchainOracle` contract. Specify `None` to use the
    /// default contract address for the connected chain.
    pub oracle: Option<eth::ContractAddress>,
}

impl OneInch {
    pub async fn new(config: Config) -> Result<Self, Error> {
        let web3 = ethrpc::web3(module_path!(), &config.ethrpc);
        let oracle = match config.oracle {
            Some(address) => contracts::OneInchOffchainOracle::at(&web3, address.0),
            None => contracts::OneInchOffchainOracle::deployed(&web3).await?,
        };

        Ok(Self { oracle })
    }

    pub async fn price(&self, token: eth::TokenAddress) -> Result<auction::Price, Error> {
        let rate = self.oracle.get_rate_to_eth(token.0, false).call().await?;
        if rate.is_zero() {
            // The oracle contract returns a 0 rate for tokens that it doesn't
            // know about.
            return Err(Error::NotFound);
        }

        Ok(auction::Price(eth::Ether(rate)))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the token price is not available")]
    NotFound,
    #[error(transparent)]
    Blockchain(Box<dyn std::error::Error + Send + Sync>),
}

impl From<contracts::ethcontract::errors::MethodError> for Error {
    fn from(value: contracts::ethcontract::errors::MethodError) -> Self {
        Self::Blockchain(value.into())
    }
}

impl From<contracts::ethcontract::errors::DeployError> for Error {
    fn from(value: contracts::ethcontract::errors::DeployError) -> Self {
        Self::Blockchain(value.into())
    }
}
