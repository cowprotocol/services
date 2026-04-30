use {
    crate::domain::{self, competition, quote},
    eth_domain_types as eth,
    serde::Deserialize,
    serde_with::serde_as,
};

impl Order {
    pub fn into_domain(self) -> quote::Order {
        self.into_domain_with_interactions(
            Default::default(),
            Default::default(),
            Default::default(),
        )
    }

    pub fn into_domain_with_interactions(
        self,
        owner: eth::Address,
        pre_interactions: Vec<domain::Interaction>,
        post_interactions: Vec<domain::Interaction>,
    ) -> quote::Order {
        quote::Order {
            tokens: quote::Tokens::new(self.sell_token.into(), self.buy_token.into()),
            amount: self.amount.into(),
            side: match self.kind {
                Kind::Sell => competition::order::Side::Sell,
                Kind::Buy => competition::order::Side::Buy,
            },
            deadline: self.deadline,
            owner,
            pre_interactions,
            post_interactions,
        }
    }
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    sell_token: eth::Address,
    buy_token: eth::Address,
    #[serde_as(as = "serde_ext::U256")]
    amount: eth::U256,
    kind: Kind,
    deadline: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Kind {
    Sell,
    Buy,
}

/// Order body for the POST /quote endpoint, which additionally allows
/// specifying pre/post interactions.
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostOrder {
    #[serde(flatten)]
    pub order: Order,
    #[serde(default)]
    pub from: eth::Address,
    #[serde(default)]
    pub interactions: Interactions,
}

impl PostOrder {
    pub fn into_domain(self) -> quote::Order {
        self.order.into_domain_with_interactions(
            self.from,
            self.interactions.pre.into_iter().map(Into::into).collect(),
            self.interactions.post.into_iter().map(Into::into).collect(),
        )
    }
}

#[serde_as]
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Interactions {
    #[serde(default)]
    pub pre: Vec<Interaction>,
    #[serde(default)]
    pub post: Vec<Interaction>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Interaction {
    pub target: eth::Address,
    #[serde_as(as = "serde_ext::U256")]
    pub value: eth::U256,
    #[serde_as(as = "serde_ext::Hex")]
    pub call_data: Vec<u8>,
}

impl From<Interaction> for domain::Interaction {
    fn from(i: Interaction) -> Self {
        Self {
            target: i.target,
            value: i.value.into(),
            call_data: i.call_data.into(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("received an order with identical buy and sell tokens")]
    SameTokens,
}
