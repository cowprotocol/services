use {
    crate::{domain::competition, util::serialize},
    serde::Serialize,
    serde_with::serde_as,
};

impl RevealResponse {
    pub fn new(reveal: competition::Revealed) -> Self {
        Self {
            calldata: Calldata {
                internalized: reveal.internalized_calldata.into(),
                uninternalized: reveal.uninternalized_calldata.into(),
            },
        }
    }
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevealResponse {
    calldata: Calldata,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Calldata {
    #[serde_as(as = "serialize::Hex")]
    internalized: Vec<u8>,
    #[serde_as(as = "serialize::Hex")]
    uninternalized: Vec<u8>,
}
