//! Types mirroring the `solana.*` schema.
//!
//! The schema is not on the connection's search path, so Postgres reports its
//! enums schema-qualified and the type names carry the qualification.

/// Which amount the order fixes.
#[derive(Clone, Copy, Debug, Eq, PartialEq, sqlx::Type)]
#[sqlx(type_name = "solana.OrderKind")]
#[sqlx(rename_all = "lowercase")]
pub enum OrderKind {
    Buy,
    Sell,
}

/// What an order event attests about its order.
#[derive(Clone, Copy, Debug, Eq, PartialEq, sqlx::Type)]
#[sqlx(type_name = "solana.OrderEventLabel")]
#[sqlx(rename_all = "lowercase")]
pub enum OrderEventLabel {
    Created,
    Ready,
    Filtered,
    Invalid,
    Executing,
    Considered,
    Traded,
    Cancelled,
}
