use {derive_more::Debug, std::time::Duration};

#[derive(Debug, Clone)]
pub struct Config {
    pub liquorice: Option<Liquorice>,
}

#[derive(Debug, Clone)]
pub struct Liquorice {
    /// Liquorice API base URL
    pub base_url: String,
    /// API key for the Liquorice API
    #[debug(ignore)]
    pub api_key: String,
    /// The HTTP timeout for requests to the Liquorice API
    pub http_timeout: Duration,
}
