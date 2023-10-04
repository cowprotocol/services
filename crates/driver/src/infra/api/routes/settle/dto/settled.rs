use {
    crate::{domain::competition, util::serialize},
    serde::Serialize,
    serde_with::serde_as,
};

impl Settled {
    pub fn new(calldata: competition::Settled) -> Self {
        Self {
            calldata: CalldataInner {
                internalized: calldata.internalized_calldata.into(),
                uninternalized: calldata.uninternalized_calldata.into(),
            },
        }
    }
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settled {
    calldata: CalldataInner,
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
