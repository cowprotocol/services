use {
    primitive_types::{H160, U256},
    serde::{Deserialize, Serialize},
};

#[derive(Eq, PartialEq, Clone, Debug, Hash, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractionData {
    pub target: H160,
    #[serde(with = "crate::u256_decimal")]
    pub value: U256,
    #[serde(with = "crate::bytes_hex")]
    pub call_data: Vec<u8>,
}
