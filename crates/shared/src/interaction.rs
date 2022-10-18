use ethcontract::Bytes;
use model::interaction::InteractionData as InteractionDataFromOrder;
use primitive_types::{H160, U256};
use serde::{Deserialize, Serialize};

pub trait Interaction: std::fmt::Debug + Send + Sync {
    // TODO: not sure if this should return a result.
    // Write::write returns a result but we know we write to a vector in memory so we know it will
    // never fail. Then the question becomes whether interactions should be allowed to fail encoding
    // for other reasons.
    fn encode(&self) -> Vec<EncodedInteraction>;
}

pub type EncodedInteraction = (
    H160,           // target
    U256,           // value
    Bytes<Vec<u8>>, // callData
);

impl Interaction for EncodedInteraction {
    fn encode(&self) -> Vec<EncodedInteraction> {
        vec![self.clone()]
    }
}

#[derive(Eq, PartialEq, Clone, Debug, Hash, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractionData {
    pub to: H160,
    pub value: U256,
    pub call_data: Vec<u8>,
}

impl Interaction for InteractionData {
    fn encode(&self) -> Vec<EncodedInteraction> {
        vec![(self.to, self.value, Bytes(self.call_data.clone()))]
    }
}

pub fn interaction_data_from_order(interaction: InteractionDataFromOrder) -> InteractionData {
    InteractionData {
        to: interaction.target,
        value: interaction.value,
        call_data: interaction.call_data,
    }
}
