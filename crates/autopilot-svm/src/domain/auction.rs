//! The Solana auction domain: solvable orders typed over the shared chain
//! vocabulary and their assembly from database rows.

use {
    crate::run_loop::AuctionInfo,
    chain_types::solana::{AppData, IntentHash, Pubkey},
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
    /// separate receiver field like on EVM.
    pub buy_token_account: Pubkey,
    pub sell_amount: u64,
    pub buy_amount: u64,
    pub valid_to: u32,
    pub kind: OrderKind,
    pub partially_fillable: bool,
    pub order_pda: Pubkey,
    pub app_data: AppData,
    /// Whether the order PDA already exists on chain. A pending sponsored
    /// order creates its own accounts (the order PDA, the buy token account)
    /// only at settlement time through its presigned transaction.
    pub created_on_chain: bool,
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
        super::{Auction, HashMap, Order, OrderKind, Pubkey},
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
        }
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
