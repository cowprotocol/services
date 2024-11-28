use {
    primitive_types::H256,
    serde::{Deserialize, Serialize},
    serde_with::{serde_as, skip_serializing_none},
};

#[serde_as]
#[skip_serializing_none]
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    /// Unique ID of the solution (per driver competition), to settle.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub solution_id: u64,
    /// The last block number in which the solution TX can be included
    pub submission_deadline_latest_block: u64,
    /// Auction ID in which the specified solution ID is competing.
    #[serde_as(as = "Option<serde_with::DisplayFromStr>")]
    pub auction_id: Option<i64>,
}

#[serde_as]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    pub calldata: Calldata,
    pub tx_hash: H256,
}

#[serde_as]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Calldata {
    #[serde(with = "bytes_hex")]
    pub internalized: Vec<u8>,
    #[serde(with = "bytes_hex")]
    pub uninternalized: Vec<u8>,
}
