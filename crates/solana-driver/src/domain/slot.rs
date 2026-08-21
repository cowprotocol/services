//! A Solana slot number.
///
/// TODO: consolidate `Slot` (and other shared Solana primitive types) into the
/// `chain-types` crate.
use std::fmt;

/// A Solana slot number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Slot(pub u64);

impl fmt::Display for Slot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
