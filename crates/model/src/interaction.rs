use {
    number::U256,
    primitive_types::H160,
    serde::{Deserialize, Serialize},
    std::fmt::{self, Debug, Formatter},
};

#[derive(Eq, PartialEq, Clone, Hash, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractionData {
    pub target: H160,
    pub value: U256,
    #[serde(with = "crate::bytes_hex")]
    pub call_data: Vec<u8>,
}

impl Debug for InteractionData {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("InteractionData")
            .field("target", &self.target)
            .field("value", &self.value)
            .field(
                "call_data",
                &format_args!("0x{}", hex::encode(&self.call_data)),
            )
            .finish()
    }
}
