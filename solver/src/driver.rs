use crate::{
    ethereum::{Ethereum, SettlementContract},
    settlement,
};
use anyhow::Result;
use model::{Order, TokenPair};
use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Arc,
    time::{Duration, Instant},
};

pub struct Driver {
    contract: Arc<dyn SettlementContract>,
    ethereum: Arc<dyn Ethereum>,
    nonces: HashMap<TokenPair, u32>,
    max_order_age: Duration,
}

impl Driver {
    pub async fn settle_if_needed(&mut self, mut orders: Vec<Order>) -> Result<()> {
        self.remove_invalid_orders(&mut orders);
        self.update_missing_nonces(&orders).await?;
        // This is a loop because several settlements of different token pairs could be triggered
        // in the same update.
        while let Some(settlement) =
            settlement::find_settlement(&orders, &self.nonces, Instant::now(), self.max_order_age)
        {
            // Optimistically increase the nonce assuming our settlement goes through. This prevents
            // from settling the same orders next this function is called because the nonce will no
            // longer match.
            // TODO: send settlement transaction and set up reaction to failure
            // Unwrap because we have the nonce for all tokens in the order book.
            *self.nonces.get_mut(&settlement.token_pair).unwrap() += 1;
        }
        Ok(())
    }

    fn remove_invalid_orders(&self, orders: &mut Vec<Order>) {
        // TODO: Filter invalid orders based on spending being approved and token balance. Probably
        // using a cache so that we make less requests to the node.
        orders.retain(|order| order.user_provided.token_pair().is_some());
    }

    async fn update_missing_nonces(&mut self, orders: &[Order]) -> Result<()> {
        for order in orders {
            // Unwrap because same token pair would be an invalid order which has already been
            // filtered out.
            let token_pair = order.user_provided.token_pair().unwrap();
            match self.nonces.entry(token_pair) {
                Entry::Occupied(_) => (),
                Entry::Vacant(entry) => {
                    entry.insert(self.contract.get_nonce(token_pair).await?);
                }
            }
        }
        Ok(())
    }

    fn settlement_failed(&mut self, token_pair: TokenPair) {
        // Causes the nonce to get updated next time it is needed. We do this instead of
        // decrementing it in case it somehow got out of sync with the contract.
        self.nonces.remove(&token_pair);
    }
}
