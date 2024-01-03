use {
    number::serialization::HexOrDecimalU256,
    primitive_types::{H160, U256},
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
};

#[serde_as]
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub sell_token: H160,
    pub buy_token: H160,
    pub kind: Kind,
    #[serde_as(as = "HexOrDecimalU256")]
    pub amount: U256,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    #[default]
    Buy,
    Sell,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged, rename_all = "camelCase", deny_unknown_fields)]
pub enum Response {
    Successful {
        #[serde_as(as = "HexOrDecimalU256")]
        sell_amount: U256,
        #[serde_as(as = "HexOrDecimalU256")]
        buy_amount: U256,
        gas: u64,
    },
    Unfillable {
        unfillable_reason: String,
    },
}
