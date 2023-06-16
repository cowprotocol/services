use {
    anyhow::{anyhow, Context, Result},
    model::app_id::AppDataHash,
    reqwest::{Client, StatusCode},
    url::Url,
};

pub struct Ipfs {
    client: Client,
    base: Url,
    query: Option<String>,
}

impl Ipfs {
    pub fn new(client: Client, base: Url, query: Option<String>) -> Self {
        assert!(!base.cannot_be_a_base());
        Self {
            client,
            base,
            query,
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
    /// This function treats all status codes except "200 OK" as errors and you
    /// likely want to use it with a timeout.
    pub async fn fetch(&self, cid: &str) -> Result<Vec<u8>> {
        let url = self.prepare_url(cid);
        let response = self.client.get(url).send().await.context("send")?;
        let status = response.status();
        let body = response.bytes().await.context("body")?;
        match status {
            StatusCode::OK => Ok(body.into()),
            _ => {
                let body_text = String::from_utf8_lossy(&body);
                let body_text: &str = &body_text;
                Err(anyhow!("status {status}, body {body_text:?}"))
            }
        }
    }

    fn prepare_url(&self, cid: &str) -> Url {
        let mut url = self.base.clone();
        let mut path = url.path_segments_mut().unwrap();
        path.push("ipfs");
        path.push(cid);
        std::mem::drop(path);
        if let Some(query) = &self.query {
            url.set_query(Some(query.as_str()));
        }
        url
    }
}

/// Tries to find full app data corresponding to the contract app data on IPFS.
///
/// A return value of `Some` indicates that either the old or new CID format was
/// found on IPFS and points to valid utf-8.
///
/// A return value of `None` indicates that neither CID was found. This might be
/// a temporary condition as IPFS is a decentralized network.
pub async fn full_app_data_from_ipfs(
    ipfs: &Ipfs,
    contract_app_data: &AppDataHash,
) -> Option<String> {
    let old = old_app_data_cid(contract_app_data);
    let new = new_app_data_cid(contract_app_data);
    let fetch = |cid: String| async move {
        let result = ipfs.fetch(&cid).await;
        match &result {
            Ok(_) => {
                tracing::debug!("found full app data for {contract_app_data:?} at CID {cid}");
            }
            Err(err) => {
                tracing::debug!("no full app data for {contract_app_data:?} at CID {cid}: {err:?}");
            }
        };
        let result = String::from_utf8(result?);
        if result.is_err() {
            tracing::debug!("CID {cid} doesn't point to utf-8");
        }
        result.map_err(anyhow::Error::from)
    };
    futures::future::select_ok([std::pin::pin!(fetch(old)), std::pin::pin!(fetch(new))])
        .await
        .ok()
        .map(|(ok, _rest)| ok)
}

fn new_app_data_cid(contract_app_data: &AppDataHash) -> String {
    let raw_cid = app_data_hash::create_ipfs_cid(&contract_app_data.0);
    multibase::encode(multibase::Base::Base32Lower, raw_cid)
}

fn old_app_data_cid(contract_app_data: &AppDataHash) -> String {
    let mut raw_cid = [0u8; 4 + 32];
    raw_cid[0] = 1; // cid version
    raw_cid[1] = 0x70; // dag-pb
    raw_cid[2] = 0x12; // sha2-256
    raw_cid[3] = 32; // hash length
    raw_cid[4..].copy_from_slice(&contract_app_data.0);
    multibase::encode(multibase::Base::Base32Lower, raw_cid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn public_gateway() {
        let ipfs = Ipfs::new(Default::default(), "https://ipfs.io".parse().unwrap(), None);
        let cid = "Qma4Dwke5h8mgJyZMDRvKqM3RF7c6Mxcj3fR4um9UGaNF6";
        let content = ipfs.fetch(cid).await.unwrap();
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
        let content = ipfs.fetch(cid).await.unwrap();
        let content = std::str::from_utf8(&content).unwrap();
        println!("{content}");
    }

    // Can be compared with CID explorer to make sure CIDs encode the right data.
    #[test]
    fn cid() {
        let hash = AppDataHash(hex_literal::hex!(
            "8af4e8c9973577b08ac21d17d331aade86c11ebcc5124744d621ca8365ec9424"
        ));
        let cid = new_app_data_cid(&hash);
        println!("{cid}");

        let hash = AppDataHash(hex_literal::hex!(
            "AE16F2D8B960FFE3DE70E074CECFB24441D4CDC67E8A68566B9E6CE3037CB41D"
        ));
        let cid = old_app_data_cid(&hash);
        println!("{cid}");
    }
}
