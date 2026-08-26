//! DEX-adapter boundary: quote one order into an executable swap.
//!
//! `Dex` dispatches to the configured engine.

pub mod jupiter;

use solana_sdk::{instruction::Instruction, pubkey::Pubkey};

/// A single order to quote, distilled from the auction.
#[derive(Debug, Clone)]
pub struct Order {
    pub sell_mint: Pubkey,
    pub buy_mint: Pubkey,
    /// Where the swap sends its output: the settlement's buy-mint buffer,
    /// resolved upstream (driver or autopilot). Passed to Jupiter as
    /// `destinationTokenAccount`. `FinalizeSettle` then pushes to the user.
    pub buy_destination: Pubkey,
    /// Sell amount for a `Sell`, buy amount for a `Buy`.
    pub amount: u64,
    pub side: Side,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Side {
    Buy,
    Sell,
}

/// A quoted swap: the executed amounts plus the instructions that perform it,
/// in execution order (setup, swap, cleanup). The address lookup tables travel
/// alongside so the driver can build the v0 transaction the instructions
/// assume.
#[derive(Debug, Clone)]
pub struct Swap {
    pub in_amount: u64,
    pub out_amount: u64,
    pub instructions: Vec<Instruction>,
    pub address_lookup_tables: Vec<Pubkey>,
}

/// The configured DEX backend.
pub enum Dex {
    Jupiter(jupiter::Jupiter),
}

impl Dex {
    /// Build the swap for `order` that the settlement signer `user` executes.
    ///
    /// The route spends its input from the solver's sell-mint ATA. Jupiter
    /// has no source-account override, so the settlement must pull the sell
    /// funds into that ATA, creating it if missing, before the swap executes.
    ///
    /// Note: the driver creates the solver's missing sell-mint ATA with an
    /// idempotent setup instruction inserted before `BeginSettle` (see the
    /// `solana-driver`'s `Settlement::prepare`/`instructions`), so the pull
    /// destination always exists by the time the swap runs.
    pub async fn swap(&self, order: &Order, user: &Pubkey) -> Result<Swap, jupiter::Error> {
        match self {
            Dex::Jupiter(jupiter) => jupiter.swap(order, user).await,
        }
    }
}
