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
            sell_token: quote.sell.token.into(),
            buy_token: quote.buy.token.into(),
            sell_amount: quote.sell.amount,
            buy_amount: quote.buy.amount,
            interactions: quote
                .interactions
                .iter()
                .map(|interaction| match interaction {
                    competition::solution::Interaction::Custom(interaction) => {
                        Interaction::Custom(CustomInteraction {
                            internalize: interaction.internalize,
                            target: interaction.target.into(),
                            value: interaction.value.into(),
                            call_data: interaction.call_data.clone(),
                            inputs: interaction
                                .inputs
                                .iter()
                                .map(|asset| Asset {
                                    token: asset.token.into(),
                                    amount: asset.amount,
                                })
                                .collect(),
                            outputs: interaction
                                .outputs
                                .iter()
                                .map(|asset| Asset {
                                    token: asset.token.into(),
                                    amount: asset.amount,
                                })
                                .collect(),
                        })
                    }
                    competition::solution::Interaction::Liquidity(interaction) => {
                        Interaction::Liquidity(LiquidityInteraction {
                            internalize: interaction.internalize,
                            id: interaction.liquidity.id.into(),
                            input_token: interaction.input.token.into(),
                            output_token: interaction.output.token.into(),
                            input_amount: interaction.input.amount,
                            output_amount: interaction.output.amount,
                        })
                    }
                })
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde_as]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    // TODO I think this might be a mistake. Instead of these four fields, how about this just
    // returns a single amount field which is a U256? That should be enough, right? It would
    // also simplify some of the other code which is nice.
    sell_token: eth::H160,
    buy_token: eth::H160,
    #[serde_as(as = "serialize::U256")]
    sell_amount: eth::U256,
    #[serde_as(as = "serialize::U256")]
    buy_amount: eth::U256,

    interactions: Vec<Interaction>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Interaction {
    Liquidity(LiquidityInteraction),
    Custom(CustomInteraction),
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LiquidityInteraction {
    internalize: bool,
    id: usize,
    input_token: eth::H160,
    output_token: eth::H160,
    #[serde_as(as = "serialize::U256")]
    input_amount: eth::U256,
    #[serde_as(as = "serialize::U256")]
    output_amount: eth::U256,
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CustomInteraction {
    internalize: bool,
    target: eth::H160,
    #[serde_as(as = "serialize::U256")]
    value: eth::U256,
    #[serde_as(as = "serialize::Hex")]
    call_data: Vec<u8>,
    inputs: Vec<Asset>,
    outputs: Vec<Asset>,
}

#[serde_as]
#[derive(Debug, Serialize)]
struct Asset {
    token: eth::H160,
    #[serde_as(as = "serialize::U256")]
    amount: eth::U256,
}
