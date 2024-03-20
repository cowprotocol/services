use {
    crate::{
        domain::{
            competition::{self, auction, order},
            eth,
            time,
        },
        infra::{solver::Timeouts, tokens, Ethereum},
        util::serialize,
    },
    serde::Deserialize,
    serde_with::serde_as,
};

impl Auction {
    pub async fn into_domain(
        self,
        eth: &Ethereum,
        tokens: &tokens::Fetcher,
        timeouts: Timeouts,
    ) -> Result<competition::Auction, Error> {
        let token_addresses: Vec<_> = self
            .tokens
            .iter()
            .map(|token| token.address.into())
            .collect();
        let token_infos = tokens.get(&token_addresses).await;

        competition::Auction::new(
            Some(self.id.try_into()?),
            self.orders
                .into_iter()
                .map(|order| competition::Order {
                    uid: order.uid.into(),
                    receiver: order.receiver.map(Into::into),
                    valid_to: order.valid_to.into(),
                    buy: eth::Asset {
                        amount: order.buy_amount.into(),
                        token: order.buy_token.into(),
                    },
                    sell: eth::Asset {
                        amount: order.sell_amount.into(),
                        token: order.sell_token.into(),
                    },
                    side: match order.kind {
                        Kind::Sell => competition::order::Side::Sell,
                        Kind::Buy => competition::order::Side::Buy,
                    },
                    user_fee: order.user_fee.into(),
                    kind: match order.class {
                        Class::Market => competition::order::Kind::Market,
                        Class::Limit => competition::order::Kind::Limit,
                        Class::Liquidity => competition::order::Kind::Liquidity,
                    },
                    app_data: order.app_data.into(),
                    partial: if order.partially_fillable {
                        competition::order::Partial::Yes {
                            available: match order.kind {
                                Kind::Sell => {
                                    order.sell_amount.saturating_sub(order.executed).into()
                                }
                                Kind::Buy => order.buy_amount.saturating_sub(order.executed).into(),
                            },
                        }
                    } else {
                        competition::order::Partial::No
                    },
                    pre_interactions: order
                        .pre_interactions
                        .into_iter()
                        .map(|interaction| eth::Interaction {
                            target: interaction.target.into(),
                            value: interaction.value.into(),
                            call_data: interaction.call_data.into(),
                        })
                        .collect(),
                    post_interactions: order
                        .post_interactions
                        .into_iter()
                        .map(|interaction| eth::Interaction {
                            target: interaction.target.into(),
                            value: interaction.value.into(),
                            call_data: interaction.call_data.into(),
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
                        BuyTokenBalance::Internal => competition::order::BuyTokenBalance::Internal,
                    },
                    signature: competition::order::Signature {
                        scheme: match order.signing_scheme {
                            SigningScheme::Eip712 => competition::order::signature::Scheme::Eip712,
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
                        data: order.signature.into(),
                        signer: order.owner.into(),
                    },
                    protocol_fees: order
                        .protocol_fees
                        .into_iter()
                        .map(|policy| match policy {
                            FeePolicy::Surplus {
                                factor,
                                max_volume_factor,
                            } => competition::order::FeePolicy::Surplus {
                                factor,
                                max_volume_factor,
                            },
                            FeePolicy::PriceImprovement {
                                factor,
                                max_volume_factor,
                                quote,
                            } => competition::order::FeePolicy::PriceImprovement {
                                factor,
                                max_volume_factor,
                                quote: quote.into_domain(order.sell_token, order.buy_token),
                            },
                            FeePolicy::Volume { factor } => {
                                competition::order::FeePolicy::Volume { factor }
                            }
                        })
                        .collect(),
                })
                .collect(),
            self.tokens.into_iter().map(|token| {
                let info = token_infos.get(&token.address.into());
                competition::auction::Token {
                    decimals: info.and_then(|i| i.decimals),
                    symbol: info.and_then(|i| i.symbol.clone()),
                    address: token.address.into(),
                    price: token.price.map(Into::into),
                    available_balance: info.map(|i| i.balance).unwrap_or(0.into()).into(),
                    trusted: token.trusted,
                }
            }),
            time::Deadline::new(self.deadline, timeouts),
            eth,
            self.score_cap.try_into().map_err(|_| Error::ZeroScoreCap)?,
        )
        .await
        .map_err(Into::into)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid auction ID")]
    InvalidAuctionId,
    #[error("surplus fee is missing for limit order")]
    MissingSurplusFee,
    #[error("invalid tokens in auction")]
    InvalidTokens,
    #[error("invalid order amounts in auction")]
    InvalidAmounts,
    #[error("zero score cap")]
    ZeroScoreCap,
    #[error("blockchain error: {0:?}")]
    Blockchain(#[source] crate::infra::blockchain::Error),
}

impl From<auction::InvalidId> for Error {
    fn from(_value: auction::InvalidId) -> Self {
        Self::InvalidAuctionId
    }
}

impl From<auction::Error> for Error {
    fn from(value: auction::Error) -> Self {
        match value {
            auction::Error::InvalidTokens => Self::InvalidTokens,
            auction::Error::InvalidAmounts => Self::InvalidAmounts,
            auction::Error::Blockchain(err) => Self::Blockchain(err),
        }
    }
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Auction {
    #[serde_as(as = "serde_with::DisplayFromStr")]
    id: i64,
    tokens: Vec<Token>,
    orders: Vec<Order>,
    deadline: chrono::DateTime<chrono::Utc>,
    #[serde_as(as = "serialize::U256")]
    score_cap: eth::U256,
}

impl Auction {
    pub fn id(&self) -> i64 {
        self.id
    }
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
    uid: [u8; order::UID_LEN],
    sell_token: eth::H160,
    buy_token: eth::H160,
    #[serde_as(as = "serialize::U256")]
    sell_amount: eth::U256,
    #[serde_as(as = "serialize::U256")]
    buy_amount: eth::U256,
    #[serde_as(as = "serialize::U256")]
    user_fee: eth::U256,
    protocol_fees: Vec<FeePolicy>,
    valid_to: u32,
    kind: Kind,
    receiver: Option<eth::H160>,
    owner: eth::H160,
    partially_fillable: bool,
    /// Always zero if the order is not partially fillable.
    #[serde_as(as = "serialize::U256")]
    executed: eth::U256,
    pre_interactions: Vec<Interaction>,
    post_interactions: Vec<Interaction>,
    #[serde(default)]
    sell_token_balance: SellTokenBalance,
    #[serde(default)]
    buy_token_balance: BuyTokenBalance,
    class: Class,
    #[serde_as(as = "serialize::Hex")]
    app_data: [u8; order::APP_DATA_LEN],
    signing_scheme: SigningScheme,
    #[serde_as(as = "serialize::Hex")]
    signature: Vec<u8>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
enum SellTokenBalance {
    #[default]
    Erc20,
    Internal,
    External,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
enum Class {
    Market,
    Limit,
    Liquidity,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
enum FeePolicy {
    #[serde(rename_all = "camelCase")]
    Surplus { factor: f64, max_volume_factor: f64 },
    #[serde(rename_all = "camelCase")]
    PriceImprovement {
        factor: f64,
        max_volume_factor: f64,
        quote: Quote,
    },
    #[serde(rename_all = "camelCase")]
    Volume { factor: f64 },
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Quote {
    #[serde_as(as = "serialize::U256")]
    pub sell_amount: eth::U256,
    #[serde_as(as = "serialize::U256")]
    pub buy_amount: eth::U256,
    #[serde_as(as = "serialize::U256")]
    pub fee: eth::U256,
}

impl Quote {
    fn into_domain(
        self,
        sell_token: eth::H160,
        buy_token: eth::H160,
    ) -> competition::order::fees::Quote {
        competition::order::fees::Quote {
            sell: eth::Asset {
                amount: self.sell_amount.into(),
                token: sell_token.into(),
            },
            buy: eth::Asset {
                amount: self.buy_amount.into(),
                token: buy_token.into(),
            },
            fee: eth::Asset {
                amount: self.fee.into(),
                token: sell_token.into(),
            },
        }
    }
}
