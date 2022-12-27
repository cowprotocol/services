use {
    crate::{
        domain::{competition, eth},
        util::serialize,
    },
    itertools::Itertools,
    primitive_types::{H160, U256},
    serde::Deserialize,
    serde_with::serde_as,
    std::{collections::HashMap, str::FromStr},
};

impl TryInto<competition::Auction> for Auction {
    type Error = Error;

    fn try_into(self) -> Result<competition::Auction, Self::Error> {
        Ok(competition::Auction {
            id: match self.id {
                Some(id) => Some(FromStr::from_str(&id).map_err(|_| Error::InvalidAuctionId)?),
                None => None,
            },
            tokens: self
                .tokens
                .into_iter()
                .map(|(address, token)| competition::auction::Token {
                    decimals: token.decimals,
                    symbol: token.symbol,
                    address: address.into(),
                    price: token.reference_price.map(Into::into),
                    available_balance: token.available_balance,
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
                            .interactions
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
                .try_collect()?,
            // TODO #899
            liquidity: Default::default(),
            gas_price: self.effective_gas_price.into(),
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
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Auction {
    id: Option<String>,
    tokens: HashMap<H160, Token>,
    orders: Vec<Order>,
    #[serde_as(as = "serialize::U256")]
    effective_gas_price: U256,
    deadline: chrono::DateTime<chrono::Utc>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Order {
    #[serde_as(as = "serialize::Hex")]
    uid: [u8; 56],
    sell_token: H160,
    buy_token: H160,
    #[serde_as(as = "serialize::U256")]
    sell_amount: U256,
    #[serde_as(as = "serialize::U256")]
    buy_amount: U256,
    #[serde_as(as = "serialize::U256")]
    solver_fee: U256,
    #[serde_as(as = "serialize::U256")]
    user_fee: U256,
    valid_to: u32,
    kind: Kind,
    receiver: Option<H160>,
    owner: H160,
    partially_fillable: bool,
    /// Always zero if the order is not partially fillable.
    #[serde_as(as = "serialize::U256")]
    executed: U256,
    interactions: Vec<Interaction>,
    sell_token_balance: SellTokenBalance,
    buy_token_balance: BuyTokenBalance,
    class: Class,
    #[serde_as(as = "Option<serialize::U256>")]
    surplus_fee: Option<U256>,
    #[serde_as(as = "serialize::Hex")]
    app_data: [u8; 32],
    reward: f64,
    signing_scheme: SigningScheme,
    #[serde_as(as = "serialize::Hex")]
    signature: Vec<u8>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Kind {
    Sell,
    Buy,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Interaction {
    target: H160,
    #[serde_as(as = "serialize::U256")]
    value: U256,
    #[serde_as(as = "serialize::Hex")]
    call_data: Vec<u8>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
enum SellTokenBalance {
    #[default]
    Erc20,
    Internal,
    External,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
enum BuyTokenBalance {
    #[default]
    Erc20,
    Internal,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum SigningScheme {
    Eip712,
    EthSign,
    PreSign,
    Eip1271,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Class {
    Market,
    Limit,
    Liquidity,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Token {
    decimals: Option<u8>,
    symbol: Option<String>,
    #[serde_as(as = "Option<serialize::U256>")]
    reference_price: Option<U256>,
    #[serde_as(as = "serialize::U256")]
    available_balance: U256,
    trusted: bool,
}
