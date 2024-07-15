//! Solvers propose solutions to an [`crate::domain::Auction`].
//!
//! A winning solution becomes a [`Settlement::Solution`] once it is executed
//! on-chain.

use {
    super::{tokenized, trade, Trade, Transaction},
    crate::{
        domain::{auction, eth},
        infra,
    },
};

/// A solution together with the `auction_id` for which it was picked as a
/// winner.
///
/// Referenced as a [`Settlement::Solution`] in the codebase.
#[allow(dead_code)]
#[derive(Debug)]
pub struct Solution {
    trades: Vec<Trade>,
    /// Data that was appended to the regular call data of the `settle()` call
    /// as a form of on-chain meta data. This is used to associate a
    /// solution with an auction for which this solution was picked as a winner.
    auction_id: auction::Id,
    deadline: eth::BlockNo,
}

impl Solution {
    pub async fn new(
        tx: &Transaction,
        domain_separator: &eth::DomainSeparator,
        persistence: &infra::Persistence,
    ) -> Result<Self, Error> {
        let tokenized::Tokenized {
            tokens,
            clearing_prices,
            trades: decoded_trades,
            interactions: _interactions,
            auction_id,
        } = tokenized::Tokenized::new(&tx.input)?;

        let mut trades = Vec::with_capacity(decoded_trades.len());
        for trade in decoded_trades {
            let flags = tokenized::TradeFlags(trade.8);
            let sell_token_index = trade.0.as_usize();
            let buy_token_index = trade.1.as_usize();
            let sell_token = tokens[sell_token_index];
            let buy_token = tokens[buy_token_index];
            let uniform_sell_token_index = tokens
                .iter()
                .position(|token| token == &sell_token)
                .unwrap();
            let uniform_buy_token_index =
                tokens.iter().position(|token| token == &buy_token).unwrap();
            trades.push(Trade::new(
                tokenized::order_uid(&trade, &tokens, domain_separator)
                    .map_err(Error::OrderUidRecover)?,
                eth::Asset {
                    token: sell_token.into(),
                    amount: trade.3.into(),
                },
                eth::Asset {
                    token: buy_token.into(),
                    amount: trade.4.into(),
                },
                flags.side(),
                trade.9.into(),
                trade::Prices {
                    uniform: trade::ClearingPrices {
                        sell: clearing_prices[uniform_sell_token_index],
                        buy: clearing_prices[uniform_buy_token_index],
                    },
                    custom: trade::ClearingPrices {
                        sell: clearing_prices[sell_token_index],
                        buy: clearing_prices[buy_token_index],
                    },
                },
            ));
        }

        let deadline = persistence.get_auction_deadline(auction_id).await?;

        Ok(Self {
            trades,
            deadline,
            auction_id,
        })
    }

    pub fn auction_id(&self) -> auction::Id {
        self.auction_id
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Decoding(#[from] tokenized::error::Decoding),
    #[error("failed to recover order uid {0}")]
    OrderUidRecover(tokenized::error::Uid),
    #[error(transparent)]
    Deadline(#[from] infra::persistence::error::Deadline),
}
