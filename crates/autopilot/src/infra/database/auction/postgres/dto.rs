use {
    crate::{
        boundary::{self, OrderUid},
        domain,
    },
    model::bytes_hex::BytesHex,
    number::serialization::HexOrDecimalU256,
    primitive_types::{H160, U256},
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
    std::collections::BTreeMap,
};

pub fn from_domain(auction: domain::Auction) -> Auction {
    Auction {
        block: auction.block,
        latest_settlement_block: auction.latest_settlement_block,
        orders: auction.orders.into_iter().map(Into::into).collect(),
        prices: auction.prices,
    }
}

pub fn to_domain(auction: Auction) -> domain::Auction {
    domain::Auction {
        block: auction.block,
        latest_settlement_block: auction.latest_settlement_block,
        orders: auction.orders.into_iter().map(Into::into).collect(),
        prices: auction.prices,
    }
}

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Auction {
    /// The block that this auction is valid for.
    /// The block number for the auction. Orders and prices are guaranteed to be
    /// valid on this block.
    pub block: u64,

    /// The latest block on which a settlement has been processed. This field is
    /// used to tell which orders are still in-flight. See
    /// [`InFlightOrders`].
    ///
    /// Note that under certain conditions it is possible for a settlement to
    /// have been mined as part of [`block`] but not have yet been processed.
    pub latest_settlement_block: u64,

    /// The solvable orders included in the auction.
    pub orders: Vec<Order>,

    /// The reference prices for all traded tokens in the auction.
    pub prices: BTreeMap<H160, U256>,
}

pub type AuctionId = i64;

#[serde_as]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuctionWithId {
    /// Increments whenever the backend updates the auction.
    pub id: AuctionId,
    #[serde(flatten)]
    pub auction: Auction,
}

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    pub uid: OrderUid,
    pub sell_token: H160,
    pub buy_token: H160,
    #[serde_as(as = "HexOrDecimalU256")]
    pub sell_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub buy_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub solver_fee: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub user_fee: U256,
    pub valid_to: u32,
    pub kind: OrderKind,
    pub receiver: Option<H160>,
    pub owner: H160,
    pub partially_fillable: bool,
    #[serde_as(as = "HexOrDecimalU256")]
    pub executed: U256,
    pub pre_interactions: Vec<Interaction>,
    pub post_interactions: Vec<Interaction>,
    pub sell_token_balance: boundary::SellTokenSource,
    pub buy_token_balance: boundary::BuyTokenDestination,
    pub class: Class,
    pub app_data: boundary::AppDataHash,
    #[serde(flatten)]
    pub signature: boundary::Signature,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eth_flow: Option<boundary::EthflowData>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub onchain_order: Option<boundary::OnchainOrderData>,
    /// The types of fees that will be collected by the protocol.
    /// Multiple fees are applied in the order they are listed
    pub fee_policies: Vec<FeePolicy>,
}

impl From<domain::Order> for Order {
    fn from(order: domain::Order) -> Self {
        Self {
            uid: order.uid,
            sell_token: order.sell_token,
            buy_token: order.buy_token,
            sell_amount: order.sell_amount,
            buy_amount: order.buy_amount,
            solver_fee: order.solver_fee,
            user_fee: order.user_fee,
            valid_to: order.valid_to,
            kind: order.kind.into(),
            receiver: order.receiver,
            owner: order.owner,
            partially_fillable: order.partially_fillable,
            executed: order.executed,
            pre_interactions: order.pre_interactions.into_iter().map(Into::into).collect(),
            post_interactions: order
                .post_interactions
                .into_iter()
                .map(Into::into)
                .collect(),
            sell_token_balance: order.sell_token_balance,
            buy_token_balance: order.buy_token_balance,
            class: order.class.into(),
            app_data: order.app_data,
            signature: order.signature,
            eth_flow: order.eth_flow,
            onchain_order: order.onchain_order,
            fee_policies: order.fee_policies.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<Order> for domain::Order {
    fn from(order: Order) -> Self {
        Self {
            uid: order.uid,
            sell_token: order.sell_token,
            buy_token: order.buy_token,
            sell_amount: order.sell_amount,
            buy_amount: order.buy_amount,
            solver_fee: order.solver_fee,
            user_fee: order.user_fee,
            valid_to: order.valid_to,
            kind: order.kind.into(),
            receiver: order.receiver,
            owner: order.owner,
            partially_fillable: order.partially_fillable,
            executed: order.executed,
            pre_interactions: order.pre_interactions.into_iter().map(Into::into).collect(),
            post_interactions: order
                .post_interactions
                .into_iter()
                .map(Into::into)
                .collect(),
            sell_token_balance: order.sell_token_balance,
            buy_token_balance: order.buy_token_balance,
            class: order.class.into(),
            app_data: order.app_data,
            signature: order.signature,
            eth_flow: order.eth_flow,
            onchain_order: order.onchain_order,
            fee_policies: order.fee_policies.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OrderKind {
    Buy,
    Sell,
}

impl From<domain::auction::order::OrderKind> for OrderKind {
    fn from(kind: domain::auction::order::OrderKind) -> Self {
        match kind {
            domain::auction::order::OrderKind::Buy => Self::Buy,
            domain::auction::order::OrderKind::Sell => Self::Sell,
        }
    }
}

impl From<OrderKind> for domain::auction::order::OrderKind {
    fn from(kind: OrderKind) -> Self {
        match kind {
            OrderKind::Buy => Self::Buy,
            OrderKind::Sell => Self::Sell,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Class {
    Limit,
    Market,
    Liquidity,
}

impl From<domain::auction::order::Class> for Class {
    fn from(class: domain::auction::order::Class) -> Self {
        match class {
            domain::auction::order::Class::Limit => Self::Limit,
            domain::auction::order::Class::Market => Self::Market,
            domain::auction::order::Class::Liquidity => Self::Liquidity,
        }
    }
}

impl From<Class> for domain::auction::order::Class {
    fn from(class: Class) -> Self {
        match class {
            Class::Limit => Self::Limit,
            Class::Market => Self::Market,
            Class::Liquidity => Self::Liquidity,
        }
    }
}

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Interaction {
    pub target: H160,
    #[serde_as(as = "HexOrDecimalU256")]
    pub value: U256,
    #[serde_as(as = "BytesHex")]
    pub call_data: Vec<u8>,
}

impl From<domain::auction::order::Interaction> for Interaction {
    fn from(interaction: domain::auction::order::Interaction) -> Self {
        Self {
            target: interaction.target,
            value: interaction.value,
            call_data: interaction.call_data,
        }
    }
}

impl From<Interaction> for domain::auction::order::Interaction {
    fn from(interaction: Interaction) -> Self {
        Self {
            target: interaction.target,
            value: interaction.value,
            call_data: interaction.call_data,
        }
    }
}

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FeePolicy {
    #[serde(rename_all = "camelCase")]
    PriceImprovement { factor: f64, max_volume_factor: f64 },
    #[serde(rename_all = "camelCase")]
    Volume { factor: f64 },
}

impl From<domain::fee::Policy> for FeePolicy {
    fn from(policy: domain::fee::Policy) -> Self {
        match policy {
            domain::fee::Policy::PriceImprovement {
                factor,
                max_volume_factor,
            } => Self::PriceImprovement {
                factor,
                max_volume_factor,
            },
            domain::fee::Policy::Volume { factor } => Self::Volume { factor },
        }
    }
}

impl From<FeePolicy> for domain::fee::Policy {
    fn from(policy: FeePolicy) -> Self {
        match policy {
            FeePolicy::PriceImprovement {
                factor,
                max_volume_factor,
            } => Self::PriceImprovement {
                factor,
                max_volume_factor,
            },
            FeePolicy::Volume { factor } => Self::Volume { factor },
        }
    }
}
