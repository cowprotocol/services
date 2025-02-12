use {serde::Serialize, serde_with::serde_as};

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Request {
    Banned {
        reason: BanReason,
        until_timestamp: u64,
    },
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BanReason {
    UnsettledConsecutiveAuctions,
}
