use {crate::infra::notify, serde::Deserialize, serde_with::serde_as};

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NotifyRequest {
    /// The driver won multiple consecutive auctions but never settled them.
    UnsettledConsecutiveAuctions(u64),
    /// Driver's success settling rate is below the threshold.
    HighSettleFailureRate(u64),
}

impl From<NotifyRequest> for notify::Kind {
    fn from(value: NotifyRequest) -> Self {
        match value {
            NotifyRequest::UnsettledConsecutiveAuctions(until_timestamp) => notify::Kind::Banned {
                reason: notify::BanReason::UnsettledConsecutiveAuctions,
                until_timestamp,
            },
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Unable to notify solver")]
    UnableToNotify,
}
