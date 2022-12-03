use thiserror::Error;

pub async fn send(limit_bytes: usize, req: reqwest::RequestBuilder) -> Result<String, Error> {
    let mut res = req.send().await?;
    let mut data = Vec::new();
    while let Some(chunk) = res.chunk().await? {
        if data.len() + chunk.len() > limit_bytes {
            return Err(Error::ResponseTooLarge { limit_bytes });
        }
        data.extend_from_slice(&chunk);
    }
    String::from_utf8(data).map_err(Into::into)
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("response error: {0:?}")]
    Response(#[from] reqwest::Error),
    #[error("the response was too large, the limit was {limit_bytes} bytes")]
    ResponseTooLarge { limit_bytes: usize },
    #[error("the response could not be parsed as UTF-8: {0:?}")]
    NotUtf8(#[from] std::string::FromUtf8Error),
}
