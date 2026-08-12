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
            Some(self.id.try_into()?),
            self.orders
                .into_iter()
                .map(|order| {
                    let app_data = app_data
                        .get(&AppDataHash::from(order.app_data))
                        .map(|data| AppData::Full(data.clone()));
                    order.into_domain(app_data)
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

/// The two kinds of `/solve` request bodies, distinguished by an internal
/// `kind` tag: the full auction or - if the driver opted in - only the
/// difference to the previously received auction. Bodies without the tag are
/// full auctions since they predate delta requests.
#[derive(Debug)]
pub enum SolveRequestBody {
    Full(SolveRequest),
    Delta(SolveRequestDelta),
}

/// Parses a `/solve` request body.
///
/// Bodies are parsed as full auctions first so the overwhelmingly common
/// multi-MB full bodies are only scanned once; only when that fails is the
/// `kind` tag probed to dispatch to another request kind. An internally
/// tagged serde enum can't be used because the tag may be absent (backwards
/// compatibility) and because serde buffers the entire document to find the
/// tag. Note that this requires any future request kind to not also parse
/// as a valid full auction (deltas don't: they have no `orders` field).
pub fn parse(body: &[u8]) -> serde_json::Result<SolveRequestBody> {
    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    enum RequestKind {
        Delta,
    }
    #[derive(Debug, Deserialize)]
    struct Probe {
        #[serde(default)]
        kind: Option<RequestKind>,
    }
    let full_err = match serde_json::from_slice(body) {
        Ok(request) => return Ok(SolveRequestBody::Full(request)),
        Err(err) => err,
    };
    match serde_json::from_slice::<Probe>(body).map(|probe| probe.kind) {
        Ok(Some(RequestKind::Delta)) => Ok(SolveRequestBody::Delta(serde_json::from_slice(body)?)),
        // The body is not of another known request kind, so report why it
        // doesn't parse as a full auction.
        _ => Err(full_err),
    }
}

/// Difference of an auction relative to the auction `base_id` which this
/// driver received earlier. Only orders are diffed; tokens and all scalar
/// fields are sent whole.
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SolveRequestDelta {
    #[serde_as(as = "serde_with::DisplayFromStr")]
    id: i64,
    /// Id of the auction this delta applies to.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    base_id: i64,
    tokens: Vec<Token>,
    /// Orders that were added or modified since the base auction.
    updated_orders: Vec<Order>,
    /// Uids of orders that were removed since the base auction.
    #[serde_as(as = "Vec<serde_ext::Hex>")]
    removed_orders: Vec<[u8; order::UID_LEN]>,
    deadline: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    surplus_capturing_jit_order_owners: Vec<eth::Address>,
}

impl SolveRequestDelta {
    /// Reconstructs the full auction by applying this delta to the base
    /// auction.
    pub fn apply(self, base: Option<&DeltaBase>) -> Result<SolveRequest, DeltaBaseMismatch> {
        let base =
            base.filter(|base| base.auction_id == self.base_id)
                .ok_or(DeltaBaseMismatch {
                    expected: self.base_id,
                    actual: base.map(|base| base.auction_id),
                })?;

        let base_uids: HashSet<_> = base.orders.iter().map(|order| order.uid).collect();
        let (changed, added): (Vec<_>, Vec<_>) = self
            .updated_orders
            .into_iter()
            .partition(|order| base_uids.contains(&order.uid));
        let mut changed: HashMap<_, _> = changed
            .into_iter()
            .map(|order| (order.uid, order))
            .collect();
        let removed: HashSet<_> = self.removed_orders.into_iter().collect();

        let mut orders = Vec::with_capacity(base.orders.len() + added.len());
        for order in &base.orders {
            if removed.contains(&order.uid) {
                continue;
            }
            orders.push(match changed.remove(&order.uid) {
                Some(updated) => updated,
                None => order.clone(),
            });
        }
        orders.extend(added);

        Ok(SolveRequest {
            id: self.id,
            tokens: self.tokens,
            orders,
            deadline: self.deadline,
            surplus_capturing_jit_order_owners: self.surplus_capturing_jit_order_owners,
        })
    }
}

/// Orders of the most recently received auction; the base which delta
/// requests are applied to.
#[derive(Debug)]
pub struct DeltaBase {
    auction_id: i64,
    orders: Vec<Order>,
}

impl DeltaBase {
    pub fn snapshot(request: &SolveRequest) -> Self {
        Self {
            auction_id: request.id,
            orders: request.orders.clone(),
        }
    }

    pub fn auction_id(&self) -> i64 {
        self.auction_id
    }
}

/// A delta request referenced a base auction this driver doesn't have.
/// Reported as HTTP 409 so the autopilot re-sends the full auction.
#[derive(Debug, thiserror::Error)]
#[error("delta request expects base auction {expected} but the driver has {actual:?}")]
pub struct DeltaBaseMismatch {
    expected: i64,
    actual: Option<i64>,
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
#[derive(Clone, Debug, Deserialize)]
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
    created: u32,
    valid_to: u32,
    kind: Kind,
    receiver: Option<eth::Address>,
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
    class: Class,
    #[serde_as(as = "serde_ext::Hex")]
    pub(crate) app_data: [u8; order::app_data::APP_DATA_LEN],
    signing_scheme: SigningScheme,
    #[serde_as(as = "serde_ext::Hex")]
    signature: Vec<u8>,
    quote: Option<Quote>,
}

impl Order {
    #[expect(deprecated)]
    pub(crate) fn into_domain(self, app_data: Option<AppData>) -> competition::Order {
        let app_data = app_data.unwrap_or_else(|| AppData::Hash(AppDataHash::from(self.app_data)));
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
        competition::Order {
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
                kind: match self.class {
                    Class::Market => competition::order::Kind::Market,
                    Class::Limit => competition::order::Kind::Limit,
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
                protocol_fees: self
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
                            quote: quote.into_domain(self.sell_token, self.buy_token),
                        },
                        FeePolicy::Volume { factor } => {
                            competition::order::FeePolicy::Volume { factor }
                        }
                    })
                    .collect(),
                quote: self
                    .quote
                    .map(|q| q.into_domain(self.sell_token, self.buy_token)),
            }),
            app_data,
            partial,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Kind {
    Sell,
    Buy,
}

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Interaction {
    target: eth::Address,
    #[serde_as(as = "serde_ext::U256")]
    value: eth::U256,
    #[serde_as(as = "serde_ext::Hex")]
    call_data: Vec<u8>,
}

#[derive(Clone, Debug, Default, Deserialize)]
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

#[derive(Clone, Debug, Default, Deserialize)]
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

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum SigningScheme {
    Eip712,
    EthSign,
    PreSign,
    Eip1271,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Class {
    Market,
    Limit,
}

#[derive(Clone, Debug, Deserialize)]
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
#[derive(Clone, Debug, Deserialize)]
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

    /// JSON of a minimal order as the autopilot serializes it, with all
    /// bytes of the uid set to `uid_byte`.
    fn order_json(uid_byte: u8, executed: u64) -> serde_json::Value {
        serde_json::json!({
            "uid": format!("0x{}", format!("{uid_byte:02x}").repeat(56)),
            "sellToken": "0x2222222222222222222222222222222222222222",
            "buyToken": "0x3333333333333333333333333333333333333333",
            "sellAmount": "1000",
            "buyAmount": "2000",
            "protocolFees": [],
            "created": 1,
            "validTo": 2,
            "kind": "sell",
            "receiver": null,
            "owner": "0x4444444444444444444444444444444444444444",
            "partiallyFillable": true,
            "executed": executed.to_string(),
            "preInteractions": [],
            "postInteractions": [],
            "sellTokenBalance": "erc20",
            "buyTokenBalance": "erc20",
            "class": "limit",
            "appData": format!("0x{}", "00".repeat(32)),
            "signingScheme": "eip712",
            "signature": format!("0x{}1c", "01".repeat(64)),
            "quote": null,
        })
    }

    fn full_request_json() -> serde_json::Value {
        serde_json::json!({
            "id": "1",
            "tokens": [{
                "address": "0x5555555555555555555555555555555555555555",
                "price": "1000000000000000000",
                "trusted": true,
            }],
            "orders": [order_json(0x11, 0), order_json(0x22, 0), order_json(0x33, 0)],
            "deadline": "2023-11-14T22:13:20Z",
            "surplusCapturingJitOrderOwners": [],
        })
    }

    /// The shape of this request is pinned by the autopilot's tests
    /// (crates/autopilot/src/infra/solvers/dto/solve.rs); both sides must
    /// agree on the wire format.
    fn delta_request_json() -> serde_json::Value {
        serde_json::json!({
            "kind": "delta",
            "id": "2",
            "baseId": "1",
            "tokens": [{
                "address": "0x6666666666666666666666666666666666666666",
                "price": "2000000000000000000",
                "trusted": false,
            }],
            "updatedOrders": [order_json(0x22, 100), order_json(0x44, 0)],
            "removedOrders": [format!("0x{}", "33".repeat(56))],
            "deadline": "2023-11-14T22:15:44Z",
            "surplusCapturingJitOrderOwners": [],
        })
    }

    fn parse_value(value: serde_json::Value) -> serde_json::Result<SolveRequestBody> {
        parse(&serde_json::to_vec(&value).unwrap())
    }

    #[test]
    fn parses_full_request_without_kind_tag() {
        let SolveRequestBody::Full(request) = parse_value(full_request_json()).unwrap() else {
            panic!("expected full request");
        };
        assert_eq!(request.id, 1);
        assert_eq!(request.orders.len(), 3);
    }

    #[test]
    fn parses_full_request_with_kind_tag() {
        let mut json = full_request_json();
        json["kind"] = "full".into();
        let SolveRequestBody::Full(request) = parse_value(json).unwrap() else {
            panic!("expected full request");
        };
        assert_eq!(request.id, 1);
    }

    #[test]
    fn rejects_unknown_kind_tag() {
        // A future request kind (not parseable as a full auction) must fail
        // loudly instead of being misinterpreted.
        let mut json = delta_request_json();
        json["kind"] = "somethingElse".into();
        assert!(parse_value(json).is_err());
    }

    #[test]
    fn applies_delta_to_base() {
        let SolveRequestBody::Full(full) = parse_value(full_request_json()).unwrap() else {
            panic!("expected full request");
        };
        let base = DeltaBase::snapshot(&full);

        let SolveRequestBody::Delta(delta) = parse_value(delta_request_json()).unwrap() else {
            panic!("expected delta request");
        };
        let request = delta.apply(Some(&base)).unwrap();

        assert_eq!(request.id, 2);
        // Modified orders are replaced in place, removed orders dropped and
        // added orders appended.
        let uids: Vec<u8> = request.orders.iter().map(|order| order.uid[0]).collect();
        assert_eq!(uids, vec![0x11, 0x22, 0x44]);
        assert_eq!(request.orders[1].executed, eth::U256::from(100));
        // Everything that is not an order is taken from the delta.
        assert_eq!(request.tokens.len(), 1);
        assert_eq!(
            request.tokens[0].address,
            "0x6666666666666666666666666666666666666666"
                .parse::<eth::Address>()
                .unwrap()
        );
        assert_eq!(
            request.deadline,
            "2023-11-14T22:15:44Z"
                .parse::<chrono::DateTime<chrono::Utc>>()
                .unwrap()
        );
    }

    #[test]
    fn rejects_delta_on_base_mismatch() {
        let SolveRequestBody::Full(full) = parse_value(full_request_json()).unwrap() else {
            panic!("expected full request");
        };
        let mut json = delta_request_json();
        json["baseId"] = "7".into();
        let SolveRequestBody::Delta(delta) = parse_value(json).unwrap() else {
            panic!("expected delta request");
        };
        assert!(delta.apply(Some(&DeltaBase::snapshot(&full))).is_err());
    }

    #[test]
    fn rejects_delta_without_base() {
        let SolveRequestBody::Delta(delta) = parse_value(delta_request_json()).unwrap() else {
            panic!("expected delta request");
        };
        assert!(delta.apply(None).is_err());
    }
}
