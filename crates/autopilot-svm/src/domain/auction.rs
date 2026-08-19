//! The Solana auction domain: solvable orders typed over the shared chain
//! vocabulary and their assembly from database rows.

use {
    crate::run_loop::AuctionInfo,
    chain_types::solana::{IntentHash, Pubkey},
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
    /// separate receiver field like on EVM.
    pub buy_token_account: Pubkey,
    pub sell_amount: u64,
    pub buy_amount: u64,
    pub valid_to: u32,
    pub kind: OrderKind,
    pub partially_fillable: bool,
    pub order_pda: Pubkey,
}

/// The cut auction the loop fans out to solvers.
#[derive(Clone, Debug)]
pub struct Auction {
    /// Autopilot-assigned id. Excluded from equality: the dedupe compares two
    /// cuts by content, and the id is allocated only for a fresh cut.
    pub id: i64,
    /// Slot the auction was cut at. Excluded from equality for the same
    /// reason: the tip moves every cycle, the content may not.
    pub tip: u64,
    pub orders: Vec<Order>,
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
    use super::{Auction, Order, OrderKind};

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
        }
    }

    #[test]
    fn auction_equality_ignores_id_and_tip() {
        let orders = vec![order(10)];
        let a = Auction {
            id: 1,
            tip: 100,
            orders: orders.clone(),
        };
        let b = Auction {
            id: 2,
            tip: 200,
            orders,
        };
        assert_eq!(a, b);
        assert_ne!(
            a,
            Auction {
                id: 1,
                tip: 100,
                orders: vec![],
            }
        );
    }
}
