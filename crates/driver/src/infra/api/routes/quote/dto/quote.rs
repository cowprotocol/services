use {
    crate::{
        domain::{self, eth, quote},
        util::serialize,
    },
    model::{
        order::{BuyTokenDestination, SellTokenSource},
        signature::SigningScheme,
    },
    serde::Serialize,
    serde_with::serde_as,
};

impl Quote {
    pub fn new(quote: quote::Quote) -> Self {
        Self {
            amount: quote.amount,
            pre_interactions: quote
                .pre_interactions
                .iter()
                .map(|interaction| Interaction {
                    target: interaction.target.into(),
                    value: interaction.value.into(),
                    call_data: interaction.call_data.clone().into(),
                })
                .collect(),
            interactions: quote
                .interactions
                .iter()
                .map(|interaction| Interaction {
                    target: interaction.target.into(),
                    value: interaction.value.into(),
                    call_data: interaction.call_data.clone().into(),
                })
                .collect(),
            solver: quote.solver.0,
            gas: quote.gas.map(|gas| gas.0.as_u64()),
            tx_origin: quote.tx_origin.map(|addr| addr.0),
            jit_orders: quote.jit_orders.into_iter().map(Into::into).collect(),
        }
    }
}

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    #[serde_as(as = "serialize::U256")]
    amount: eth::U256,
    pre_interactions: Vec<Interaction>,
    interactions: Vec<Interaction>,
    solver: eth::H160,
    #[serde(skip_serializing_if = "Option::is_none")]
    gas: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tx_origin: Option<eth::H160>,
    jit_orders: Vec<JitOrder>,
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

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct JitOrder {
    buy_token: eth::H160,
    sell_token: eth::H160,
    #[serde_as(as = "serialize::U256")]
    sell_amount: eth::U256,
    #[serde_as(as = "serialize::U256")]
    buy_amount: eth::U256,
    receiver: eth::H160,
    valid_to: u32,
    #[serde_as(as = "serialize::Hex")]
    app_data: [u8; 32],
    side: Side,
    sell_token_source: SellTokenSource,
    buy_token_destination: BuyTokenDestination,
    #[serde_as(as = "serialize::Hex")]
    signature: Vec<u8>,
    signing_scheme: SigningScheme,
}

impl From<domain::competition::order::Jit> for JitOrder {
    fn from(jit: domain::competition::order::Jit) -> Self {
        Self {
            sell_token: jit.sell.token.into(),
            buy_token: jit.buy.token.into(),
            sell_amount: jit.sell.amount.into(),
            buy_amount: jit.buy.amount.into(),
            receiver: jit.receiver.into(),
            valid_to: jit.valid_to.into(),
            app_data: jit.app_data.into(),
            side: jit.side.into(),
            sell_token_source: jit.sell_token_balance.into(),
            buy_token_destination: jit.buy_token_balance.into(),
            signature: jit.signature.data.into(),
            signing_scheme: jit.signature.scheme.to_boundary_scheme(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
enum Side {
    Sell,
    Buy,
}

impl From<domain::competition::order::Side> for Side {
    fn from(side: domain::competition::order::Side) -> Self {
        match side {
            domain::competition::order::Side::Sell => Side::Sell,
            domain::competition::order::Side::Buy => Side::Buy,
        }
    }
}
