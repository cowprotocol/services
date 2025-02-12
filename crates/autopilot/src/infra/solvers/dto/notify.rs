use {serde::Serialize, serde_with::serde_as};

#[serde_as]
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Request {
    Banned {
        reason: BanReason,
        banned_until_timestamp: u64,
    },
}

#[serde_as]
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BanReason {
    /// The driver won multiple consecutive auctions but never settled them.
    UnsettledConsecutiveAuctions,
    /// Driver's settle failure rate is above the threshold.
    HighSettleFailureRate,
}
