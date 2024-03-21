use {crate::domain::eth, thiserror::Error};

mod dto;

const DEFAULT_URL: &str = "https://api.tenderly.co/api";

#[derive(Debug, Clone)]
pub(super) struct Tenderly {
    endpoint: reqwest::Url,
    client: reqwest::Client,
    config: Config,
    chain_id: eth::ChainId,
}

#[derive(Debug, Clone)]
pub struct Config {
    /// The URL of the Tenderly API.
    pub url: Option<reqwest::Url>,
    /// The Tenderly API key.
    pub api_key: String,
    /// The user associated with the API key.
    pub user: String,
    /// The project to use.
    pub project: String,
    /// Save the transaction on Tenderly for later inspection, e.g. via the
    /// dashboard.
    pub save: bool,
    /// Save the transaction as above, even in the case of failure.
    pub save_if_fails: bool,
}

impl Tenderly {
    pub(super) fn new(config: Config, chain_id: eth::ChainId) -> Self {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        headers.insert(reqwest::header::ACCEPT, "application/json".parse().unwrap());
        let mut api_key = reqwest::header::HeaderValue::from_str(&config.api_key).unwrap();
        api_key.set_sensitive(true);
        headers.insert("x-access-key", api_key);
        Self {
            endpoint: reqwest::Url::parse(&format!(
                "{}/v1/account/{}/project/{}/simulate",
                config
                    .url
                    .as_ref()
                    .map(|url| url.to_string())
                    .unwrap_or_else(|| DEFAULT_URL.to_owned()),
                config.user,
                config.project
            ))
            .unwrap(),
            client: reqwest::ClientBuilder::new()
                .default_headers(headers)
                .build()
                .unwrap(),
            config,
            chain_id,
        }
    }

    pub(super) async fn simulate(
        &self,
        tx: eth::Tx,
        generate_access_list: GenerateAccessList,
    ) -> Result<Simulation, Error> {
        let res: dto::Response = self
            .client
            .post(self.endpoint.clone())
            .json(&dto::Request {
                network_id: self.chain_id.to_string(),
                from: tx.from.into(),
                to: tx.to.into(),
                input: tx.input.into(),
                value: tx.value.into(),
                save: self.config.save,
                save_if_fails: self.config.save_if_fails,
                generate_access_list: generate_access_list == GenerateAccessList::Yes,
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

#[derive(Debug)]
pub struct Simulation {
    pub id: SimulationId,
    pub gas: eth::Gas,
    pub access_list: eth::AccessList,
}

// We want the string to be printed together with a simulation so we
// don't care that it's not used for anything else.
#[allow(dead_code)]
#[derive(Debug)]
pub struct SimulationId(String);

#[derive(Debug, PartialEq, Eq)]
pub(super) enum GenerateAccessList {
    Yes,
    No,
}

#[derive(Debug, Error)]
#[error("tenderly error")]
pub enum Error {
    Http(#[from] reqwest::Error),
    Revert(SimulationId),
}

impl From<dto::Response> for Result<Simulation, Error> {
    fn from(res: dto::Response) -> Self {
        let id = SimulationId(res.simulation.id);
        if res.transaction.status {
            Ok(Simulation {
                id,
                gas: res.transaction.gas_used.into(),
                access_list: res.generated_access_list.unwrap_or_default().into(),
            })
        } else {
            Err(Error::Revert(id))
        }
    }
}
