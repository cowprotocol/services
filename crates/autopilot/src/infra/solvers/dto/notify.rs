use {
    chrono::{DateTime, Utc},
    serde::Serialize,
    serde_with::serde_as,
};

#[serde_as]
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Request {
    Banned {
        reason: BanReason,
        until: DateTime<Utc>,
    },
}

#[serde_as]
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BanReason {
    /// The driver won multiple consecutive auctions but never settled them.
    UnsettledConsecutiveAuctions,
    /// Driver's settle failure rate is above the threshold.
    HighSettleFailureRate,
}

impl BanReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            BanReason::UnsettledConsecutiveAuctions => "non_settling",
            BanReason::HighSettleFailureRate => "high_settle_failure_rate",
        }
    }
}
