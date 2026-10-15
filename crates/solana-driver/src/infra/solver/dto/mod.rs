//! Wire DTOs for the driver <-> solver-engine HTTP boundary.
//!
//! These structs are the driver's own mirror of the `solana-solvers`
//! `dto/{auction,solution}.rs` types; they are deliberately not shared with the
//! solver crate so the wire format can evolve on one side at a time. Serde
//! tests in each module pin the JSON shape against the literals the solver
//! crate tests assert.
//!
//! Eventually, we could extract a shared `dto` crate (like the EVM driver does
//! with `solvers-dto`) to keep the API consistent without needing these tests.

pub mod auction;
pub mod solution;

pub use solution::Solutions;
