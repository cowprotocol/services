//! This module defines Settlement as originated from a mined transaction
//! calldata.

use {
    crate::{
        boundary,
        domain::{
            auction::{self, order},
            eth,
        },
    },
    ethcontract::{common::FunctionExt, tokens::Tokenize, U256},
    trade::Trade,
};

mod tokenized;
mod trade;

/// Settlement originated from a calldata of a settlement transaction.
#[derive(Debug)]
#[allow(dead_code)]
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
    const META_DATA_LEN: usize = 8;

    pub fn native_surplus(&self, prices: &auction::Prices) -> Result<eth::Ether, trade::Error> {
        self.trades
            .iter()
            .map(|trade| trade.native_surplus(prices))
            .try_fold(num::Zero::zero(), |acc, score| {
                score.map(|score| acc + score)
            })
    }

    pub fn new(
        calldata: &eth::Calldata,
        domain_separator: &eth::DomainSeparator,
    ) -> Result<Self, Error> {
        let function = contracts::GPv2Settlement::raw_contract()
            .abi
            .function("settle")
            .unwrap();
        let data = calldata
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
            trades.push(trade::Trade::new(
                tokenized::order_uid(&trade, &tokens, domain_separator)
                    .map_err(|err| EncodingError::OrderUidRecover(err).with(auction_id))?,
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

        Ok(Self { trades, auction_id })
    }
}

/// Trade flags are encoded in a 256-bit integer field. For more information on
/// how flags are encoded see:
/// <https://github.com/cowprotocol/contracts/blob/v1.0.0/src/contracts/libraries/GPv2Trade.sol#L58-L94>
#[derive(Debug, PartialEq, Eq)]
struct TradeFlags(pub U256);

impl TradeFlags {
    fn as_u8(&self) -> u8 {
        self.0.byte(0)
    }

    pub fn side(&self) -> order::Side {
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
    OrderUidRecover(#[from] tokenized::Error),
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

mod tests {
    use {
        crate::domain::{auction, eth},
        hex_literal::hex,
    };

    // https://etherscan.io/tx/0xc48dc0d43ffb43891d8c3ad7bcf05f11465518a2610869b20b0b4ccb61497634
    #[test]
    fn settlement() {
        let calldata = hex!(
            "
        13d79a0b
        0000000000000000000000000000000000000000000000000000000000000080
        0000000000000000000000000000000000000000000000000000000000000120
        00000000000000000000000000000000000000000000000000000000000001c0
        00000000000000000000000000000000000000000000000000000000000003c0
        0000000000000000000000000000000000000000000000000000000000000004
        000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2
        000000000000000000000000c52fafdc900cb92ae01e6e4f8979af7f436e2eb2
        000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2
        000000000000000000000000c52fafdc900cb92ae01e6e4f8979af7f436e2eb2
        0000000000000000000000000000000000000000000000000000000000000004
        0000000000000000000000000000000000000000000000010000000000000000
        0000000000000000000000000000000000000000000000000023f003f04b5a92
        0000000000000000000000000000000000000000000000f676b2510588839eb6
        00000000000000000000000000000000000000000000000022b1c8c1227a0000
        0000000000000000000000000000000000000000000000000000000000000001
        0000000000000000000000000000000000000000000000000000000000000020
        0000000000000000000000000000000000000000000000000000000000000002
        0000000000000000000000000000000000000000000000000000000000000003
        0000000000000000000000009398a8948e1ac88432a509b218f9ac8cf9cecdee
        00000000000000000000000000000000000000000000000022b1c8c1227a0000
        0000000000000000000000000000000000000000000000f11f89f17728c24a5c
        00000000000000000000000000000000000000000000000000000000ffffffff
        ae848d463143d030dd3875930a875de6417f58adc5dde0e94d485706d34b4797
        0000000000000000000000000000000000000000000000000000000000000000
        0000000000000000000000000000000000000000000000000000000000000040
        00000000000000000000000000000000000000000000000022b1c8c1227a0000
        0000000000000000000000000000000000000000000000000000000000000160
        0000000000000000000000000000000000000000000000000000000000000028
        40a50cf069e992aa4536211b23f286ef8875218740a50cf069e992aa4536211b
        23f286ef88752187000000000000000000000000000000000000000000000000
        0000000000000000000000000000000000000000000000000000000000000060
        0000000000000000000000000000000000000000000000000000000000000140
        00000000000000000000000000000000000000000000000000000000000004c0
        0000000000000000000000000000000000000000000000000000000000000001
        0000000000000000000000000000000000000000000000000000000000000020
        00000000000000000000000040a50cf069e992aa4536211b23f286ef88752187
        0000000000000000000000000000000000000000000000000000000000000000
        0000000000000000000000000000000000000000000000000000000000000060
        0000000000000000000000000000000000000000000000000000000000000004
        4c84c1c800000000000000000000000000000000000000000000000000000000
        0000000000000000000000000000000000000000000000000000000000000003
        0000000000000000000000000000000000000000000000000000000000000060
        0000000000000000000000000000000000000000000000000000000000000140
        0000000000000000000000000000000000000000000000000000000000000220
        00000000000000000000000000000000be48a3000b818e9615d85aacfed4ca97
        0000000000000000000000000000000000000000000000000000000000000000
        0000000000000000000000000000000000000000000000000000000000000060
        000000000000000000000000000000000000000000000000000000000000004f
        0000000101010000000000000000063a508037887d5d5aca4b69771e56f3c92c
        20840dd09188a65771d8000000000000002c400000000000000001c02aaa39b2
        23fe8d0a0e5c4f27ead9083c756cc20000000000000000000000000000000000
        000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2
        0000000000000000000000000000000000000000000000000000000000000000
        0000000000000000000000000000000000000000000000000000000000000060
        0000000000000000000000000000000000000000000000000000000000000044
        a9059cbb000000000000000000000000c88deb1ce0bc4a4306b7f20be2abd28a
        d3a5c8d10000000000000000000000000000000000000000000000001c5efcf2
        c41873fd00000000000000000000000000000000000000000000000000000000
        000000000000000000000000c88deb1ce0bc4a4306b7f20be2abd28ad3a5c8d1
        0000000000000000000000000000000000000000000000000000000000000000
        0000000000000000000000000000000000000000000000000000000000000060
        00000000000000000000000000000000000000000000000000000000000000a4
        022c0d9f00000000000000000000000000000000000000000000000000000000
        000000000000000000000000000000000000000000000000000000ca2b0dae6c
        b90dbc4b0000000000000000000000009008d19f58aabd9ed0d60971565aa851
        0560ab4100000000000000000000000000000000000000000000000000000000
        0000008000000000000000000000000000000000000000000000000000000000
        0000000000000000000000000000000000000000000000000000000000000000
        0000000000000000000000000000000000000000000000000000000000000000
        000000000084120c"
        )
        .to_vec();

        let domain_separator = eth::DomainSeparator(hex!(
            "c078f884a2676e1345748b1feace7b0abee5d00ecadb6e574dcdd109a63e8943"
        ));
        let settlement = super::Settlement::new(&calldata.into(), &domain_separator).unwrap();
        assert_eq!(settlement.trades.len(), 1);

        // prices read from https://solver-instances.s3.eu-central-1.amazonaws.com/prod/mainnet/legacy/8655372.json
        let prices: auction::Prices = From::from([
            (
                eth::TokenAddress(eth::H160::from_slice(&hex!(
                    "c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"
                ))),
                auction::Price::new(eth::U256::from(1000000000000000000u128).into()).unwrap(),
            ),
            (
                eth::TokenAddress(eth::H160::from_slice(&hex!(
                    "c52fafdc900cb92ae01e6e4f8979af7f436e2eb2"
                ))),
                auction::Price::new(eth::U256::from(537359915436704u128).into()).unwrap(),
            ),
        ]);

        // surplus (score) read from https://api.cow.fi/mainnet/api/v1/solver_competition/by_tx_hash/0xc48dc0d43ffb43891d8c3ad7bcf05f11465518a2610869b20b0b4ccb61497634
        assert_eq!(
            settlement.native_surplus(&prices).unwrap().0,
            eth::U256::from(52937525819789126u128)
        );
    }
}
