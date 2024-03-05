//! This module defines Settlement as originated from a mined transaction
//! calldata.

use {
    crate::{
        boundary,
        domain::{
            auction::{self, order},
            eth,
            fee,
        },
    },
    ethcontract::{common::FunctionExt, tokens::Tokenize, U256},
    std::collections::HashMap,
};

pub mod tokenized;
pub mod trade;

pub use trade::{ClearingPrices, Trade};

/// Settlement originated from a calldata of a settlement transaction.
#[derive(Debug)]
pub struct Settlement {
    trades: Vec<Trade>,
    /// Data that was appended to the regular call data of the `settle()` call
    /// as a form of on-chain meta data. This is used to associate a
    /// settlement with an auction.
    auction_id: auction::Id,
}

impl Settlement {
    /// Number of bytes that may be appended to the calldata to store an auction
    /// id.
    pub const META_DATA_LEN: usize = 8;

    pub fn native_surplus(
        &self,
        prices: &auction::Prices,
    ) -> Result<eth::TokenAmount, trade::Error> {
        self.trades
            .iter()
            .map(|trade| trade.native_surplus(prices))
            .try_fold(num::Zero::zero(), |acc, score| {
                score.map(|score| acc + score)
            })
    }

    pub fn native_fee(&self, prices: &auction::Prices) -> Result<eth::TokenAmount, trade::Error> {
        self.trades
            .iter()
            .map(|trade| trade.native_fee(prices))
            .try_fold(num::Zero::zero(), |acc, score| {
                score.map(|score| acc + score)
            })
    }

    pub fn all_trade_fees(&self) -> HashMap<order::OrderUid, Option<eth::Asset>> {
        self.trades
            .iter()
            .map(|trade| {
                let fee = trade
                    .fee_in_sell_token()
                    .map_err(|err| {
                        tracing::warn!("fee failed for trade {:?}, err {}", trade.order_uid(), err)
                    })
                    .ok();
                (trade.order_uid(), fee)
            })
            .collect()
    }

    /// CIP38 score defined as surplus + protocol fee
    ///
    /// Denominated in NATIVE token
    pub fn score(
        &self,
        prices: &auction::Prices,
        policies: &[fee::Policy],
    ) -> Result<eth::TokenAmount, trade::Error> {
        self.trades
            .iter()
            .map(|trade| trade.score(prices, policies))
            .try_fold(num::Zero::zero(), |acc, score| {
                score.map(|score| acc + score)
            })
    }

    pub fn new(
        calldata: &super::transaction::CallData,
        domain_separator: &eth::DomainSeparator,
    ) -> Result<Self, Error> {
        let function = contracts::GPv2Settlement::raw_contract()
            .abi
            .function("settle")
            .unwrap();
        let data = calldata
            .0
             .0
            .strip_prefix(&function.selector())
            .ok_or(Error::InvalidSelector)?;

        let (calldata, metadata) = data.split_at(data.len() - Self::META_DATA_LEN);
        let metadata: Option<[u8; Self::META_DATA_LEN]> = metadata.try_into().ok();
        let auction_id = metadata
            .map(auction::Id::from_be_bytes)
            .ok_or(Error::MissingAuctionId)?;

        let tokenized = function
            .decode_input(calldata)
            .map_err(|err| EncodingError::Decoding(err).with(auction_id))?;
        let tokenized = <tokenized::Settlement>::from_token(web3::ethabi::Token::Tuple(tokenized))
            .map_err(|err| EncodingError::Tokenizing(err).with(auction_id))?;

        let (tokens, clearing_prices, decoded_trades, _interactions) = tokenized;

        let mut trades = Vec::with_capacity(decoded_trades.len());
        for trade in decoded_trades {
            let flags = TradeFlags(trade.8);
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
                boundary::order::order_uid(&trade, &tokens, domain_separator)
                    .map_err(|err| EncodingError::OrderUidRecover(err).with(auction_id))?,
                eth::Asset {
                    token: sell_token.into(),
                    amount: trade.3.into(),
                },
                eth::Asset {
                    token: buy_token.into(),
                    amount: trade.4.into(),
                },
                flags.order_kind(),
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

        Ok(Self { trades, auction_id })
    }

    pub fn auction_id(&self) -> auction::Id {
        self.auction_id
    }

    pub fn trades(&self) -> &[Trade] {
        &self.trades
    }
}

/// Trade flags are encoded in a 256-bit integer field. For more information on
/// how flags are encoded see:
/// <https://github.com/cowprotocol/contracts/blob/v1.0.0/src/contracts/libraries/GPv2Trade.sol#L58-L94>
#[derive(Debug, PartialEq, Eq)]
pub struct TradeFlags(pub U256);

impl TradeFlags {
    fn as_u8(&self) -> u8 {
        self.0.byte(0)
    }

    pub fn order_kind(&self) -> order::Side {
        if self.as_u8() & 0b1 == 0 {
            order::Side::Sell
        } else {
            order::Side::Buy
        }
    }

    pub fn partially_fillable(&self) -> bool {
        self.as_u8() & 0b10 != 0
    }

    pub fn sell_token_balance(&self) -> boundary::SellTokenSource {
        if self.as_u8() & 0x08 == 0 {
            boundary::SellTokenSource::Erc20
        } else if self.as_u8() & 0x04 == 0 {
            boundary::SellTokenSource::External
        } else {
            boundary::SellTokenSource::Internal
        }
    }

    pub fn buy_token_balance(&self) -> boundary::BuyTokenDestination {
        if self.as_u8() & 0x10 == 0 {
            boundary::BuyTokenDestination::Erc20
        } else {
            boundary::BuyTokenDestination::Internal
        }
    }

    pub fn signing_scheme(&self) -> boundary::SigningScheme {
        match (self.as_u8() >> 5) & 0b11 {
            0b00 => boundary::SigningScheme::Eip712,
            0b01 => boundary::SigningScheme::EthSign,
            0b10 => boundary::SigningScheme::Eip1271,
            0b11 => boundary::SigningScheme::PreSign,
            _ => unreachable!(),
        }
    }
}

impl From<U256> for TradeFlags {
    fn from(value: U256) -> Self {
        Self(value)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EncodingError {
    #[error("unable to decode settlement calldata: {0}")]
    Decoding(#[from] web3::ethabi::Error),
    #[error("unable to tokenize calldata into expected format: {0}")]
    Tokenizing(#[from] ethcontract::tokens::Error),
    #[error("unable to recover order uid: {0}")]
    OrderUidRecover(#[from] boundary::order::Error),
    #[error("no auction id found in calldata")]
    MissingAuctionId,
}

impl EncodingError {
    pub fn with(self, auction: auction::Id) -> Error {
        Error::Encoding(auction, self)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("transaction calldata is not a settlement")]
    InvalidSelector,
    #[error("no auction id found in calldata")]
    MissingAuctionId,
    #[error("auction {0} failed encoding: {1}")]
    Encoding(auction::Id, EncodingError),
}

impl Error {
    pub fn auction_id(&self) -> Option<auction::Id> {
        match self {
            Self::InvalidSelector => None,
            Self::MissingAuctionId => None,
            Self::Encoding(auction, _) => Some(*auction),
        }
    }
}
