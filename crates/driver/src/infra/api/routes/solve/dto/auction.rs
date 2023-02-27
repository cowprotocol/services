use {
    crate::{
        domain::{competition, eth},
        infra::Ethereum,
        util::serialize,
    },
    itertools::Itertools,
    serde::Deserialize,
    serde_with::serde_as,
};

impl Auction {
    pub async fn into_domain(self, eth: &Ethereum) -> Result<competition::Auction, Error> {
        Ok(competition::Auction {
            id: Some((self.id as u64).into()),
            tokens: self
                .tokens
                .into_iter()
                .map(|token| competition::auction::Token {
                    decimals: None,
                    symbol: None,
                    address: token.address.into(),
                    price: token.price.map(Into::into),
                    available_balance: Default::default(),
                    trusted: token.trusted,
                })
                .collect(),
            orders: self
                .orders
                .into_iter()
                .map(|order| {
                    Ok(competition::Order {
                        uid: order.uid.into(),
                        receiver: order.receiver.map(Into::into),
                        valid_to: order.valid_to.into(),
                        sell: eth::Asset {
                            amount: order.sell_amount,
                            token: order.sell_token.into(),
                        },
                        buy: eth::Asset {
                            amount: order.buy_amount,
                            token: order.buy_token.into(),
                        },
                        side: match order.kind {
                            Kind::Sell => competition::order::Side::Sell,
                            Kind::Buy => competition::order::Side::Buy,
                        },
                        fee: competition::order::Fee {
                            user: order.user_fee.into(),
                            solver: order.solver_fee.into(),
                        },
                        kind: match order.class {
                            Class::Market => competition::order::Kind::Market,
                            Class::Limit => competition::order::Kind::Limit {
                                surplus_fee: order
                                    .surplus_fee
                                    .ok_or(Error::MissingSurplusFee)?
                                    .into(),
                            },
                            Class::Liquidity => competition::order::Kind::Liquidity,
                        },
                        app_data: order.app_data.into(),
                        partial: if order.partially_fillable {
                            competition::order::Partial::Yes {
                                executed: order.executed.into(),
                            }
                        } else {
                            competition::order::Partial::No
                        },
                        interactions: order
                            .pre_interactions
                            .into_iter()
                            .map(|interaction| eth::Interaction {
                                target: interaction.target.into(),
                                value: interaction.value.into(),
                                call_data: interaction.call_data,
                            })
                            .collect(),
                        sell_token_balance: match order.sell_token_balance {
                            SellTokenBalance::Erc20 => competition::order::SellTokenBalance::Erc20,
                            SellTokenBalance::Internal => {
                                competition::order::SellTokenBalance::Internal
                            }
                            SellTokenBalance::External => {
                                competition::order::SellTokenBalance::External
                            }
                        },
                        buy_token_balance: match order.buy_token_balance {
                            BuyTokenBalance::Erc20 => competition::order::BuyTokenBalance::Erc20,
                            BuyTokenBalance::Internal => {
                                competition::order::BuyTokenBalance::Internal
                            }
                        },
                        signature: competition::order::Signature {
                            scheme: match order.signing_scheme {
                                SigningScheme::Eip712 => {
                                    competition::order::signature::Scheme::Eip712
                                }
                                SigningScheme::EthSign => {
                                    competition::order::signature::Scheme::EthSign
                                }
                                SigningScheme::PreSign => {
                                    competition::order::signature::Scheme::PreSign
                                }
                                SigningScheme::Eip1271 => {
                                    competition::order::signature::Scheme::Eip1271
                                }
                            },
                            data: order.signature,
                            signer: order.owner.into(),
                        },
                        reward: order.reward,
                    })
                })
                .try_collect::<_, _, Error>()?,
            gas_price: eth.gas_price().await.map_err(Error::GasPrice)?,
            deadline: self.deadline.into(),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid auction ID")]
    InvalidAuctionId,
    #[error("surplus fee is missing for limit order")]
    MissingSurplusFee,
    #[error("error getting gas price")]
    GasPrice(#[source] crate::infra::blockchain::Error),
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Auction {
    id: i64,
    tokens: Vec<Token>,
    orders: Vec<Order>,
    deadline: chrono::DateTime<chrono::Utc>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Token {
    pub address: eth::H160,
    #[serde_as(as = "Option<serialize::U256>")]
    pub price: Option<eth::U256>,
    pub trusted: bool,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Order {
    #[serde_as(as = "serialize::Hex")]
    uid: [u8; 56],
    sell_token: eth::H160,
    buy_token: eth::H160,
    #[serde_as(as = "serialize::U256")]
    sell_amount: eth::U256,
    #[serde_as(as = "serialize::U256")]
    buy_amount: eth::U256,
    #[serde_as(as = "serialize::U256")]
    solver_fee: eth::U256,
    #[serde_as(as = "serialize::U256")]
    user_fee: eth::U256,
    valid_to: u32,
    kind: Kind,
    receiver: Option<eth::H160>,
    owner: eth::H160,
    partially_fillable: bool,
    /// Always zero if the order is not partially fillable.
    #[serde_as(as = "serialize::U256")]
    executed: eth::U256,
    pre_interactions: Vec<Interaction>,
    #[serde(default)]
    sell_token_balance: SellTokenBalance,
    #[serde(default)]
    buy_token_balance: BuyTokenBalance,
    class: Class,
    #[serde_as(as = "Option<serialize::U256>")]
    surplus_fee: Option<eth::U256>,
    #[serde_as(as = "serialize::Hex")]
    app_data: [u8; 32],
    reward: f64,
    signing_scheme: SigningScheme,
    #[serde_as(as = "serialize::Hex")]
    signature: Vec<u8>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase", deny_unknown_fields)]
enum Kind {
    Sell,
    Buy,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Interaction {
    target: eth::H160,
    #[serde_as(as = "serialize::U256")]
    value: eth::U256,
    #[serde_as(as = "serialize::Hex")]
    call_data: Vec<u8>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase", deny_unknown_fields)]
enum SellTokenBalance {
    #[default]
    Erc20,
    Internal,
    External,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase", deny_unknown_fields)]
enum BuyTokenBalance {
    #[default]
    Erc20,
    Internal,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase", deny_unknown_fields)]
enum SigningScheme {
    Eip712,
    EthSign,
    PreSign,
    Eip1271,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase", deny_unknown_fields)]
enum Class {
    Market,
    Limit,
    Liquidity,
}
