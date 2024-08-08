//! This module defines Solution as originated from a mined transaction
//! calldata.

use {
    self::trade::Trade,
    crate::domain::{
        auction::{self},
        competition,
        eth,
    },
};

mod tokenized;
mod trade;
pub use error::Error;
use {crate::domain, std::collections::HashMap};

/// A solution that was executed on-chain.
///
/// Contains only data observable on-chain. No off-chain data is used to create
/// this struct.
///
/// Referenced as [`settlement::Solution`] in the codebase.
#[derive(Debug)]
pub struct Solution {
    trades: Vec<Trade>,
    /// Data that was appended to the regular call data of the `settle()` call
    /// as a form of on-chain meta data. This is used to associate a
    /// solution with an auction for which this solution was picked as a winner.
    auction_id: auction::Id,
}

impl Solution {
    pub fn auction_id(&self) -> auction::Id {
        self.auction_id
    }

    /// CIP38 score calculation
    pub fn score(&self, auction: &super::Auction) -> Result<competition::Score, error::Score> {
        Ok(competition::Score::new(
            self.trades
                .iter()
                .map(|trade| trade.score(auction))
                .sum::<Result<eth::Ether, trade::Error>>()?,
        )?)
    }

    /// Total surplus for all trades in the solution.
    ///
    /// Always returns a value, even if some trades have incomplete surplus
    /// calculation.
    pub fn native_surplus(&self, auction: &super::Auction) -> eth::Ether {
        self.trades
            .iter()
            .map(|trade| {
                trade.native_surplus(auction).unwrap_or_else(|err| {
                    tracing::warn!(
                        ?err,
                        "possible incomplete surplus calculation for trade {}",
                        trade.order_uid()
                    );
                    num::zero()
                })
            })
            .sum()
    }

    /// Total fee for all trades in the solution.
    ///
    /// Always returns a value, even if some trades have incomplete fee
    /// calculation.
    pub fn native_fee(&self, prices: &auction::Prices) -> eth::Ether {
        self.trades
            .iter()
            .map(|trade| {
                trade.native_fee(prices).unwrap_or_else(|err| {
                    tracing::warn!(
                        ?err,
                        "possible incomplete fee calculation for trade {}",
                        trade.order_uid()
                    );
                    num::zero()
                })
            })
            .sum()
    }

    /// Returns fees denominated in sell token for each order in the solution.
    pub fn fees(
        &self,
        prices: &auction::Prices,
    ) -> HashMap<domain::OrderUid, Option<eth::SellTokenAmount>> {
        self.trades
            .iter()
            .map(|trade| (*trade.order_uid(), trade.fee_in_sell_token(prices).ok()))
            .collect()
    }

    pub fn new(
        calldata: &eth::Calldata,
        domain_separator: &eth::DomainSeparator,
    ) -> Result<Self, Error> {
        let tokenized::Tokenized {
            tokens,
            clearing_prices,
            trades: decoded_trades,
            interactions: _interactions,
            auction_id,
        } = tokenized::Tokenized::new(calldata)?;

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
            trades.push(trade::Trade::new(
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
                        sell: clearing_prices[uniform_sell_token_index].into(),
                        buy: clearing_prices[uniform_buy_token_index].into(),
                    },
                    custom: trade::ClearingPrices {
                        sell: clearing_prices[sell_token_index].into(),
                        buy: clearing_prices[buy_token_index].into(),
                    },
                },
            ));
        }

        Ok(Self { trades, auction_id })
    }
}

pub mod error {
    use super::*;

    #[derive(Debug, thiserror::Error)]
    pub enum Error {
        #[error(transparent)]
        Decoding(#[from] tokenized::error::Decoding),
        #[error("failed to recover order uid {0} for auction {1}")]
        OrderUidRecover(tokenized::error::Uid, auction::Id),
    }

    impl Error {
        pub fn auction_id(&self) -> Option<auction::Id> {
            match self {
                Self::Decoding(err) => err.auction_id(),
                Self::OrderUidRecover(_, auction_id) => Some(*auction_id),
            }
        }
    }

    #[derive(Debug, thiserror::Error)]
    pub enum Score {
        /// Per CIP38, zero score solutions are rejected.
        #[error(transparent)]
        Zero(#[from] competition::ZeroScore),
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

#[cfg(test)]
mod tests {
    use {
        crate::{
            domain,
            domain::{auction, eth},
        },
        hex_literal::hex,
        std::collections::HashMap,
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
        let solution = super::Solution::new(&calldata.into(), &domain_separator).unwrap();
        assert_eq!(solution.trades.len(), 1);

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

        let auction = super::super::Auction {
            prices,
            surplus_capturing_jit_order_owners: vec![],
            id: 0,
            orders: HashMap::from([(domain::OrderUid(hex!("10dab31217bb6cc2ace0fe601c15d342f7626a1ee5ef0495449800e73156998740a50cf069e992aa4536211b23f286ef88752187ffffffff")), vec![])]),
        };

        // surplus (score) read from https://api.cow.fi/mainnet/api/v1/solver_competition/by_tx_hash/0xc48dc0d43ffb43891d8c3ad7bcf05f11465518a2610869b20b0b4ccb61497634
        assert_eq!(
            solution.native_surplus(&auction).0,
            eth::U256::from(52937525819789126u128)
        );
        // fee read from "executedSurplusFee" https://api.cow.fi/mainnet/api/v1/orders/0x10dab31217bb6cc2ace0fe601c15d342f7626a1ee5ef0495449800e73156998740a50cf069e992aa4536211b23f286ef88752187ffffffff
        // "executedSurplusFee" and native fee are equal because the sell token is ETH
        assert_eq!(
            solution.native_fee(&auction.prices).0,
            eth::U256::from(6752697350740628u128)
        );
        // fee read from "executedSurplusFee" https://api.cow.fi/mainnet/api/v1/orders/0x10dab31217bb6cc2ace0fe601c15d342f7626a1ee5ef0495449800e73156998740a50cf069e992aa4536211b23f286ef88752187ffffffff
        assert_eq!(
            solution.fees(&auction.prices),
            HashMap::from([(domain::OrderUid(hex!("10dab31217bb6cc2ace0fe601c15d342f7626a1ee5ef0495449800e73156998740a50cf069e992aa4536211b23f286ef88752187ffffffff")), Some(eth::SellTokenAmount(eth::U256::from(6752697350740628u128))))])
        );
    }

    // https://etherscan.io/tx/0x688508eb59bd20dc8c0d7c0c0b01200865822c889f0fcef10113e28202783243
    #[test]
    fn settlement_with_protocol_fee() {
        let calldata = hex!(
            "
        13d79a0b
        0000000000000000000000000000000000000000000000000000000000000080
        0000000000000000000000000000000000000000000000000000000000000120
        00000000000000000000000000000000000000000000000000000000000001c0
        00000000000000000000000000000000000000000000000000000000000003e0
        0000000000000000000000000000000000000000000000000000000000000004
        000000000000000000000000056fd409e1d7a124bd7017459dfea2f387b6d5cd
        000000000000000000000000dac17f958d2ee523a2206206994597c13d831ec7
        000000000000000000000000dac17f958d2ee523a2206206994597c13d831ec7
        000000000000000000000000056fd409e1d7a124bd7017459dfea2f387b6d5cd
        0000000000000000000000000000000000000000000000000000000000000004
        00000000000000000000000000000000000000000000000000000019b743b945
        0000000000000000000000000000000000000000000000000000000000a87cf3
        0000000000000000000000000000000000000000000000000000000000a87c7c
        00000000000000000000000000000000000000000000000000000019b8b69873
        0000000000000000000000000000000000000000000000000000000000000001
        0000000000000000000000000000000000000000000000000000000000000020
        0000000000000000000000000000000000000000000000000000000000000002
        0000000000000000000000000000000000000000000000000000000000000003
        000000000000000000000000f87da2093abee9b13a6f89671e4c3a3f80b42767
        0000000000000000000000000000000000000000000000000000006d6e2edc00
        0000000000000000000000000000000000000000000000000000000002cccdff
        000000000000000000000000000000000000000000000000000000006799c219
        2d365e5affcfa62cf1067b845add9c01bedcb2fc5d7a37442d2177262af26a0c
        0000000000000000000000000000000000000000000000000000000000000000
        0000000000000000000000000000000000000000000000000000000000000002
        00000000000000000000000000000000000000000000000000000019b8b69873
        0000000000000000000000000000000000000000000000000000000000000160
        0000000000000000000000000000000000000000000000000000000000000041
        e2ef661343676f9f4371ce809f728bb39a406f47835ee2b0104a8a1f340409ae
        742dfe47fe469c024dc2fb7f80b99878b35985d66312856a8b5dcf5de4b069ee
        1c00000000000000000000000000000000000000000000000000000000000000
        0000000000000000000000000000000000000000000000000000000000000060
        0000000000000000000000000000000000000000000000000000000000000080
        0000000000000000000000000000000000000000000000000000000000000520
        0000000000000000000000000000000000000000000000000000000000000000
        0000000000000000000000000000000000000000000000000000000000000003
        0000000000000000000000000000000000000000000000000000000000000060
        0000000000000000000000000000000000000000000000000000000000000140
        00000000000000000000000000000000000000000000000000000000000002e0
        000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48
        0000000000000000000000000000000000000000000000000000000000000000
        0000000000000000000000000000000000000000000000000000000000000060
        0000000000000000000000000000000000000000000000000000000000000044
        095ea7b3000000000000000000000000e592427a0aece92de3edee1f18e0157c
        05861564ffffffffffffffffffffffffffffffffffffffffffffffffffffffff
        ffffffff00000000000000000000000000000000000000000000000000000000
        000000000000000000000000e592427a0aece92de3edee1f18e0157c05861564
        0000000000000000000000000000000000000000000000000000000000000000
        0000000000000000000000000000000000000000000000000000000000000060
        0000000000000000000000000000000000000000000000000000000000000104
        db3e2198000000000000000000000000dac17f958d2ee523a2206206994597c1
        3d831ec7000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce
        3606eb4800000000000000000000000000000000000000000000000000000000
        000001f40000000000000000000000009008d19f58aabd9ed0d60971565aa851
        0560ab4100000000000000000000000000000000000000000000000000000000
        66abb94e00000000000000000000000000000000000000000000000000000019
        b4b64b9b00000000000000000000000000000000000000000000000000000019
        bdd90a1800000000000000000000000000000000000000000000000000000000
        0000000000000000000000000000000000000000000000000000000000000000
        000000000000000000000000e592427a0aece92de3edee1f18e0157c05861564
        0000000000000000000000000000000000000000000000000000000000000000
        0000000000000000000000000000000000000000000000000000000000000060
        0000000000000000000000000000000000000000000000000000000000000104
        db3e2198000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce
        3606eb48000000000000000000000000056fd409e1d7a124bd7017459dfea2f3
        87b6d5cd00000000000000000000000000000000000000000000000000000000
        000001f40000000000000000000000009008d19f58aabd9ed0d60971565aa851
        0560ab4100000000000000000000000000000000000000000000000000000000
        66abb94e00000000000000000000000000000000000000000000000000000000
        00a87cf300000000000000000000000000000000000000000000000000000019
        bb4af52700000000000000000000000000000000000000000000000000000000
        0000000000000000000000000000000000000000000000000000000000000000
        0000000000000000000000000000000000000000000000000000000000000000
        00000000008c912c"
        )
        .to_vec();

        let domain_separator = eth::DomainSeparator(hex!(
            "c078f884a2676e1345748b1feace7b0abee5d00ecadb6e574dcdd109a63e8943"
        ));
        let solution = super::Solution::new(&calldata.into(), &domain_separator).unwrap();
        assert_eq!(solution.trades.len(), 1);

        let prices: auction::Prices = From::from([
            (
                eth::TokenAddress(eth::H160::from_slice(&hex!(
                    "dac17f958d2ee523a2206206994597c13d831ec7"
                ))),
                auction::Price::new(eth::U256::from(321341140475275961528483840u128).into())
                    .unwrap(),
            ),
            (
                eth::TokenAddress(eth::H160::from_slice(&hex!(
                    "056fd409e1d7a124bd7017459dfea2f387b6d5cd"
                ))),
                auction::Price::new(eth::U256::from(3177764302250520038326415654912u128).into())
                    .unwrap(),
            ),
        ]);

        let auction = super::super::Auction {
            prices,
            surplus_capturing_jit_order_owners: vec![],
            id: 0,
            orders: HashMap::from([(domain::OrderUid(hex!("c6a81144bc822569a0752c7a537fa9cbbf6344cb187ce0ff15a534b571e277eaf87da2093abee9b13a6f89671e4c3a3f80b427676799c219")), vec![domain::fee::Policy::Surplus {
                factor: 0.5f64.try_into().unwrap(),
                max_volume_factor: 0.01.try_into().unwrap(),
            }])]),
        };

        assert_eq!(
            solution.native_surplus(&auction).0,
            eth::U256::from(384509480572312u128)
        );

        assert_eq!(
            solution.score(&auction).unwrap().get().0,
            eth::U256::from(769018961144624u128) // 2 x surplus
        );
    }
}
