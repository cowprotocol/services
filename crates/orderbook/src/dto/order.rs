use {
    model::{
        app_data::AppDataHash,
        interaction::InteractionData,
        order::{BuyTokenDestination, OrderClass, OrderKind, OrderUid, SellTokenSource},
        signature::Signature,
    },
    primitive_types::H160,
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    pub uid: OrderUid,
    pub sell_token: H160,
    pub buy_token: H160,
    pub sell_amount: number::U256,
    pub buy_amount: number::U256,
    pub user_fee: number::U256,
    pub protocol_fees: Vec<FeePolicy>,
    pub valid_to: u32,
    pub kind: OrderKind,
    pub receiver: Option<H160>,
    pub owner: H160,
    pub partially_fillable: bool,
    pub executed: number::U256,
    pub pre_interactions: Vec<InteractionData>,
    pub post_interactions: Vec<InteractionData>,
    pub sell_token_balance: SellTokenSource,
    pub buy_token_balance: BuyTokenDestination,
    #[serde(flatten)]
    pub class: OrderClass,
    pub app_data: AppDataHash,
    #[serde(flatten)]
    pub signature: Signature,
}

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FeePolicy {
    #[serde(rename_all = "camelCase")]
    Surplus { factor: f64, max_volume_factor: f64 },
    #[serde(rename_all = "camelCase")]
    Volume { factor: f64 },
}
