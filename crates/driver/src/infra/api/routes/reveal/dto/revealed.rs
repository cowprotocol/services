use {
    crate::{
        domain::{competition, competition::order},
        util::serialize,
    },
    serde::Serialize,
    serde_with::serde_as,
};

impl Revealed {
    pub fn new(reveal: competition::Revealed) -> Self {
        Self {
            orders: reveal.orders.into_iter().map(Into::into).collect(),
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
pub struct Revealed {
    #[serde_as(as = "Vec<serialize::Hex>")]
    orders: Vec<[u8; order::UID_LEN]>,
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
