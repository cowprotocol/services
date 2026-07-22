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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// Quote `order` for settlement signer `user`.
    pub async fn swap(&self, order: &Order, user: &Pubkey) -> Result<Swap, jupiter::Error> {
        match self {
            Dex::Jupiter(jupiter) => jupiter.swap(order, user).await,
        }
    }
}
