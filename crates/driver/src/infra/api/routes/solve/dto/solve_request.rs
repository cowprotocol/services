use {
    crate::{
        domain::{
            self,
            competition::{
                self,
                auction,
                order::{
                    self,
                    app_data::{AppData, AppDataHash},
                },
            },
        },
        infra::{Ethereum, tokens},
    },
    eth_domain_types as eth,
    serde::Deserialize,
    serde_with::serde_as,
    std::{
        collections::{HashMap, HashSet},
        sync::Arc,
    },
    tracing::instrument,
};

impl SolveRequest {
    // Deprecated External/Internal arms are retained in case a new
    // order is placed between now and the PR blocking them going live.
    #[allow(deprecated)]
    #[instrument(skip_all)]
    pub async fn into_domain(
        self,
        eth: &Ethereum,
        tokens: &tokens::Fetcher,
        app_data: HashMap<Arc<AppDataHash>, Arc<app_data::ValidatedAppData>>,
    ) -> Result<competition::Auction, Error> {
        let token_addresses: Vec<_> = self
            .tokens
            .iter()
            .map(|token| token.address.into())
            .collect();
        let token_infos = tokens.get(&token_addresses).await;

        // register all tokens where internal buffer trading is allowed
        // for continuous balance monitoring
        tokens.keep_track_of_balances(
            self.tokens
                .iter()
                .filter_map(|t| t.trusted.then_some(&t.address)),
        );

        competition::Auction::new(
            auction::Kind::Competition(self.id.try_into()?),
            self.orders
                .into_iter()
                .map(|order| {
                    let app_data = app_data
                        .get(&AppDataHash::from(order.app_data))
                        .map(|data| AppData::Full(data.clone()));
                    order.into_domain(app_data)
                })
                .collect::<Result<Vec<_>, _>>()?,
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
            self.deadline,
            eth,
            self.surplus_capturing_jit_order_owners
                .into_iter()
                .collect::<HashSet<_>>(),
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
    #[error("protocol fee origins do not match protocol fees")]
    InvalidProtocolFeeOrigins,
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
            auction::Error::Blockchain(err) => Self::Blockchain(err),
        }
    }
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SolveRequest {
    #[serde_as(as = "serde_with::DisplayFromStr")]
    id: i64,
    tokens: Vec<Token>,
    orders: Vec<Order>,
    deadline: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    surplus_capturing_jit_order_owners: Vec<eth::Address>,
}

impl SolveRequest {
    pub fn id(&self) -> i64 {
        self.id
    }
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Token {
    pub address: eth::Address,
    #[serde_as(as = "Option<serde_ext::U256>")]
    pub price: Option<eth::U256>,
    pub trusted: bool,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Order {
    #[serde_as(as = "serde_ext::Hex")]
    uid: [u8; order::UID_LEN],
    sell_token: eth::Address,
    buy_token: eth::Address,
    #[serde_as(as = "serde_ext::U256")]
    sell_amount: eth::U256,
    #[serde_as(as = "serde_ext::U256")]
    buy_amount: eth::U256,
    protocol_fees: Vec<FeePolicy>,
    /// Origin of each entry in `protocol_fees`, matched by index. When absent,
    /// every fee is a protocol fee.
    #[serde(default)]
    protocol_fee_origins: Vec<Origin>,
    created: u32,
    valid_to: u32,
    kind: Kind,
    #[serde(default)]
    receiver: eth::Receiver,
    owner: eth::Address,
    partially_fillable: bool,
    /// Always zero if the order is not partially fillable.
    #[serde_as(as = "serde_ext::U256")]
    executed: eth::U256,
    pre_interactions: Vec<Interaction>,
    post_interactions: Vec<Interaction>,
    #[serde(default)]
    sell_token_balance: SellTokenBalance,
    #[serde(default)]
    buy_token_balance: BuyTokenBalance,
    /// Deprecated: every order is a limit order. The field is still accepted
    /// for backwards compatibility but ignored.
    #[serde(default)]
    #[allow(dead_code)]
    class: Class,
    #[serde_as(as = "serde_ext::Hex")]
    pub(crate) app_data: [u8; order::app_data::APP_DATA_LEN],
    signing_scheme: SigningScheme,
    #[serde_as(as = "serde_ext::Hex")]
    signature: Vec<u8>,
    quote: Option<Quote>,
    #[serde_as(as = "Option<serde_ext::U256>")]
    #[serde(default)]
    penalty_cap_native: Option<eth::U256>,
}

impl Order {
    #[expect(deprecated)]
    pub(crate) fn into_domain(
        self,
        app_data: Option<AppData>,
    ) -> Result<competition::Order, Error> {
        let app_data = app_data.unwrap_or_else(|| AppData::Hash(AppDataHash::from(self.app_data)));
        let origins = if self.protocol_fee_origins.is_empty() {
            vec![Origin::Protocol; self.protocol_fees.len()]
        } else if self.protocol_fee_origins.len() == self.protocol_fees.len() {
            self.protocol_fee_origins
        } else {
            return Err(Error::InvalidProtocolFeeOrigins);
        };
        let protocol_fees = origins
            .into_iter()
            .zip(self.protocol_fees)
            .map(|(origin, policy)| competition::order::ProtocolFee {
                origin: match origin {
                    Origin::Protocol => competition::order::Origin::Protocol,
                    Origin::Partner => competition::order::Origin::Partner,
                },
                policy: match policy {
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
                        quote: quote.into_domain(self.sell_token, self.buy_token),
                    },
                    FeePolicy::Volume { factor } => competition::order::FeePolicy::Volume {
                        factor,
                        contributes_to_score: true,
                    },
                },
            })
            .collect();
        let partial = if self.partially_fillable {
            competition::order::Partial::Yes {
                available: match self.kind {
                    Kind::Sell => self.sell_amount.saturating_sub(self.executed).into(),
                    Kind::Buy => self.buy_amount.saturating_sub(self.executed).into(),
                },
            }
        } else {
            competition::order::Partial::No
        };
        Ok(competition::Order {
            data: Arc::new(competition::order::OrderData {
                uid: self.uid.into(),
                receiver: self.receiver,
                created: self.created.into(),
                valid_to: self.valid_to.into(),
                buy: eth::Asset {
                    amount: self.buy_amount.into(),
                    token: self.buy_token.into(),
                },
                sell: eth::Asset {
                    amount: self.sell_amount.into(),
                    token: self.sell_token.into(),
                },
                side: match self.kind {
                    Kind::Sell => competition::order::Side::Sell,
                    Kind::Buy => competition::order::Side::Buy,
                },
                pre_interactions: self
                    .pre_interactions
                    .into_iter()
                    .map(|interaction| domain::Interaction {
                        target: interaction.target,
                        value: interaction.value.into(),
                        call_data: interaction.call_data.into(),
                    })
                    .collect(),
                post_interactions: self
                    .post_interactions
                    .into_iter()
                    .map(|interaction| domain::Interaction {
                        target: interaction.target,
                        value: interaction.value.into(),
                        call_data: interaction.call_data.into(),
                    })
                    .collect(),
                sell_token_balance: match self.sell_token_balance {
                    SellTokenBalance::Erc20 => competition::order::SellTokenBalance::Erc20,
                    SellTokenBalance::Internal => competition::order::SellTokenBalance::Internal,
                    SellTokenBalance::External => competition::order::SellTokenBalance::External,
                },
                buy_token_balance: match self.buy_token_balance {
                    BuyTokenBalance::Erc20 => competition::order::BuyTokenBalance::Erc20,
                    BuyTokenBalance::Internal => competition::order::BuyTokenBalance::Internal,
                },
                signature: competition::order::Signature {
                    scheme: match self.signing_scheme {
                        SigningScheme::Eip712 => competition::order::signature::Scheme::Eip712,
                        SigningScheme::EthSign => competition::order::signature::Scheme::EthSign,
                        SigningScheme::PreSign => competition::order::signature::Scheme::PreSign,
                        SigningScheme::Eip1271 => competition::order::signature::Scheme::Eip1271,
                    },
                    data: self.signature.into(),
                    signer: self.owner,
                },
                protocol_fees,
                quote: self
                    .quote
                    .map(|q| q.into_domain(self.sell_token, self.buy_token)),
                penalty_cap_native: self.penalty_cap_native.map(Into::into),
            }),
            app_data,
            partial,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Kind {
    Sell,
    Buy,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Interaction {
    target: eth::Address,
    #[serde_as(as = "serde_ext::U256")]
    value: eth::U256,
    #[serde_as(as = "serde_ext::Hex")]
    call_data: Vec<u8>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
enum SellTokenBalance {
    #[default]
    Erc20,
    #[deprecated(
        note = "Balancer Vault token sources are deprecated and no longer appear in auctions; \
                only erc20 is used"
    )]
    Internal,
    #[deprecated(
        note = "Balancer Vault token sources are deprecated and no longer appear in auctions; \
                only erc20 is used"
    )]
    External,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
enum BuyTokenBalance {
    #[default]
    Erc20,
    #[deprecated(
        note = "Balancer Vault token sources are deprecated and no longer appear in auctions; \
                only erc20 is used"
    )]
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

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Class {
    Market,
    #[default]
    Limit,
}

/// Who a protocol fee benefits. The solver's own fee is not part of the API.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Origin {
    Protocol,
    Partner,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
pub struct Quote {
    #[serde_as(as = "serde_ext::U256")]
    pub sell_amount: eth::U256,
    #[serde_as(as = "serde_ext::U256")]
    pub buy_amount: eth::U256,
    #[serde_as(as = "serde_ext::U256")]
    pub fee: eth::U256,
    pub solver: eth::Address,
}

impl Quote {
    fn into_domain(
        self,
        sell_token: eth::Address,
        buy_token: eth::Address,
    ) -> competition::order::Quote {
        competition::order::Quote {
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
            solver: self.solver,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order_json(extra: serde_json::Value) -> serde_json::Value {
        let mut json = serde_json::json!({
            "uid": format!("0x{}", "11".repeat(56)),
            "sellToken": format!("0x{}", "aa".repeat(20)),
            "buyToken": format!("0x{}", "bb".repeat(20)),
            "sellAmount": "1000",
            "buyAmount": "900",
            "created": 0,
            "validTo": 100,
            "kind": "sell",
            "owner": format!("0x{}", "cc".repeat(20)),
            "partiallyFillable": false,
            "executed": "0",
            "preInteractions": [],
            "postInteractions": [],
            "appData": format!("0x{}", "00".repeat(32)),
            "signingScheme": "eip712",
            "signature": "0x",
            "quote": null,
            "protocolFees": [
                {"volume": {"factor": 0.1}},
                {"volume": {"factor": 0.2}},
            ],
        });
        json.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        json
    }

    #[test]
    fn protocol_fees_without_origins_are_all_protocol() {
        let order: Order = serde_json::from_value(order_json(serde_json::json!({}))).unwrap();
        let order = order.into_domain(None).unwrap();
        let fees = &order.data.protocol_fees;
        assert_eq!(fees.len(), 2);
        assert!(
            fees.iter()
                .all(|fee| matches!(fee.origin, competition::order::Origin::Protocol))
        );
    }

    #[test]
    fn protocol_fee_origins_match_protocol_fees_by_index() {
        let json = order_json(serde_json::json!({
            "protocolFeeOrigins": ["protocol", "partner"],
        }));
        let order: Order = serde_json::from_value(json).unwrap();
        let order = order.into_domain(None).unwrap();
        let fees = &order.data.protocol_fees;
        assert!(matches!(
            fees[0].origin,
            competition::order::Origin::Protocol
        ));
        assert!(matches!(
            fees[1].origin,
            competition::order::Origin::Partner
        ));
    }

    #[test]
    fn protocol_fee_origins_must_match_protocol_fees_length() {
        let json = order_json(serde_json::json!({
            "protocolFeeOrigins": ["partner"],
        }));
        let order: Order = serde_json::from_value(json).unwrap();
        assert!(order.into_domain(None).is_err());
    }

    #[test]
    fn solver_is_not_a_wire_origin() {
        let json = order_json(serde_json::json!({
            "protocolFeeOrigins": ["protocol", "solver"],
        }));
        assert!(serde_json::from_value::<Order>(json).is_err());
    }
}
