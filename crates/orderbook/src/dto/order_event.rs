use {
    chrono::{DateTime, Utc},
    model::order::OrderUid,
    serde::{Deserialize, Serialize},
};

/// A step in the life cycle of an order.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderEvent {
    pub uid: OrderUid,
    pub timestamp: DateTime<Utc>,
    pub label: Label,
    /// Only set for the labels `filtered` and `invalid`.
    pub reason: Option<Reason>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Label {
    Created,
    Ready,
    Filtered,
    Invalid,
    Executing,
    Considered,
    Traded,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    InFlight,
    BannedUser,
    InvalidSignature,
    UnsupportedToken,
    InsufficientBalance,
    DustOrder,
    MissingNativePrice,
}
