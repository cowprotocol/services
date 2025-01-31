use {serde::Serialize, serde_with::serde_as};

#[serde_as]
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Request {
    /// The driver won multiple consecutive auctions but never settled them.
    UnsettledConsecutiveAuctions,
    /// Driver's success settling rate is below the threshold.
    LowSettlingRate,
}
