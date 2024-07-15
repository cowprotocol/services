//! Solvers propose solutions to an [`crate::domain::Auction`].
//!
//! A winning solution becomes a [`Settlement`] once it is executed on-chain.

use crate::{domain, domain::eth, infra};

mod competition;
mod trade;
mod transaction;
pub use {
    competition::Auction,
    error::Error,
    trade::{tokenized, Trade},
    transaction::Transaction,
};

/// A solution together with the `Auction` for which it was picked as a winner.
///
/// Referenced as a [`Settlement`] in the codebase.
#[allow(dead_code)]
#[derive(Debug)]
pub struct Settlement {
    trades: Vec<Trade>,
    auction: Auction,
}

impl Settlement {
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
                    .map_err(|err| Error::OrderUidRecover(err, auction_id))?,
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
        if deadline < tx.block {
            // todo
        }

        let auction = persistence.get_auction(auction_id).await?;
        let solution = persistence.get_competition_solution(auction_id).await?;

        let score = domain::competition::Score::new(
            trades
                .iter()
                .map(|trade| {
                    trade.score(
                        &auction.prices,
                        auction
                            .fee_policies
                            .get(trade.order_uid())
                            .map(|value| value.as_slice())
                            .unwrap_or_default(),
                    )
                })
                .sum::<Result<eth::Ether, trade::Error>>()
                .map_err(error::Score::from)?,
        )
        .map_err(error::Score::from)?;

        if score != solution.score() {
            return Err(Error::Score(error::Score::Missmatch));
        }

        // TODO implement the rest of the checks

        Ok(Self { trades, auction })
    }
}

mod error {
    use super::*;

    #[derive(Debug, thiserror::Error)]
    pub enum Error {
        #[error(transparent)]
        Decoding(#[from] tokenized::error::Decoding),
        #[error("failed to recover order uid {0} for auction {1}")]
        OrderUidRecover(tokenized::error::Uid, crate::domain::auction::Id),
        #[error(transparent)]
        Score(#[from] Score),
        #[error(transparent)]
        Auction(#[from] infra::persistence::error::Auction),
        #[error(transparent)]
        Deadline(#[from] infra::persistence::error::Deadline),
        #[error(transparent)]
        Solution(#[from] infra::persistence::error::Solution),
    }
    #[derive(Debug, thiserror::Error)]
    pub enum Score {
        /// Per CIP38, zero score solutions are rejected.
        #[error(transparent)]
        Zero(#[from] domain::competition::ZeroScore),
        /// The solver's score calculation is lower than the promised score
        /// during competition.
        #[error("score different from score promised during competition")]
        Missmatch,
        /// Score calculation requires native prices for all tokens in the
        /// solution, so that the surplus can be normalized to native currency.
        #[error("missing native price for token {0:?}")]
        MissingPrice(eth::TokenAddress),
        #[error(transparent)]
        Math(trade::error::Math),
    }

    impl From<trade::Error> for Score {
        fn from(err: trade::Error) -> Self {
            match err {
                trade::Error::MissingPrice(token) => Self::MissingPrice(token),
                trade::Error::Math(err) => Self::Math(err),
            }
        }
    }
}
