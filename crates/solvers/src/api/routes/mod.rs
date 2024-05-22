use serde::Serialize;

mod healthz;
mod metrics;
mod solve;

pub(super) use {healthz::healthz, metrics::metrics, solve::solve};

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Response<T> {
    Ok(T),
    Err(Error),
}

#[derive(Debug, Serialize)]
pub struct Error {
    pub message: &'static str,
}

impl From<&'static str> for Error {
    fn from(message: &'static str) -> Self {
        Self { message }
    }
}
