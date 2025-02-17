use {
    chrono::{DateTime, Utc},
    serde::Serialize,
    serde_with::serde_as,
};

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Request {
    Banned {
        reason: BanReason,
        until: DateTime<Utc>,
    },
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BanReason {
    UnsettledConsecutiveAuctions,
}
