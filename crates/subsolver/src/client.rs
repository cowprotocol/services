use {crate::proposal::SignedProposal, url::Url};

pub struct ByosClient {
    http: reqwest::Client,
    base_url: Url,
}

impl ByosClient {
    pub fn new(base_url: Url) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("failed to build HTTP client"),
            base_url,
        }
    }

    pub async fn submit_proposal(&self, proposal: &SignedProposal) -> anyhow::Result<u64> {
        let url = self.base_url.join("/proposals").expect("valid url join");

        let body = serde_json::json!({
            "orderUid": format!("0x{}", const_hex::encode(proposal.order_uid)),
            "sellAmount": proposal.sell_amount.to_string(),
            "buyAmount": proposal.buy_amount.to_string(),
            "interactions": proposal.interactions,
            "validUntil": proposal.valid_until,
            "nonce": proposal.nonce.to_string(),
            "signature": format!("0x{}", const_hex::encode(proposal.signature)),
        });

        let response = self.http.post(url).json(&body).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            anyhow::bail!("BYOS rejected proposal: {status} {text}");
        }

        let result: serde_json::Value = response.json().await?;
        let id = result["id"].as_u64().unwrap_or(0);
        Ok(id)
    }
}
