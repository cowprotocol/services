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
    tokens: Vec<eth::Address>,
    clearing_prices: Vec<eth::U256>,
    trades: Vec<Trade>,
    interactions: [Vec<Interaction>; 3],
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
        let tokenized = function.decode_input(calldata)?;
        let tokenized = <tokenized::Settlement>::from_token(web3::ethabi::Token::Tuple(tokenized))?;

        let (tokens, clearing_prices, decoded_trades, interactions) = tokenized;

        let mut trades = Vec::with_capacity(decoded_trades.len());
        for trade in decoded_trades {
            let flags = TradeFlags(trade.8);
            let signature = boundary::Signature::from_bytes(flags.signing_scheme(), &trade.10 .0)
                .map_err(boundary::order::Error::Signature)?;

            trades.push(Trade {
                order_uid: boundary::order::order_uid(&trade, &tokens, &domain_separator)?,
                sell_token_index: trade.0.as_usize(),
                buy_token_index: trade.1.as_usize(),
                receiver: trade.2.into(),
                sell_amount: trade.3.into(),
                buy_amount: trade.4.into(),
                valid_to: trade.5,
                app_data: order::AppDataHash(trade.6 .0),
                fee_amount: trade.7.into(),
                flags,
                executed: trade.9.into(),
                signature: signature.into(),
            })
        }
        let tokens: Vec<eth::Address> = tokens.into_iter().map(Into::into).collect();
        let interactions = interactions.map(|inner| inner.into_iter().map(Into::into).collect());
        let metadata: Option<[u8; Self::META_DATA_LEN]> = metadata.try_into().ok();
        let auction_id = metadata
            .map(auction::Id::from_be_bytes)
            .ok_or(Error::MissingAuctionId)?;

        Ok(Self {
            tokens,
            clearing_prices,
            trades,
            interactions,
            auction_id,
        })
    }

    pub fn auction_id(&self) -> auction::Id {
        self.auction_id
    }

    pub fn trades(&self) -> &[Trade] {
        &self.trades
    }

    pub fn tokens(&self) -> &[eth::Address] {
        &self.tokens
    }

    pub fn clearing_prices(&self) -> &[eth::U256] {
        &self.clearing_prices
    }
}

#[derive(Debug)]
pub struct Trade {
    pub sell_token_index: usize,
    pub buy_token_index: usize,
    pub receiver: eth::Address,
    pub sell_amount: eth::TokenAmount,
    pub buy_amount: eth::TokenAmount,
    pub valid_to: u32,
    pub app_data: order::AppDataHash,
    pub fee_amount: eth::TokenAmount,
    pub flags: TradeFlags,
    pub executed: eth::TargetAmount,
    pub signature: order::Signature,

    /// [ Additional derived fields ]
    ///
    /// The order uid of the order associated with this trade.
    pub order_uid: domain::OrderUid,
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
pub enum Error {
    #[error("transaction calldata is not a settlement")]
    InvalidSelector,
    #[error("unable to decode settlement calldata: {0}")]
    Decoding(#[from] web3::ethabi::Error),
    #[error("unable to tokenize calldata into expected format: {0}")]
    Tokenizing(#[from] ethcontract::tokens::Error),
    #[error("unable to recover order uid: {0}")]
    OrderUidRecover(#[from] boundary::order::Error),
    #[error("no auction id found in calldata")]
    MissingAuctionId,
}
