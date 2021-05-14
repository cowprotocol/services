use std::time::Duration;

use anyhow::Result;
use reqwest::{
    header::{HeaderValue, ACCEPT},
    Client, ClientBuilder,
};

pub fn default_http_client() -> Result<Client> {
    Ok(ClientBuilder::new()
        .user_agent("gp-v2-services/2.0.0")
        .default_headers(
            vec![(ACCEPT, HeaderValue::from_static("application/json"))]
                .into_iter()
                .collect(),
        )
        .timeout(Duration::from_secs(60))
        .https_only(true)
        .build()?)
}
