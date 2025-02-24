use {
    crate::infra::notify,
    chrono::{DateTime, Utc},
    serde::Deserialize,
    serde_with::serde_as,
};

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NotifyRequest {
    Banned {
        reason: BanReason,
        until: DateTime<Utc>,
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
            NotifyRequest::Banned { reason, until } => notify::Kind::Banned {
                reason: match reason {
                    BanReason::UnsettledConsecutiveAuctions => {
                        notify::BanReason::UnsettledConsecutiveAuctions
                    }
                    BanReason::HighSettleFailureRate => notify::BanReason::HighSettleFailureRate,
                },
                until,
            },
        }
    }
}
