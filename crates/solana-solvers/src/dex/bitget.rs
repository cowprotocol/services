//! Bitget swap-API adapter.

use {
    super::{Error, Order, Quote, Swap},
    solana_sdk::pubkey::Pubkey,
};

pub struct Bitget;

impl Bitget {
    // TODO(BE-253): quote the order.
    pub async fn quote(&self, _order: &Order) -> Result<Quote, Error> {
        Err(Error::NotFound)
    }

    // TODO(BE-253): build the swap for the quote.
    pub async fn swap(&self, _quote: &Quote, _taker: &Pubkey) -> Result<Swap, Error> {
        Err(Error::NotFound)
    }
}
