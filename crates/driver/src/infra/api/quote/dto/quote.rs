use {
    crate::{
        domain::{competition, eth},
        util::serialize,
    },
    serde::Serialize,
    serde_with::serde_as,
};

impl Quote {
    pub fn from_domain(quote: &competition::quote::Quote) -> Self {
        Self {
            amount: quote.amount,
            interactions: quote
                .interactions
                .iter()
                .map(|interaction| match interaction {
                    competition::solution::Interaction::Custom(interaction) => Interaction {
                        target: interaction.target.into(),
                        value: interaction.value.into(),
                        call_data: interaction.call_data.clone(),
                    },
                    competition::solution::Interaction::Liquidity(..) => todo!(),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde_as]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    amount: eth::U256,
    interactions: Vec<Interaction>,
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
