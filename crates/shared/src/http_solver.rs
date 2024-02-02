use {
    crate::http_client::response_body_with_size_limit,
    ::model::auction::AuctionId,
    anyhow::{anyhow, Context},
    reqwest::{
        header::{self, HeaderValue},
        Client,
        StatusCode,
        Url,
    },
    serde_json::json,
    std::time::Duration,
    tracing::Instrument,
};

pub mod gas_model;
pub mod model;

const SOLVER_RESPONSE_SIZE_LIMIT: usize = 10_000_000;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("rate limited")]
    RateLimited,
    #[error("timeout")]
    DeadlineExceeded,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Implements an abstract HTTP solver API, can be mocked, instrumented, etc.
#[mockall::automock]
#[async_trait::async_trait]
pub trait HttpSolverApi: Send + Sync {
    /// Submit a batch auction to the solver and wait for a solution.
    async fn solve(
        &self,
        model: &model::BatchAuctionModel,
        timeout: Duration,
    ) -> Result<model::SettledBatchAuctionModel, Error>;

    /// Callback to notify the solver how it performed in the given auction (if
    /// it won or failed for some reason)
    fn notify_auction_result(&self, auction_id: AuctionId, result: model::AuctionResult);
}

/// Default implementation for HTTP solver API that uses the reqwest client.
pub struct DefaultHttpSolverApi {
    /// Name of this solver.
    ///
    /// Used for logging and metrics reporting purposes.
    pub name: String,

    /// Network ID or name.
    ///
    /// Used for logging and metrics reporting purposes.
    pub network_name: String,

    /// Chain ID.
    ///
    /// Used for logging and metrics reporting purposes.
    pub chain_id: u64,

    /// Base solver url.
    pub base: Url,

    /// The custom URL path used for the solve request.
    pub solve_path: String,

    /// An async HTTP client instance that will be used to interact with the
    /// solver.
    pub client: Client,

    /// Solve requests to the API are sent gzipped.
    pub gzip_requests: bool,

    /// Other solver parameters.
    pub config: SolverConfig,
}

/// Configuration for solver requests.
#[derive(Debug)]
pub struct SolverConfig {
    /// Optional value for the `X-API-KEY` header.
    pub api_key: Option<String>,

    /// Controls value of the `max_nr_exec_orders` parameter.
    pub max_nr_exec_orders: u32,

    /// Controls if/how to set `use_internal_buffers`.
    pub use_internal_buffers: Option<bool>,

    /// Controls the objective function to optimize for.
    pub objective: Option<Objective>,
}

impl Default for SolverConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            max_nr_exec_orders: 100,
            use_internal_buffers: None,
            objective: None,
        }
    }
}

#[derive(Debug)]
pub enum Objective {
    CappedSurplusFeesCosts,
    SurplusFeesCosts,
}

#[async_trait::async_trait]
impl HttpSolverApi for DefaultHttpSolverApi {
    async fn solve(
        &self,
        model: &model::BatchAuctionModel,
        timeout: Duration,
    ) -> Result<model::SettledBatchAuctionModel, Error> {
        // The timeout we give to the solver is one second less than
        // the deadline to make up for overhead from the network.
        // We use one second because a now-deleted solver used integer timeouts.
        let solver_timeout = timeout
            .checked_sub(Duration::from_secs(1))
            .context("no time left to send request")?;

        let mut url = crate::url::join(&self.base, &self.solve_path);

        let maybe_auction_id = model.metadata.as_ref().and_then(|data| data.auction_id);
        let instance_name = self.generate_instance_name(maybe_auction_id.unwrap_or(0));

        url.query_pairs_mut()
            .append_pair("instance_name", &instance_name)
            // We use integer remaining seconds for legacy reasons. Note that
            // this means that we don't have much granularity with the time
            // limit.
            .append_pair("time_limit", &solver_timeout.as_secs().to_string())
            .append_pair(
                "max_nr_exec_orders",
                self.config.max_nr_exec_orders.to_string().as_str(),
            );
        if let Some(use_internal_buffers) = self.config.use_internal_buffers {
            url.query_pairs_mut().append_pair(
                "use_internal_buffers",
                use_internal_buffers.to_string().as_str(),
            );
        }
        match self.config.objective {
            Some(Objective::CappedSurplusFeesCosts) => {
                url.query_pairs_mut()
                    .append_pair("objective", "cappedsurplusfeescosts");
            }
            Some(Objective::SurplusFeesCosts) => {
                url.query_pairs_mut()
                    .append_pair("objective", "surplusfeescosts");
            }
            _ => {}
        }
        if let Some(auction_id) = maybe_auction_id {
            url.query_pairs_mut()
                .append_pair("auction_id", auction_id.to_string().as_str());
        }
        let request_id = observe::request_id::get_task_local_storage();
        if let Some(id) = &request_id {
            url.query_pairs_mut().append_pair("request_id", id);
        }
        let query = url.query().map(ToString::to_string).unwrap_or_default();
        let mut request = self
            .client
            .post(url)
            .timeout(timeout)
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ACCEPT, "application/json");
        if let Some(id) = request_id {
            request = request.header("X-REQUEST-ID", id);
        }
        if let Some(api_key) = &self.config.api_key {
            let mut header = HeaderValue::from_str(api_key.as_str()).unwrap();
            header.set_sensitive(true);
            request = request.header("X-API-KEY", header);
        }
        if self.gzip_requests {
            let mut encoder = flate2::write::GzEncoder::new(Vec::new(), Default::default());
            serde_json::to_writer(&mut encoder, &model).unwrap();
            let body = encoder.finish().unwrap();
            request = request.header(header::CONTENT_ENCODING, "gzip");
            request = request.body(body);
        } else {
            let body = serde_json::to_vec(&model).unwrap();
            request = request.body(body);
        };
        // temporary log, not needed once the code is stable for colocation
        tracing::trace!(
            "http request url: {}, timeout: {:?}, body: {:?}",
            query,
            timeout,
            model
        );
        let mut response = request.send().await.map_err(|err| {
            if err.is_timeout() {
                Error::DeadlineExceeded
            } else {
                anyhow!(err).context("failed to send request").into()
            }
        })?;
        let status = response.status();
        let response_body =
            response_body_with_size_limit(&mut response, SOLVER_RESPONSE_SIZE_LIMIT)
                .await
                .context("response body")?;
        let text = std::str::from_utf8(&response_body).context("failed to decode response body")?;
        tracing::trace!(body = %text, "http response");
        let context = || format!("request query {query}, response body {text}");
        if status == StatusCode::TOO_MANY_REQUESTS {
            return Err(Error::RateLimited);
        }
        if !status.is_success() {
            return Err(anyhow!(
                "solver response is not success: status {}, {}",
                status,
                context()
            )
            .into());
        }
        serde_json::from_str(text)
            .with_context(|| format!("failed to decode response json, {}", context()))
            .map_err(Into::into)
    }

    fn notify_auction_result(&self, auction_id: AuctionId, result: model::AuctionResult) {
        let mut url = crate::url::join(&self.base, &self.solve_path);
        // `/notify` should be a sibling of the `/solve` endpoint
        url.path_segments_mut().unwrap().pop().push("notify");

        let chain_id = self.chain_id;
        let client = self.client.clone();
        let config_api_key = self.config.api_key.clone();
        tracing::debug!(solver_name = self.name, ?result, "notify auction result");
        let future = async move {
            url.query_pairs_mut()
                .append_pair("auction_id", auction_id.to_string().as_str());

            url.query_pairs_mut()
                .append_pair("chain_id", chain_id.to_string().as_str());

            let mut request = client
                .post(url)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::ACCEPT, "application/json");

            if let Some(api_key) = config_api_key {
                let mut header = HeaderValue::from_str(api_key.as_str()).unwrap();
                header.set_sensitive(true);
                request = request.header("X-API-KEY", header);
            }

            let _result = request.json(&json!(result)).send().await;
        };
        tokio::task::spawn(future.instrument(tracing::Span::current()));
    }
}

impl DefaultHttpSolverApi {
    fn generate_instance_name(&self, auction_id: AuctionId) -> String {
        let now = chrono::Utc::now();
        format!(
            "{}_{}_{}_{}",
            now, self.network_name, self.chain_id, auction_id
        )
        .chars()
        .map(|x| match x {
            ' ' => '_',
            '/' => '_',
            _ => x,
        })
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::{model::SettledBatchAuctionModel, *},
        flate2::write::GzEncoder,
        std::collections::HashMap,
        tokio::{
            io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        },
    };

    /// Reads a full http request. Returns headers and body.
    async fn handle_http_request(
        stream: &mut (impl AsyncRead + Unpin),
    ) -> (HashMap<String, String>, Vec<u8>) {
        let needle = b"\r\n\r\n";
        let mut buf: Vec<u8> = Default::default();
        let headers_end: usize = 'outer: loop {
            let old_len = buf.len();
            stream.read_buf(&mut buf).await.unwrap();
            let end = match buf.len().checked_sub(needle.len()) {
                None => continue,
                Some(i) => i,
            };
            for i in old_len..end {
                if &buf[i..i + needle.len()] == needle {
                    break 'outer i;
                }
            }
        };

        let mut lines = std::str::from_utf8(&buf[..headers_end])
            .unwrap()
            .split("\r\n");
        assert!(lines.next().unwrap().starts_with("POST"));

        let mut headers: HashMap<String, String> = Default::default();
        for line in lines {
            let mut split = line.split(": ");
            let key = split.next().unwrap();
            let value = split.next().unwrap();
            assert!(split.next().is_none());
            headers.insert(key.to_string(), value.to_string());
        }

        let content_length: usize = headers.get("content-length").unwrap().parse().unwrap();
        let old_len = buf.len();
        let body_start = headers_end + needle.len();
        buf.resize(body_start + content_length, 0);
        stream.read_exact(&mut buf[old_len..]).await.unwrap();

        (headers, buf.split_off(body_start))
    }

    #[tokio::test]
    async fn supports_gzip_response() {
        let listener = TcpListener::bind("localhost:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let listen = async move {
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                let (mut read, mut write) = stream.split();

                let (headers, body) = handle_http_request(&mut read).await;

                match headers.get("content-encoding").map(String::as_str) {
                    None => {
                        println!("reading plaintext request");
                        let _: serde_json::Value = serde_json::from_slice(body.as_slice()).unwrap();
                    }
                    Some("gzip") => {
                        println!("reading gzip request");
                        let reader = flate2::read::GzDecoder::new(body.as_slice());
                        let _: serde_json::Value = serde_json::from_reader(reader).unwrap();
                    }
                    _ => panic!(),
                }

                match headers.get("accept-encoding").map(String::as_str) {
                    None => {
                        println!("sending plaintext response");
                        let body =
                            serde_json::to_vec(&SettledBatchAuctionModel::default()).unwrap();
                        let response = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n";
                        write.write_all(response).await.unwrap();
                        write.write_all(&body).await.unwrap();
                    }
                    Some("gzip") => {
                        println!("sending gzip response");
                        let mut encoder = GzEncoder::new(Vec::new(), Default::default());
                        serde_json::to_writer(&mut encoder, &SettledBatchAuctionModel::default())
                            .unwrap();
                        let body = encoder.finish().unwrap();
                        let response = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Encoding: gzip\r\n\r\n";
                        write.write_all(response).await.unwrap();
                        write.write_all(&body).await.unwrap();
                    }
                    _ => panic!(),
                };
                stream.shutdown().await.unwrap();
            }
        };
        tokio::task::spawn(listen);

        let mut api = DefaultHttpSolverApi {
            name: Default::default(),
            network_name: Default::default(),
            chain_id: Default::default(),
            base: format!("http://localhost:{port}").parse().unwrap(),
            solve_path: "solve".to_owned(),
            client: Default::default(),
            gzip_requests: false,
            config: Default::default(),
        };
        // The default reqwest::Client supports gzip responses if the corresponding
        // crate feature is enabled.
        api.solve(&Default::default(), Duration::from_secs(1))
            .await
            .unwrap();

        // We can explicitly disable gzip response support.
        api.client = reqwest::ClientBuilder::new().no_gzip().build().unwrap();
        api.solve(&Default::default(), Duration::from_secs(1))
            .await
            .unwrap();

        // We can send a gzipped request. See debug prints for verification.
        api.gzip_requests = true;
        api.solve(&Default::default(), Duration::from_secs(1))
            .await
            .unwrap();
    }
}
