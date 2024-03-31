use {
    anyhow::{anyhow, Context, Result},
    reqwest::{Client, ClientBuilder, StatusCode},
    serde::{Deserialize, Serialize},
    std::time::Duration,
    url::Url,
};

pub struct Ipfs {
    client: Client,
    base: Url,
    auth_token: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PinByHashRequest {
    pub hash_to_pin: String,
    pub pinata_metadata: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PinResponse {
    pub id: String,
    pub ipfs_hash: String,
    pub status: String,
    pub name: Option<String>,
}

impl Ipfs {
    pub fn new(client: ClientBuilder, base: Url, auth_token: Option<String>) -> Self {
        assert!(!base.cannot_be_a_base());
        Self {
            client: client.timeout(Duration::from_secs(5)).build().unwrap(),
            base,
            auth_token,
        }
    }

    /// IPFS gateway behavior when a CID cannot be found is inconsistent and can
    /// be confusing:
    ///
    /// - The public ipfs.io gateway responds "504 Gateway Timeout" after 2
    ///   minutes.
    /// - The public cloudflare gateway responds "524" after 20 seconds.
    /// - A private Pinata gateway responds "404 Not Found" after 2 minutes.
    ///
    /// This function treats timeouts and all status codes except "200 OK" as
    /// Ok(None).
    pub async fn fetch(&self, cid: &str) -> Result<Option<Vec<u8>>> {
        let url = self.prepare_url(cid);
        let response = match self.client.get(url).send().await {
            Ok(response) => response,
            Err(err) if err.is_timeout() => return Ok(None),
            result @ Err(_) => return Err(result.context("send").unwrap_err()),
        };
        let status = response.status();
        let body = response.bytes().await.context("body")?;
        match status {
            StatusCode::OK => Ok(Some(body.into())),
            _ => {
                let body = String::from_utf8_lossy(&body);
                let body: &str = &body;
                tracing::trace!(%status, %body, "IPFS not found");
                Ok(None)
            }
        }
    }

    fn prepare_url(&self, cid: &str) -> Url {
        let mut url = shared::url::join(&self.base, &format!("ipfs/{cid}"));
        if let Some(jwt) = &self.auth_token {
            url.set_query(Some(&format!("pinataGatewayToken={}", jwt)));
        }
        url
    }

    pub async fn add(&self, data: PinByHashRequest) -> Result<PinResponse> {
        let url = format!("{}pinning/pinByHash", self.base);
        let jwt = if let Some(jwt) = &self.auth_token {
            jwt
        } else {
            return Err(anyhow!("Can't post without valid JSON web token"));
        };
        let resp = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", jwt))
            .header("Content-Type", "application/json")
            .json(&data)
            .send()
            .await?;

        if resp.status().is_success() {
            let data = resp.text().await?;
            let response: PinResponse = serde_json::from_str(&data)?;
            tracing::debug!("AppData pinned to IPFS with CID: {}", response.ipfs_hash);
            Ok(response)
        } else {
            Err(anyhow!(
                "Error posting AppData to IPFS: {:?}",
                resp.text().await
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn public_gateway() {
        let ipfs = Ipfs::new(Default::default(), "https://ipfs.io".parse().unwrap(), None);
        let cid = "Qma4Dwke5h8mgJyZMDRvKqM3RF7c6Mxcj3fR4um9UGaNF6";
        let content = ipfs.fetch(cid).await.unwrap().unwrap();
        let content = std::str::from_utf8(&content).unwrap();
        println!("{content}");
    }

    #[tokio::test]
    #[ignore]
    async fn private_gateway() {
        let url = std::env::var("url").unwrap();
        let query = std::env::var("query").unwrap();
        let ipfs = Ipfs::new(Default::default(), url.parse().unwrap(), Some(query));
        let cid = "Qma4Dwke5h8mgJyZMDRvKqM3RF7c6Mxcj3fR4um9UGaNF6";
        let content = ipfs.fetch(cid).await.unwrap().unwrap();
        let content = std::str::from_utf8(&content).unwrap();
        println!("{content}");
    }

    #[tokio::test]
    #[ignore]
    async fn not_found() {
        observe::tracing::initialize_reentrant("orderbook::ipfs=trace");
        let ipfs = Ipfs::new(Default::default(), "https://ipfs.io".parse().unwrap(), None);
        let cid = "Qma4Dwke5h8mgJyZMDRvKqM3RF7c6Mxcj3fR4um9UGaNF7";
        let result = ipfs.fetch(cid).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    #[ignore]
    async fn post_ipfs_doc() {
        observe::tracing::initialize_reentrant("orderbook::ipfs=trace");
        let auth = std::env::var("pinata_auth").ok();
        let ipfs = Ipfs::new(
            Default::default(),
            "https://api.pinata.cloud".parse().unwrap(),
            auth,
        );
        // Format of pin_metadata this MUST BE as follows:
        // {"keyvalues":{"appData":"{\"appCode\":\"CoW Swap\"}"}}
        // Where the inner appData field is an escaped JSON string.
        let pin_data = PinByHashRequest {
            hash_to_pin: "bafkrwih2jqcare44k37njjzoez3bxrxdt4p72nen5fiftlsiyoghqkrbmu".into(),
            pinata_metadata:  r#"{"keyvalues":{"appData":"{\"appCode\":\"CoW Swap\",\"environment\":\"production\"}"}}"#.into()
        };
        let result = ipfs.add(pin_data).await;
        assert_eq!(
            result.unwrap().ipfs_hash,
            "bafkrwih2jqcare44k37njjzoez3bxrxdt4p72nen5fiftlsiyoghqkrbmu"
        )
    }
}
