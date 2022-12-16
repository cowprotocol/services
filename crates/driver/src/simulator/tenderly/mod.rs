use {crate::logic::eth, thiserror::Error};

mod dto;

#[derive(Debug)]
pub(super) struct Tenderly {
    client: reqwest::Client,
    config: Config,
}

#[derive(Debug)]
pub struct Config {
    pub network_id: eth::NetworkName,
    /// Save the transaction on Tenderly for later inspection, e.g. via the
    /// dashboard.
    pub save: bool,
    /// Save the transaction as above, even in the case of failure.
    pub save_if_fails: bool,
}

impl Tenderly {
    pub fn new(_config: Config) -> Self {
        todo!()
    }

    pub async fn simulate(
        &self,
        _tx: &eth::Tx,
        _access_list: &eth::AccessList,
        _speed: Speed,
    ) -> Result<eth::Simulation, Error> {
        todo!()
    }
}

#[derive(Debug)]
pub(super) enum Speed {
    Slow,
    Fast,
}

#[derive(Debug, Error)]
#[error("tenderly error")]
pub struct Error;
