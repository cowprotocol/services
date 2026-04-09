use {
    alloy::primitives::Bytes,
    eth_domain_types::{Address, Ether},
};

/// An onchain transaction which interacts with a smart contract.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Interaction {
    pub target: Address,
    pub value: Ether,
    pub call_data: Bytes,
}

impl From<Interaction> for model::interaction::InteractionData {
    fn from(interaction: Interaction) -> Self {
        Self {
            target: interaction.target,
            value: interaction.value.0,
            call_data: interaction.call_data.to_vec(),
        }
    }
}

impl From<model::interaction::InteractionData> for Interaction {
    fn from(interaction: model::interaction::InteractionData) -> Self {
        Self {
            target: interaction.target,
            value: interaction.value.into(),
            call_data: interaction.call_data.into(),
        }
    }
}
