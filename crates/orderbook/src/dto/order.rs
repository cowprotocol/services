use {
    app_data::AppDataHash,
    model::{
        interaction::InteractionData,
        order::{BuyTokenDestination, OrderClass, OrderKind, OrderUid, SellTokenSource},
        signature::Signature,
    },
    number::serialization::HexOrDecimalU256,
    primitive_types::{H160, U256},
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
};

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
    pub protocol_fees: Vec<FeePolicy>,
    pub created: u32,
    pub valid_to: u32,
    pub kind: OrderKind,
    pub receiver: Option<H160>,
    pub owner: H160,
    pub partially_fillable: bool,
    #[serde_as(as = "HexOrDecimalU256")]
    pub executed: U256,
    pub pre_interactions: Vec<InteractionData>,
    pub post_interactions: Vec<InteractionData>,
    pub sell_token_balance: SellTokenSource,
    pub buy_token_balance: BuyTokenDestination,
    #[serde(flatten)]
    pub class: OrderClass,
    pub app_data: AppDataHash,
    #[serde(flatten)]
    pub signature: Signature,
    pub quote: Quote,
}

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    #[serde_as(as = "HexOrDecimalU256")]
    pub sell_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub buy_amount: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub fee: U256,
    pub solver: H160,
}

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FeePolicy {
    #[serde(rename_all = "camelCase")]
    Surplus { factor: f64, max_volume_factor: f64 },
    #[serde(rename_all = "camelCase")]
    Volume { factor: f64 },
    #[serde(rename_all = "camelCase")]
    PriceImprovement {
        factor: f64,
        max_volume_factor: f64,
        quote: Quote,
    },
}

#[serde_as]
#[derive(Serialize, PartialEq, Debug, Clone)]
#[cfg_attr(any(test, feature = "e2e"), derive(serde::Deserialize))]
#[serde(rename_all = "camelCase")]
pub struct ExecutedAmounts {
    #[serde_as(as = "HexOrDecimalU256")]
    pub sell: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub buy: U256,
}

/// Indicates that a solver has provided a solution, with `executed_amounts`
/// determining whether the solution was provided for the desired order.
#[derive(Serialize, PartialEq, Debug, Clone)]
#[cfg_attr(any(test, feature = "e2e"), derive(serde::Deserialize))]
#[serde(rename_all = "camelCase")]
pub struct SolutionInclusion {
    /// The name or identifier of the solver.
    pub solver: String,
    /// The executed amounts for the order as proposed by the solver, included
    /// if the solution was for the desired order, or omitted otherwise.
    pub executed_amounts: Option<ExecutedAmounts>,
}

#[derive(Serialize, PartialEq, Debug, Clone)]
#[cfg_attr(any(test, feature = "e2e"), derive(serde::Deserialize))]
#[serde(tag = "type", rename_all = "camelCase", content = "value")]
pub enum Status {
    /// Order is part of the orderbook but not actively being worked on. This
    /// can for example happen if the necessary balances are missing or if
    /// the order's signature check fails.
    Open,
    /// Order awaits being put into the current auction.
    Scheduled,
    /// Order is part of the current and solvers are computing solutions for it.
    Active,
    /// Some solvers proposed solutions for the orders but did not win the
    /// competition.
    Solved(Vec<SolutionInclusion>),
    /// The order was contained in the winning solution which the solver
    /// currently tries to submit onchain.
    Executing(Vec<SolutionInclusion>),
    /// The order was successfully executed onchain.
    Traded(Vec<SolutionInclusion>),
    /// The user cancelled the order. It will no longer show up in any auctions.
    Cancelled,
}
