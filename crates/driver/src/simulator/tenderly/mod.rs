use {crate::logic::eth, thiserror::Error};

mod dto;

#[derive(Debug)]
pub(super) struct Tenderly {
    client: reqwest::Client,
    config: Config,
}

#[derive(Debug)]
pub struct Config {
    /// The URL of the Tenderly API.
    pub url: reqwest::Url,
    pub network: eth::Network,
    /// Save the transaction on Tenderly for later inspection, e.g. via the
    /// dashboard.
    pub save: bool,
    /// Save the transaction as above, even in the case of failure.
    pub save_if_fails: bool,
}

impl Tenderly {
    pub fn new(config: Config) -> Self {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        headers.insert(reqwest::header::ACCEPT, "application/json".parse().unwrap());
        Self {
            client: reqwest::ClientBuilder::new()
                .default_headers(headers)
                .build()
                .unwrap(),
            config,
        }
    }

    pub async fn simulate(
        &self,
        tx: &eth::Tx,
        access_list: &eth::AccessList,
        generate_access_list: GenerateAccessList,
    ) -> Result<super::Simulation, Error> {
        let url = self.config.url.join("simulate").unwrap();
        let res: dto::Response = self
            .client
            .post(url)
            .json(&dto::Request {
                network_id: self.config.network.to_string(),
                from: match tx.from {
                    eth::Account::Address(address) => address.into(),
                    eth::Account::PrivateKey(_) => panic!("expected an address, got a private key"),
                },
                to: tx.to.into(),
                input: tx.input.clone(),
                value: tx.value.into(),
                save: self.config.save,
                save_if_fails: self.config.save_if_fails,
                generate_access_list: generate_access_list == GenerateAccessList::Yes,
                access_list: if access_list.is_empty() {
                    None
                } else {
                    Some(access_list.clone().into())
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

#[derive(Debug, PartialEq, Eq)]
pub enum GenerateAccessList {
    Yes,
    No,
}

#[derive(Debug, Error)]
#[error("tenderly error")]
pub struct Error(#[from] reqwest::Error);
