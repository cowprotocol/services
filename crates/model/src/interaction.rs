use {
    alloy::primitives::{Address, U256},
    number::serialization::HexOrDecimalU256,
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
    std::fmt::{self, Debug, Formatter},
};

#[serde_as]
#[derive(Eq, PartialEq, Clone, Hash, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractionData {
    pub target: Address,
    #[serde_as(as = "HexOrDecimalU256")]
    pub value: U256,
    #[serde(with = "bytes_hex")]
    pub call_data: Vec<u8>,
}

impl Debug for InteractionData {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("InteractionData")
            .field("target", &self.target)
            .field("value", &self.value)
            .field("call_data", &const_hex::encode_prefixed(&self.call_data))
            .finish()
    }
}
