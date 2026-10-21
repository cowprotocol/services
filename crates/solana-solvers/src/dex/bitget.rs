//! Bitget swap-API adapter.

use {
    super::{Error, Order, Swap},
    solana_sdk::pubkey::Pubkey,
};

pub struct Bitget;

impl Bitget {
    // TODO(BE-253): quote the order and build the swap.
    pub async fn swap(&self, _order: &Order, _taker: &Pubkey) -> Result<Swap, Error> {
        Err(Error::NotFound)
    }
}
