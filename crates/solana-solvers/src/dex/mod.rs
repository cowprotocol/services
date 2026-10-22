//! DEX-adapter boundary: quote one order, then turn the quote into an
//! executable swap.
//!
//! `Dex` dispatches to the configured engine.

pub mod bitget;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Side {
    Buy,
    Sell,
}

/// A priced swap that cannot execute yet: the amounts, plus what the backend
/// needs to build the swap. Requoting takes one request, building the swap a
/// second one, so a caller that may requote builds only the swap it keeps.
#[derive(Debug, Clone)]
pub struct Quote {
    pub in_amount: u64,
    pub out_amount: u64,
    /// Where the swap sends its output, the quoted order's
    /// `buy_destination`.
    pub destination: Pubkey,
    /// The backend's own quote, passed back verbatim to build the swap.
    pub(crate) raw: serde_json::Value,
}

impl Quote {
    /// See [`clearing_rate`].
    pub fn clearing_rate(&self, side: Side, orders: usize) -> Option<(u64, u64)> {
        clearing_rate(self.in_amount, self.out_amount, side, orders)
    }
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
    pub fn satisfies(&self, order: &Order) -> bool {
        satisfies(order, self.in_amount, self.out_amount)
    }

    /// Output-unit gap to `order`'s limit, zero when [`Swap::satisfies`].
    pub fn shortfall(&self, order: &Order) -> u128 {
        shortfall(order, self.in_amount, self.out_amount)
    }

    /// See [`clearing_rate`].
    pub fn clearing_rate(&self, side: Side, orders: usize) -> Option<(u64, u64)> {
        clearing_rate(self.in_amount, self.out_amount, side, orders)
    }
}

/// The `(in, out)` rate at which `orders` orders on `side`, filled together
/// by a swap of `in_amount` for `out_amount`, clear: the rate their clearing
/// prices encode.
///
/// The driver derives each trade's counterpart leg from those prices,
/// rounded down. For sells that is the payout, so the payouts add up to at
/// most the swap's output. For buys it is the amount pulled from the user,
/// which could add up to `orders - 1` short of the swap's input: raising the
/// input by that much makes the pulls cover it, at a cost of under one base
/// unit per order. One order clears at the swap's own rate either way.
/// `None` when the raised input overflows.
pub fn clearing_rate(
    in_amount: u64,
    out_amount: u64,
    side: Side,
    orders: usize,
) -> Option<(u64, u64)> {
    let in_amount = match side {
        Side::Sell => in_amount,
        Side::Buy => in_amount.checked_add(u64::try_from(orders.saturating_sub(1)).ok()?)?,
    };
    Some((in_amount, out_amount))
}

/// Whether trading `in_amount` for `out_amount` fills `order` at or better
/// than its limit price.
///
/// The same inclusive cross-multiplication as the settlement program and the
/// driver, so nothing kept here is dropped downstream.
pub fn satisfies(order: &Order, in_amount: u64, out_amount: u64) -> bool {
    u128::from(out_amount) * u128::from(order.sell_amount)
        >= u128::from(in_amount) * u128::from(order.buy_amount)
}

/// Output-unit gap to `order`'s limit at the rate `in_amount` for
/// `out_amount`, zero when [`satisfies`]: same cross-multiplication, so the
/// two can't disagree.
pub fn shortfall(order: &Order, in_amount: u64, out_amount: u64) -> u128 {
    let demanded = u128::from(in_amount) * u128::from(order.buy_amount);
    let offered = u128::from(out_amount) * u128::from(order.sell_amount);
    demanded.saturating_sub(offered) / u128::from(order.sell_amount).max(1)
}

/// The configured DEX backend.
pub enum Dex {
    Jupiter(jupiter::Jupiter),
    Bitget(bitget::Bitget),
}

impl Dex {
    /// Price `order` without building its swap.
    pub async fn quote(&self, order: &Order) -> Result<Quote, Error> {
        match self {
            Dex::Jupiter(jupiter) => jupiter.quote(order).await,
            Dex::Bitget(bitget) => bitget.quote(order).await,
        }
    }

    /// Build the swap for `quote` that the settlement signer `user` executes,
    /// at the quote's amounts.
    ///
    /// The route spends its input from the sell-mint ATA of the solver.
    /// Jupiter has no source-account override. The sell funds must already be
    /// in that ATA. The caller must make sure that the ATA exists before the
    /// pull instructions of the swap run.
    pub async fn swap(&self, quote: &Quote, user: &Pubkey) -> Result<Swap, Error> {
        match self {
            Dex::Jupiter(jupiter) => jupiter.swap(quote, user).await,
            Dex::Bitget(bitget) => bitget.swap(quote, user).await,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to build the request")]
    RequestBuildFailed,
    #[error("no route for this order")]
    NotFound,
    #[error("order type is not supported")]
    OrderNotSupported,
    #[error("rate limited")]
    RateLimited,
    #[error("dex api error {status}: {body}")]
    Api { status: u16, body: String },
    #[error("malformed swap response: {0}")]
    BadResponse(String),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
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

    fn swap_of(in_amount: u64) -> Swap {
        swap(in_amount, 1)
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
    fn buys_clear_at_an_input_raised_by_one_unit_per_extra_order() {
        let swap = swap(5, 3);
        assert_eq!(swap.clearing_rate(Side::Sell, 3), Some((5, 3)));
        assert_eq!(swap.clearing_rate(Side::Buy, 1), Some((5, 3)));
        assert_eq!(swap.clearing_rate(Side::Buy, 3), Some((7, 3)));
        assert_eq!(swap_of(u64::MAX).clearing_rate(Side::Buy, 2), None);
    }

    #[test]
    fn max_legs_do_not_overflow() {
        let order = order(Side::Sell, u64::MAX, u64::MAX);
        assert!(swap(u64::MAX, u64::MAX).satisfies(&order));
        assert!(!swap(u64::MAX, u64::MAX - 1).satisfies(&order));
    }
}
