use {crate::domain::eth, thiserror::Error};

mod dto;

const API_URL: &str = "https://api.tenderly.co";

#[derive(Debug, Clone)]
pub(super) struct Tenderly {
    endpoint: reqwest::Url,
    client: reqwest::Client,
    config: Config,
    network_id: eth::NetworkId,
}

#[derive(Debug, Clone)]
pub struct Config {
    /// Optional Tenderly fork to use for simulation.
    pub fork: Option<String>,
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
    pub(super) fn new(config: Config, network_id: eth::NetworkId) -> Self {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        headers.insert(reqwest::header::ACCEPT, "application/json".parse().unwrap());
        let mut api_key = reqwest::header::HeaderValue::from_str(&config.api_key).unwrap();
        api_key.set_sensitive(true);
        headers.insert("x-access-key", api_key);
        let fork = config
            .fork
            .as_ref()
            .map(|fork| format!("/fork/{fork}"))
            .unwrap_or_default();
        Self {
            endpoint: reqwest::Url::parse(&format!(
                "{API_URL}/api/v1/account/{}/project/{}{fork}/simulate",
                config.user, config.project
            ))
            .unwrap(),
            client: reqwest::ClientBuilder::new()
                .default_headers(headers)
                .build()
                .unwrap(),
            config,
            network_id,
        }
    }

    pub(super) async fn simulate(
        &self,
        tx: eth::Tx,
        generate_access_list: GenerateAccessList,
    ) -> Result<Simulation, Error> {
        let res = self
            .client
            .post(self.endpoint.clone())
            .json(&dto::Request {
                network_id: self.network_id.to_string(),
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
            .error_for_status()?;
        let res = res.text().await?;
        tracing::debug!("tenderly simulation response: {}", res);
        let res: dto::Response = serde_json::from_str(&res)?;
        Ok(res.into())
    }

    // TODO Looks like I don't need this
    // TODO Probably inline this
    /*
    fn simulation_link(&self, tx: &eth::Tx) -> String {
        let fork = self
            .config
            .fork
            .as_ref()
            .map(|fork| format!("/fork/{fork}"))
            .unwrap_or_default();
        format!(
            "{DASHBOARD_URL}/{}/{}{fork}/simulation/new",
            self.config.user, self.config.project
        )
    }
    */
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
pub enum Error {
    Reqwest(#[from] reqwest::Error),
    Json(#[from] serde_json::Error),
}
