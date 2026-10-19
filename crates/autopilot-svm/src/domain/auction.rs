//! The Solana auction domain: solvable orders typed over the shared chain
//! vocabulary and their assembly from database rows.

use {
    crate::run_loop::AuctionInfo,
    chain_types::solana::{AppData, IntentHash, NATIVE_SOL, Pubkey},
    std::collections::HashMap,
};

/// Whether the order sells an exact amount or buys an exact amount.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrderKind {
    Sell,
    Buy,
}

/// One solvable order.
#[derive(Clone, Debug, PartialEq)]
pub struct Order {
    pub uid: IntentHash,
    pub owner: Pubkey,
    pub sell_token: Pubkey,
    pub buy_token: Pubkey,
    pub sell_token_account: Pubkey,
    /// Where the buy tokens are paid out. Any SPL token account: it names its
    /// own owner and mint, so it doubles as the receiver and there is no
    /// separate receiver field like on EVM. For a native SOL buy it is the
    /// wallet receiving the lamports.
    pub buy_token_account: Pubkey,
    pub sell_amount: u64,
    pub buy_amount: u64,
    pub valid_to: u32,
    pub kind: OrderKind,
    pub partially_fillable: bool,
    pub order_pda: Pubkey,
    pub app_data: AppData,
    /// Whether the order PDA already exists on chain. A pending sponsored
    /// order creates its own accounts (the order PDA, the buy token account
    /// of a token buy) only at settlement time through its presigned
    /// transaction.
    pub created_on_chain: bool,
    /// The cumulative fill on the order's own side: sell-token units for a
    /// sell order, buy-token units for a buy order.
    pub executed: u64,
    /// The owner-signed creation transaction of a pending sponsored order,
    /// serialized. `None` for an order created on chain.
    pub creation: Option<Vec<u8>>,
    /// What the sell token account can fund at the cut: the held amount
    /// capped by the delegated one. `None` for a pending sponsored order,
    /// whose creation funds it.
    pub sell_balance: Option<u64>,
}

impl Order {
    /// Whether the order buys native SOL instead of an SPL token.
    pub fn buys_native_sol(&self) -> bool {
        self.buy_token == NATIVE_SOL
    }

    /// The amounts still open to fill: the order-side target less `executed`,
    /// the other leg scaled in proportion.
    ///
    /// TODO: duplicate of the driver's `Order::remaining`; unify the two.
    pub fn remaining(&self) -> Remaining {
        let (target, _) = self.legs();
        self.scaled(target.saturating_sub(self.executed))
    }

    /// What a solver may fill: `remaining` scaled down to `sell_balance` for
    /// a partially fillable order. A fill-or-kill order keeps its remainder.
    /// The cut judges an order by the legs the driver sends to solvers.
    ///
    /// TODO: duplicate of the driver's `Order::available`; unify the two.
    pub fn available(&self) -> Remaining {
        let remaining = self.remaining();
        let Some(balance) = self.sell_balance else {
            return remaining;
        };
        if !self.partially_fillable || balance >= remaining.sell {
            return remaining;
        }
        let open = match self.kind {
            OrderKind::Sell => balance,
            // The largest buy whose sell leg, rounded down, fits the balance.
            // `balance < sell_amount`, so the quotient fits u64.
            OrderKind::Buy => fits(
                u128::from(self.buy_amount) * u128::from(balance) / u128::from(self.sell_amount),
            ),
        };
        self.scaled(open)
    }

    /// The signed order-side target and the other leg.
    fn legs(&self) -> (u64, u64) {
        match self.kind {
            OrderKind::Sell => (self.sell_amount, self.buy_amount),
            OrderKind::Buy => (self.buy_amount, self.sell_amount),
        }
    }

    /// The legs for `open` of the order-side target, the other leg scaled in
    /// proportion. Rounds like the driver, the sell leg down and the buy leg
    /// up, so the scaled limit is never looser than the signed one.
    fn scaled(&self, open: u64) -> Remaining {
        if open == 0 {
            return Remaining { sell: 0, buy: 0 };
        }
        let (target, other) = self.legs();
        let scaled = u128::from(other) * u128::from(open);
        let target = u128::from(target);
        // `open <= target`, so the quotient never exceeds `other`.
        match self.kind {
            OrderKind::Sell => Remaining {
                sell: open,
                buy: fits(scaled.div_ceil(target)),
            },
            OrderKind::Buy => Remaining {
                sell: fits(scaled / target),
                buy: open,
            },
        }
    }
}

/// Narrows a leg scaled from, and bounded by, `u64` amounts.
fn fits(leg: u128) -> u64 {
    u64::try_from(leg).expect("a scaled leg fits u64")
}

/// What is left of an order to fill, see [`Order::remaining`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Remaining {
    pub sell: u64,
    pub buy: u64,
}

impl Remaining {
    /// Whether a leg scaled down to nothing. The program only accepts a fill
    /// moving zero on that side, so no solver can fill the order.
    pub fn has_zero_leg(self) -> bool {
        self.sell == 0 || self.buy == 0
    }
}

/// The cut auction the loop fans out to solvers.
#[derive(Clone, Debug)]
pub struct Auction {
    /// Autopilot-assigned id. Excluded from equality: the dedupe compares two
    /// cuts by content, and the id is allocated only for a fresh cut.
    pub id: i64,
    pub orders: Vec<Order>,
    /// The lamport value of one atom of each auction token, scaled by 10^9.
    /// Tokens without a listing are absent. Excluded from equality like the
    /// id: prices refresh between cuts without making the auction new.
    pub native_prices: HashMap<Pubkey, u64>,
}

impl PartialEq for Auction {
    fn eq(&self, other: &Self) -> bool {
        self.orders == other.orders
    }
}

impl AuctionInfo for Auction {
    fn id(&self) -> i64 {
        self.id
    }
}

#[cfg(test)]
mod tests {
    use {
        super::{Auction, HashMap, Order, OrderKind, Pubkey, Remaining},
        chain_types::solana::AppData,
    };

    fn order(sell_amount: u64) -> Order {
        Order {
            uid: chain_types::solana::IntentHash([1; 32]),
            owner: chain_types::solana::Pubkey([2; 32]),
            sell_token: chain_types::solana::Pubkey([3; 32]),
            buy_token: chain_types::solana::Pubkey([4; 32]),
            sell_token_account: chain_types::solana::Pubkey([5; 32]),
            buy_token_account: chain_types::solana::Pubkey([6; 32]),
            sell_amount,
            buy_amount: 1_000,
            valid_to: 42,
            kind: OrderKind::Sell,
            partially_fillable: false,
            order_pda: chain_types::solana::Pubkey([7; 32]),
            app_data: AppData([0; 32]),
            created_on_chain: true,
            executed: 0,
            creation: None,
            sell_balance: None,
        }
    }

    /// 999 of 1000 sold leaves 1 to sell and the 1000 buy limit scales to 1,
    /// rounding up when it does not divide; 400 of 1000 bought leaves 600 to
    /// buy and the sell limit scales down to 600.
    #[test]
    fn remaining_scales_the_other_leg_like_the_driver() {
        let sell = order(1_000);
        assert_eq!(
            sell.remaining(),
            Remaining {
                sell: 1_000,
                buy: 1_000
            }
        );
        let filled = Order {
            executed: 999,
            ..sell.clone()
        };
        assert_eq!(filled.remaining(), Remaining { sell: 1, buy: 1 });
        assert_eq!(
            Order {
                buy_amount: 1_001,
                ..filled
            }
            .remaining(),
            Remaining { sell: 1, buy: 2 }
        );
        let buy = Order {
            kind: OrderKind::Buy,
            executed: 400,
            ..sell
        };
        assert_eq!(
            buy.remaining(),
            Remaining {
                sell: 600,
                buy: 600
            }
        );
    }

    /// The sell balance caps a partially fillable order like in the driver:
    /// 400 of 1000 sold leaves 600, a balance of 300 halves both legs; 400
    /// of 1000 bought leaves 600 for 600, a balance of 333 buys 333. A buy
    /// order whose balance is under the price of one buy atom scales to
    /// nothing: 10 to buy for 1000, 99 buys 0 and 100 buys 1. A
    /// fill-or-kill order keeps its remainder.
    #[test]
    fn available_scales_the_remainder_down_to_the_sell_balance() {
        let sell = Order {
            partially_fillable: true,
            executed: 400,
            sell_balance: Some(300),
            ..order(1_000)
        };
        assert_eq!(
            sell.remaining(),
            Remaining {
                sell: 600,
                buy: 600
            }
        );
        assert_eq!(
            sell.available(),
            Remaining {
                sell: 300,
                buy: 300
            }
        );
        assert_eq!(
            Order {
                partially_fillable: false,
                ..sell.clone()
            }
            .available(),
            sell.remaining()
        );
        let buy = Order {
            kind: OrderKind::Buy,
            sell_balance: Some(333),
            ..sell
        };
        assert_eq!(
            buy.available(),
            Remaining {
                sell: 333,
                buy: 333
            }
        );

        let dust = |balance| Order {
            kind: OrderKind::Buy,
            buy_amount: 10,
            partially_fillable: true,
            sell_balance: Some(balance),
            ..order(1_000)
        };
        assert!(dust(99).available().has_zero_leg());
        assert_eq!(dust(100).available(), Remaining { sell: 100, buy: 1 });
    }

    #[test]
    fn auction_equality_ignores_id_and_prices() {
        let orders = vec![order(10)];
        let a = Auction {
            id: 1,
            orders: orders.clone(),
            native_prices: HashMap::new(),
        };
        let b = Auction {
            id: 2,
            orders,
            native_prices: HashMap::from([(Pubkey([0x11; 32]), 7)]),
        };
        assert_eq!(a, b);
        assert_ne!(
            a,
            Auction {
                id: 1,
                orders: vec![],
                native_prices: HashMap::new(),
            }
        );
    }
}
