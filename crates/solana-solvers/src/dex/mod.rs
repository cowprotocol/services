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
    pub sell_amount: u64,
    pub buy_amount: u64,
    pub side: Side,
}

impl Order {
    /// The amount to quote: the sell amount for a `Sell`, the buy amount for
    /// a `Buy`.
    pub fn amount(&self) -> u64 {
        match self.side {
            Side::Sell => self.sell_amount,
            Side::Buy => self.buy_amount,
        }
    }
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

impl Swap {
    /// Whether the swap fills `order` at or better than its limit price.
    ///
    /// The same inclusive cross-multiplication as the settlement program and
    /// the driver, so nothing kept here is dropped downstream.
    pub fn satisfies(&self, order: &Order) -> bool {
        u128::from(self.out_amount) * u128::from(order.sell_amount)
            >= u128::from(self.in_amount) * u128::from(order.buy_amount)
    }

    /// Output-unit gap to `order`'s limit, zero when [`Swap::satisfies`] —
    /// same cross-multiplication, so the two can't disagree.
    pub fn shortfall(&self, order: &Order) -> u128 {
        let demanded = u128::from(self.in_amount) * u128::from(order.buy_amount);
        let offered = u128::from(self.out_amount) * u128::from(order.sell_amount);
        demanded.saturating_sub(offered) / u128::from(order.sell_amount).max(1)
    }
}

/// The configured DEX backend.
pub enum Dex {
    Jupiter(jupiter::Jupiter),
}

impl Dex {
    /// Build the swap for `order` that the settlement signer `user` executes.
    ///
    /// The route spends its input from the sell-mint ATA of the solver.
    /// Jupiter has no source-account override. The sell funds must already be
    /// in that ATA. The caller must make sure that the ATA exists before the
    /// pull instructions of the swap run.
    pub async fn swap(&self, order: &Order, user: &Pubkey) -> Result<Swap, jupiter::Error> {
        match self {
            Dex::Jupiter(jupiter) => jupiter.swap(order, user).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order(side: Side, sell_amount: u64, buy_amount: u64) -> Order {
        Order {
            sell_mint: Pubkey::new_from_array([1; 32]),
            buy_mint: Pubkey::new_from_array([2; 32]),
            buy_destination: Pubkey::new_from_array([3; 32]),
            sell_amount,
            buy_amount,
            side,
        }
    }

    fn swap(in_amount: u64, out_amount: u64) -> Swap {
        Swap {
            in_amount,
            out_amount,
            instructions: vec![],
            address_lookup_tables: vec![],
        }
    }

    #[test]
    fn amount_is_the_side_leg() {
        assert_eq!(order(Side::Sell, 1_000, 2_000).amount(), 1_000);
        assert_eq!(order(Side::Buy, 1_000, 2_000).amount(), 2_000);
    }

    #[test]
    fn a_fill_at_the_limit_satisfies_the_order() {
        let sell = order(Side::Sell, 1_000, 2_000);
        assert!(swap(1_000, 2_000).satisfies(&sell));
        assert!(!swap(1_000, 1_999).satisfies(&sell));

        let buy = order(Side::Buy, 1_000, 2_000);
        assert!(swap(1_000, 2_000).satisfies(&buy));
        assert!(!swap(1_001, 2_000).satisfies(&buy));
    }

    /// Quote auctions leave the limit leg open: zero to buy for a sell order,
    /// max to sell for a buy order.
    #[test]
    fn an_open_limit_accepts_any_fill() {
        assert!(swap(1_000, 1).satisfies(&order(Side::Sell, 1_000, 0)));
        assert!(swap(u64::MAX, 1_000).satisfies(&order(Side::Buy, u64::MAX, 1_000)));
    }

    #[test]
    fn shortfall_is_the_output_gap_at_the_limit() {
        let sell = order(Side::Sell, 1_000, 2_000);
        assert_eq!(swap(1_000, 2_000).shortfall(&sell), 0);
        assert_eq!(swap(1_000, 1_999).shortfall(&sell), 1);
        assert_eq!(swap(1_000, 1_900).shortfall(&sell), 100);

        let buy = order(Side::Buy, 1_000, 2_000);
        assert_eq!(swap(1_000, 2_000).shortfall(&buy), 0);
        assert_eq!(swap(1_001, 2_000).shortfall(&buy), 2);
        assert_eq!(swap(1_100, 2_000).shortfall(&buy), 200);
    }

    #[test]
    fn shortfall_agrees_with_satisfies() {
        for side in [Side::Sell, Side::Buy] {
            for (in_amount, out_amount) in [(1_000, 2_000), (1_000, 1_999), (1_001, 2_000)] {
                let order = order(side, 1_000, 2_000);
                let swap = swap(in_amount, out_amount);
                assert_eq!(swap.satisfies(&order), swap.shortfall(&order) == 0);
            }
        }
    }

    #[test]
    fn shortfall_survives_a_zero_sell_leg() {
        swap(1_000, 2_000).shortfall(&order(Side::Sell, 0, 2_000));
    }

    #[test]
    fn max_legs_do_not_overflow() {
        let order = order(Side::Sell, u64::MAX, u64::MAX);
        assert!(swap(u64::MAX, u64::MAX).satisfies(&order));
        assert!(!swap(u64::MAX, u64::MAX - 1).satisfies(&order));
    }
}
