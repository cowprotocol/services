use {
    crate::{
        domain::{eth, quote},
        util::serialize,
    },
    serde::Serialize,
    serde_with::serde_as,
};

impl Quote {
    pub fn new(quote: &quote::Quote) -> Self {
        Self {
            amount: quote.amount,
            interactions: quote
                .interactions
                .iter()
                .map(|interaction| InteractionWithMeta {
                    interaction: Interaction {
                        target: interaction.target.into(),
                        value: interaction.value.into(),
                        call_data: interaction.call_data.clone().into(),
                    },
                    internalize: interaction.internalize,
                    input_tokens: interaction.inputs.iter().map(|t| t.0 .0).collect(),
                })
                .collect(),
            solver: quote.solver.0,
        }
    }
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    #[serde_as(as = "serialize::U256")]
    amount: eth::U256,
    interactions: Vec<InteractionWithMeta>,
    solver: eth::H160,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Interaction {
    target: eth::H160,
    #[serde_as(as = "serialize::U256")]
    value: eth::U256,
    #[serde_as(as = "serialize::Hex")]
    call_data: Vec<u8>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InteractionWithMeta {
    interaction: Interaction,
    internalize: bool,
    input_tokens: Vec<eth::H160>,
}
