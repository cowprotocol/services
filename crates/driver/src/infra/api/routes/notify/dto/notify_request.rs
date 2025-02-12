use {crate::infra::notify, serde::Deserialize, serde_with::serde_as};

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NotifyRequest {
    /// The driver won multiple consecutive auctions but never settled them.
    UnsettledConsecutiveAuctions,
    /// Driver's success settling rate is below the threshold.
    HighSettleFailureRate,
}

impl From<NotifyRequest> for notify::Kind {
    fn from(value: NotifyRequest) -> Self {
        match value {
            NotifyRequest::UnsettledConsecutiveAuctions => {
                notify::Kind::Banned(notify::BanReason::UnsettledConsecutiveAuctions)
            }
            NotifyRequest::HighSettleFailureRate => {
                notify::Kind::Banned(notify::BanReason::HighSettleFailureRate)
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Unable to notify solver")]
    UnableToNotify,
}
