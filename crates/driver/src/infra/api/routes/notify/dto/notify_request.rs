use {crate::infra::notify, serde::Deserialize, serde_with::serde_as};

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NotifyRequest {
    Banned {
        reason: BanReason,
        banned_until_timestamp: u64,
    },
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BanReason {
    /// The driver won multiple consecutive auctions but never settled them.
    UnsettledConsecutiveAuctions,
    /// Driver's settle failure rate is above the threshold.
    HighSettleFailureRate,
}

impl From<NotifyRequest> for notify::Kind {
    fn from(value: NotifyRequest) -> Self {
        match value {
            NotifyRequest::Banned {
                reason,
                banned_until_timestamp,
            } => notify::Kind::Banned {
                reason: match reason {
                    BanReason::UnsettledConsecutiveAuctions => {
                        notify::BanReason::UnsettledConsecutiveAuctions
                    }
                    BanReason::HighSettleFailureRate => notify::BanReason::HighSettleFailureRate,
                },
                until_timestamp: banned_until_timestamp,
            },
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Unable to notify solver")]
    UnableToNotify,
}
