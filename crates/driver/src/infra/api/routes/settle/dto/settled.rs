use {
    crate::{domain::competition, util::serialize},
    serde::Serialize,
    serde_with::serde_as,
};

impl Settled {
    pub fn new(settled: competition::Settled) -> Self {
        Self {
            calldata: CalldataInner {
                internalized: settled.internalized_calldata.into(),
                uninternalized: settled.uninternalized_calldata.into(),
            },
            tx_hash: settled.tx_hash.0,
        }
    }
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settled {
    calldata: CalldataInner,
    tx_hash: primitive_types::H256,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CalldataInner {
    #[serde_as(as = "serialize::Hex")]
    internalized: Vec<u8>,
    #[serde_as(as = "serialize::Hex")]
    uninternalized: Vec<u8>,
}
