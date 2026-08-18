//! Outbound `/solve` request: the auction the driver posts to a solver engine.
//!
//! The wire format matches `solana-solvers/src/dto/auction.rs`.

use {
    crate::{
        domain::{self, Side, order_uid::OrderUid},
        util::associated_token_address,
    },
    serde::Serialize,
    serde_with::serde_as,
    solana_sdk::pubkey::Pubkey,
};

/// The auction the driver posts to `/solve`.
#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Auction {
    pub id: i64,
    /// Settlement signer the swap instructions are built for.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub taker: Pubkey,
    pub orders: Vec<Order>,
}

/// One order to quote.
#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub uid: OrderUid,
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub sell_mint: Pubkey,
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub buy_mint: Pubkey,
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub buy_destination: Pubkey,
    /// Sell amount for a sell, buy amount for a buy
    /// Represented as a quoted decimal string instead of a JSON number.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub amount: u64,
    pub side: Side,
}

impl Order {
    /// Build the wire order from a domain order and the settlement taker.
    ///
    /// The `taker` is the solver that signs the settlement transaction; the
    /// driver derives `buy_destination` as its ATA for the buy mint on the same
    /// premise as the sell token.
    fn from_order_and_taker(order: &domain::Order, taker: Pubkey) -> Self {
        Self {
            uid: order.uid,
            sell_mint: order.sell_mint,
            buy_mint: order.buy_mint,
            buy_destination: associated_token_address(&taker, &order.buy_mint),
            amount: order.amount,
            side: order.side,
        }
    }
}

impl Auction {
    /// Build the wire auction from the domain auction.
    ///
    /// The `taker` is a concept borrowed from the solana-solvers API: the
    /// solver that signs the settlement transaction. Under the current API the
    /// sell token is the taker's ATA, and the driver derives `buy_destination`
    /// as its buy-side counterpart on the same premise.
    pub fn new(auction: &domain::Auction, taker: Pubkey) -> Self {
        Self {
            id: auction.id,
            taker,
            orders: auction
                .orders
                .iter()
                .map(|order| Order::from_order_and_taker(order, taker))
                .collect(),
        }
    }
}
