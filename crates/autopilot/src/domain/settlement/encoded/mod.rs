//! This module defines Settlement in an onchain compatible format.

use {
    crate::{
        boundary,
        domain::{
            self,
            auction::{self, order},
            eth,
        },
    },
    ethcontract::{common::FunctionExt, tokens::Tokenize, Address, Bytes, U256},
};

pub mod tokenized;

/// Settlement in an encoded format, as expected by the settlement contract
/// `settle` function.
///
/// Type safe representation of the settlement transaction calldata.
#[derive(Debug)]
pub struct Encoded {
    trades: Vec<Trade>,
    _interactions: [Vec<Interaction>; 3],
    /// Data that was appended to the regular call data of the `settle()` call
    /// as a form of on-chain meta data. This gets used to associated a
    /// settlement with an auction.
    auction_id: auction::Id,
}

impl Encoded {
    /// Number of bytes that may be appended to the calldata to store an auction
    /// id.
    pub const META_DATA_LEN: usize = 8;

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

        let (tokens, clearing_prices, decoded_trades, interactions) = tokenized;

        let mut trades = Vec::with_capacity(decoded_trades.len());
        for trade in decoded_trades {
            let flags = TradeFlags(trade.8);
            let signature = boundary::Signature::from_bytes(flags.signing_scheme(), &trade.10 .0)
                .map_err(boundary::order::Error::Signature)
                .map_err(|err| EncodingError::OrderUidRecover(err).with(auction_id))?;

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
            trades.push(Trade {
                order_uid: boundary::order::order_uid(&trade, &tokens, domain_separator)
                    .map_err(|err| EncodingError::OrderUidRecover(err).with(auction_id))?,
                sell: eth::Asset {
                    token: sell_token.into(),
                    amount: trade.3.into(),
                },
                buy: eth::Asset {
                    token: buy_token.into(),
                    amount: trade.4.into(),
                },
                receiver: trade.2.into(),
                valid_to: trade.5,
                app_data: order::AppDataHash(trade.6 .0),
                flags,
                executed: trade.9.into(),
                signature: signature.into(),
                prices: Price {
                    uniform: ClearingPrices {
                        sell: clearing_prices[uniform_sell_token_index],
                        buy: clearing_prices[uniform_buy_token_index],
                    },
                    custom: ClearingPrices {
                        sell: clearing_prices[sell_token_index],
                        buy: clearing_prices[buy_token_index],
                    },
                },
            })
        }
        let _interactions = interactions.map(|inner| inner.into_iter().map(Into::into).collect());

        Ok(Self {
            trades,
            _interactions,
            auction_id,
        })
    }

    pub fn auction_id(&self) -> auction::Id {
        self.auction_id
    }

    pub fn trades(&self) -> &[Trade] {
        &self.trades
    }
}

#[derive(Debug)]
pub struct Trade {
    pub sell: eth::Asset,
    pub buy: eth::Asset,
    pub receiver: eth::Address,
    pub valid_to: u32,
    pub app_data: order::AppDataHash,
    pub flags: TradeFlags,
    pub executed: eth::TargetAmount,
    pub signature: order::Signature,

    /// [ Additional derived fields ]
    ///
    /// The order uid of the order associated with this trade.
    pub order_uid: domain::OrderUid,
    /// Derived from the settlement "clearing_prices" vector
    pub prices: Price,
}

impl Trade {
    /// Surplus based on uniform clearing prices returns the surplus without any
    /// fees applied.
    pub fn surplus_before_fee(&self) -> Option<eth::Asset> {
        super::surplus::trade_surplus(
            self.flags.order_kind(),
            self.executed,
            self.sell,
            self.buy,
            &self.prices.uniform,
        )
    }

    /// Surplus based on custom clearing prices returns the surplus after fees
    /// have been applied.
    pub fn surplus(&self) -> Option<eth::Asset> {
        super::surplus::trade_surplus(
            self.flags.order_kind(),
            self.executed,
            self.sell,
            self.buy,
            &self.prices.custom,
        )
    }
}

#[derive(Debug)]
pub struct Price {
    pub uniform: ClearingPrices,
    /// Adjusted uniform prices to account for fees (gas cost and protocol fees)
    pub custom: ClearingPrices,
}

/// Uniform clearing prices at which the trade was executed.
#[derive(Debug, Clone, Copy)]
pub struct ClearingPrices {
    pub sell: eth::U256,
    pub buy: eth::U256,
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

    pub fn order_kind(&self) -> order::Kind {
        if self.as_u8() & 0b1 == 0 {
            order::Kind::Sell
        } else {
            order::Kind::Buy
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

#[derive(Debug, PartialEq, Eq)]
pub struct Interaction {
    pub target: Address,
    pub value: U256,
    pub call_data: Bytes<Vec<u8>>,
}

impl From<(Address, U256, Bytes<Vec<u8>>)> for Interaction {
    fn from((target, value, call_data): (Address, U256, Bytes<Vec<u8>>)) -> Self {
        Self {
            target,
            value,
            call_data,
        }
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
