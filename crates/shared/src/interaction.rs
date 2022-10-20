use ethcontract::Bytes;
use primitive_types::{H160, U256};

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

impl Interaction for model::interaction::InteractionData {
    fn encode(&self) -> Vec<EncodedInteraction> {
        vec![(self.target, self.value, Bytes(self.call_data.clone()))]
    }
}
