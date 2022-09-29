use thiserror::Error;

#[derive(Debug, Clone, Error)]
#[error("Fail")]
pub enum SignatureEstimationError {}

pub async fn signature_gas_estimate() -> Result<Option<f64>, SignatureEstimationError> {
    Ok(None)
}
