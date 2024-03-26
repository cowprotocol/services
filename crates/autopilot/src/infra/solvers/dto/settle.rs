use {
    primitive_types::H256,
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
};

#[serde_as]
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    /// Unique ID of the solution (per driver competition), to settle.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub solution_id: u64,
}

#[serde_as]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Response {
    pub calldata: Calldata,
    pub tx_hash: H256,
}

#[serde_as]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Calldata {
    #[serde(with = "bytes_hex")]
    pub internalized: Vec<u8>,
    #[serde(with = "bytes_hex")]
    pub uninternalized: Vec<u8>,
}
