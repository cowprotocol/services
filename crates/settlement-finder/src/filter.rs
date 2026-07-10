//! Strict-mode trade filters for the `block` subcommand.

use {
    crate::chain::{ChainTrade, SettlementTx},
    alloy_primitives::{Address, Bytes, U256},
    anyhow::{Result, ensure},
};

/// When any of these is set, only Trade events matching all of them count.
#[derive(clap::Args)]
pub struct FilterArgs {
    /// Only report Trade events with this order uid (56 bytes hex).
    #[arg(long)]
    order_uid: Option<Bytes>,

    /// Only report Trade events with this order owner.
    #[arg(long)]
    owner: Option<Address>,

    /// Only report Trade events selling this token.
    #[arg(long)]
    sell_token: Option<Address>,

    /// Only report Trade events buying this token.
    #[arg(long)]
    buy_token: Option<Address>,

    /// Only report Trade events with exactly this executed sell amount
    /// (atoms, fees included).
    #[arg(long)]
    sell_amount: Option<U256>,

    /// Only report Trade events with exactly this executed buy amount
    /// (atoms).
    #[arg(long)]
    buy_amount: Option<U256>,

    /// Only report Trade events with exactly this executed fee amount
    /// (atoms of the sell token).
    #[arg(long)]
    fee_amount: Option<U256>,
}

impl FilterArgs {
    pub fn validate(&self) -> Result<()> {
        if let Some(uid) = &self.order_uid {
            ensure!(
                uid.len() == 56,
                "--order-uid must be 56 bytes, got {}",
                uid.len()
            );
        }
        Ok(())
    }

    pub fn is_active(&self) -> bool {
        self.order_uid.is_some()
            || self.owner.is_some()
            || self.sell_token.is_some()
            || self.buy_token.is_some()
            || self.sell_amount.is_some()
            || self.buy_amount.is_some()
            || self.fee_amount.is_some()
    }

    fn matches(&self, trade: &ChainTrade) -> bool {
        fn check<T: PartialEq>(filter: &Option<T>, value: &T) -> bool {
            filter.as_ref().is_none_or(|filter| filter == value)
        }
        self.order_uid
            .as_ref()
            .is_none_or(|uid| uid.as_ref() == trade.order_uid)
            && check(&self.owner, &trade.owner)
            && check(&self.sell_token, &trade.sell_token)
            && check(&self.buy_token, &trade.buy_token)
            && check(&self.sell_amount, &trade.sell_amount)
            && check(&self.buy_amount, &trade.buy_amount)
            && check(&self.fee_amount, &trade.fee_amount)
    }

    /// Drops non-matching trades and, if the filter is active, transactions
    /// without any matching trade.
    pub fn apply(&self, txs: &mut Vec<SettlementTx>) {
        if !self.is_active() {
            return;
        }
        for tx in txs.iter_mut() {
            tx.trades.retain(|trade| self.matches(trade));
        }
        txs.retain(|tx| !tx.trades.is_empty());
    }
}
