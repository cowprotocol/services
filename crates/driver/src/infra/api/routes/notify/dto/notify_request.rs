use {crate::infra::notify, serde::Deserialize, serde_with::serde_as};

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NotifyRequest {
    Banned {
        reason: BanReason,
        until_timestamp: u64,
    },
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BanReason {
    /// The driver won multiple consecutive auctions but never settled them.
    UnsettledConsecutiveAuctions,
}

impl From<NotifyRequest> for notify::Kind {
    fn from(value: NotifyRequest) -> Self {
        match value {
            NotifyRequest::Banned {
                reason,
                until_timestamp,
            } => notify::Kind::Banned {
                reason: match reason {
                    BanReason::UnsettledConsecutiveAuctions => {
                        notify::BanReason::UnsettledConsecutiveAuctions
                    }
                },
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
