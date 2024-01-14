//! This module contains the logic for decoding the function input for
//! GPv2Settlement::settle function.

use {
    crate::{
        boundary,
        domain::{self, eth},
    },
    anyhow::{Context, Result},
    contracts::GPv2Settlement,
    ethcontract::{common::FunctionExt, tokens::Tokenize, Address, Bytes, U256},
    model::{
        app_data::AppDataHash,
        order::{BuyTokenDestination, OrderData, OrderKind, SellTokenSource},
        signature::SigningScheme,
    },
    web3::ethabi::Token,
};

// Original type for input of `GPv2Settlement.settle` function.
type DecodedSettlementTokenized = (
    Vec<Address>,
    Vec<U256>,
    Vec<(
        U256,            // sellTokenIndex
        U256,            // buyTokenIndex
        Address,         // receiver
        U256,            // sellAmount
        U256,            // buyAmount
        u32,             // validTo
        Bytes<[u8; 32]>, // appData
        U256,            // feeAmount
        U256,            // flags
        U256,            // executedAmount
        Bytes<Vec<u8>>,  // signature
    )>,
    [Vec<(Address, U256, Bytes<Vec<u8>>)>; 3],
);

#[derive(Debug)]
pub struct Encoded {
    tokens: Vec<eth::Address>,
    clearing_prices: Vec<eth::U256>,
    trades: Vec<Trade>,
    interactions: [Vec<Interaction>; 3],
    /// Data that was appended to the regular call data of the `settle()` call
    /// as a form of on-chain meta data. This gets used to associated a
    /// settlement with an auction.
    auction_id: i64,
}

#[derive(Debug)]
struct Trade {
    sell_token_index: eth::U256,
    buy_token_index: eth::U256,
    receiver: eth::Address,
    sell_amount: eth::U256,
    buy_amount: eth::U256,
    valid_to: u32,
    app_data: domain::auction::order::AppDataHash,
    fee_amount: eth::U256,
    flags: TradeFlags,
    executed_amount: eth::U256,
    signature: domain::auction::order::Signature,

    /// [ Additional derived fields ]
    ///
    /// The order uid of the order associated with this trade.
    order_uid: domain::OrderUid,
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

    fn order_kind(&self) -> OrderKind {
        if self.as_u8() & 0b1 == 0 {
            OrderKind::Sell
        } else {
            OrderKind::Buy
        }
    }

    fn partially_fillable(&self) -> bool {
        self.as_u8() & 0b10 != 0
    }

    fn sell_token_balance(&self) -> SellTokenSource {
        if self.as_u8() & 0x08 == 0 {
            SellTokenSource::Erc20
        } else if self.as_u8() & 0x04 == 0 {
            SellTokenSource::External
        } else {
            SellTokenSource::Internal
        }
    }

    fn buy_token_balance(&self) -> BuyTokenDestination {
        if self.as_u8() & 0x10 == 0 {
            BuyTokenDestination::Erc20
        } else {
            BuyTokenDestination::Internal
        }
    }

    fn signing_scheme(&self) -> SigningScheme {
        match (self.as_u8() >> 5) & 0b11 {
            0b00 => SigningScheme::Eip712,
            0b01 => SigningScheme::EthSign,
            0b10 => SigningScheme::Eip1271,
            0b11 => SigningScheme::PreSign,
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

impl Encoded {
    /// Number of bytes that may be appended to the calldata to store an auction
    /// id.
    pub const META_DATA_LEN: usize = 8;

    pub fn new(
        call_data: &domain::settlement::transaction::CallData,
        domain_separator: eth::DomainSeparator,
    ) -> Result<Self> {
        let function = GPv2Settlement::raw_contract()
            .abi
            .function("settle")
            .unwrap();
        let data = call_data
            .0
             .0
            .strip_prefix(&function.selector())
            .ok_or(DecodingError::InvalidSelector)?;

        let (calldata, metadata) = data.split_at(data.len() - Self::META_DATA_LEN);
        let tokenized = function
            .decode_input(calldata)
            .context("tokenizing settlement calldata failed")?;
        let decoded = <DecodedSettlementTokenized>::from_token(Token::Tuple(tokenized))
            .context("decoding tokenized settlement calldata failed")?;

        let (tokens, clearing_prices, decoded_trades, interactions) = decoded;
        let tokens: Vec<eth::Address> = tokens.into_iter().map(Into::into).collect();
        let mut trades = Vec::with_capacity(decoded_trades.len());
        for trade in decoded_trades {
            let sell_token_index = trade.0;
            let buy_token_index = trade.1;
            let receiver: eth::Address = trade.2.into();
            let sell_amount = trade.3;
            let buy_amount = trade.4;
            let valid_to = trade.5;
            let app_data = domain::auction::order::AppDataHash(trade.6 .0);
            let fee_amount = trade.7;
            let flags = TradeFlags(trade.8);
            let executed_amount = trade.9;
            let signature =
                boundary::Signature::from_bytes(flags.signing_scheme(), &trade.10 .0).unwrap();

            let order_uid = {
                let order = OrderData {
                    sell_token: tokens[sell_token_index.as_u64() as usize].into(),
                    buy_token: tokens[buy_token_index.as_u64() as usize].into(),
                    sell_amount,
                    buy_amount,
                    valid_to,
                    app_data: AppDataHash(app_data.0),
                    fee_amount,
                    kind: flags.order_kind(),
                    partially_fillable: flags.partially_fillable(),
                    receiver: Some(receiver.into()),
                    sell_token_balance: flags.sell_token_balance(),
                    buy_token_balance: flags.buy_token_balance(),
                };
                let domain_separator = boundary::DomainSeparator(domain_separator.0);
                let owner = signature
                    .recover_owner(
                        &signature.to_bytes(),
                        &domain_separator,
                        &order.hash_struct(),
                    )
                    .context("cant recover owner")?;
                order.uid(&domain_separator, &owner)
            };

            trades.push(Trade {
                order_uid: order_uid.into(),
                sell_token_index,
                buy_token_index,
                receiver,
                sell_amount,
                buy_amount,
                valid_to,
                app_data,
                fee_amount,
                flags,
                executed_amount,
                signature: signature.into(),
            })
        }
        let interactions = interactions.map(|inner| inner.into_iter().map(Into::into).collect());
        let metadata: Option<[u8; Self::META_DATA_LEN]> = metadata.try_into().ok();
        let auction_id = metadata
            .map(i64::from_be_bytes)
            .ok_or(DecodingError::Other(anyhow::anyhow!(
                "failed to decode auction id from metadata"
            )))?;

        Ok(Self {
            tokens,
            clearing_prices,
            trades,
            interactions,
            auction_id,
        })
    }
}

#[derive(Debug)]
pub enum DecodingError {
    InvalidSelector,
    Other(anyhow::Error),
}

impl From<anyhow::Error> for DecodingError {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err)
    }
}

impl From<DecodingError> for anyhow::Error {
    fn from(err: DecodingError) -> Self {
        match err {
            DecodingError::InvalidSelector => anyhow::anyhow!("invalid function selector"),
            DecodingError::Other(err) => err,
        }
    }
}
