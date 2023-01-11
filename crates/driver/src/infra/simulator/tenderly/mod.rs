use {crate::domain::eth, thiserror::Error};

mod dto;

const DEFAULT_URL: &str = "https://api.tenderly.co/api";

#[derive(Debug, Clone)]
pub(super) struct Tenderly {
    endpoint: reqwest::Url,
    client: reqwest::Client,
    config: Config,
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
    pub network_id: eth::NetworkId,
    /// Save the transaction on Tenderly for later inspection, e.g. via the
    /// dashboard.
    pub save: bool,
    /// Save the transaction as above, even in the case of failure.
    pub save_if_fails: bool,
}

impl Tenderly {
    pub(super) fn new(config: Config) -> Self {
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
                network_id: self.config.network_id.to_string(),
                from: tx.from.into(),
                to: tx.to.into(),
                input: tx.input,
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
        Ok(res.into())
    }
}

#[derive(Debug)]
pub struct Simulation {
    pub gas: eth::Gas,
    pub access_list: eth::AccessList,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum GenerateAccessList {
    Yes,
    No,
}

#[derive(Debug, Error)]
#[error("tenderly error")]
pub struct Error(#[from] reqwest::Error);
