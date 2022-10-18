use primitive_types::{H160, U256};
use serde::{Deserialize, Serialize};

#[derive(Eq, PartialEq, Clone, Debug, Hash, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractionData {
    pub target: H160,
    pub value: U256,
    pub call_data: Vec<u8>,
}
