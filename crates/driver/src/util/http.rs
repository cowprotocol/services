use thiserror::Error;

pub async fn send(limit_bytes: usize, req: reqwest::RequestBuilder) -> Result<String, Error> {
    let mut res = req.send().await?;
    let mut data = Vec::new();
    while let Some(chunk) = res.chunk().await? {
        data.extend_from_slice(&chunk);
        if data.len() > limit_bytes {
            tracing::trace!(
                response = String::from_utf8_lossy(&data).as_ref(),
                "response size exceeded"
            );
            return Err(Error::ResponseTooLarge { limit_bytes });
        }
    }
    let body = String::from_utf8(data).map_err(Error::NotUtf8)?;
    if res.status().is_success() {
        Ok(body)
    } else {
        Err(Error::NotOk {
            code: res.status().as_u16(),
            body: body.clone(),
        })
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("response error: {0:?}")]
    Response(#[from] reqwest::Error),
    #[error("the response was too large, the limit was {limit_bytes} bytes")]
    ResponseTooLarge { limit_bytes: usize },
    #[error("the response could not be parsed as UTF-8: {0:?}")]
    NotUtf8(#[from] std::string::FromUtf8Error),
    #[error("the response status was not 2xx but {code:?}, body: {body:?}")]
    NotOk { code: u16, body: String },
}
