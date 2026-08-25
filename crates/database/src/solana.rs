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
