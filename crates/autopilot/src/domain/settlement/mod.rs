//! Solvers propose solutions to an [`crate::domain::Auction`].
//!
//! A winning solution becomes a [`Settlement`] once it is executed on-chain in
//! a form of settlement transaction.

use {
    crate::{domain, domain::eth, infra},
    num::Saturating,
    std::collections::HashMap,
};

mod auction;
mod trade;
mod transaction;
pub use {auction::Auction, trade::Trade, transaction::Transaction};

/// A settled transaction together with the `Auction`, for which it was executed
/// on-chain.
///
/// Referenced as a [`Settlement`] in the codebase.
#[allow(dead_code)]
#[derive(Debug)]
pub struct Settlement {
    /// The gas used by the settlement transaction.
    gas: eth::Gas,
    /// The effective gas price of the settlement transaction.
    effective_gas_price: eth::EffectiveGasPrice,
    /// The address of the solver that submitted the settlement transaction.
    solver: eth::Address,
    /// The block number of the block that contains the settlement transaction.
    block: eth::BlockNo,
    /// The associated auction.
    auction: Auction,
    /// Trades that were settled by the transaction.
    trades: Vec<Trade>,
}

impl Settlement {
    /// The gas used by the settlement.
    pub fn gas(&self) -> eth::Gas {
        self.gas
    }

    /// The effective gas price at the time of settlement.
    pub fn gas_price(&self) -> eth::EffectiveGasPrice {
        self.effective_gas_price
    }

    /// Total surplus for all trades in the settlement.
    pub fn surplus_in_ether(&self) -> eth::Ether {
        self.trades
            .iter()
            .map(|trade| {
                trade
                    .surplus_in_ether(&self.auction.prices)
                    .unwrap_or_else(|err| {
                        tracing::warn!(
                            ?err,
                            "possible incomplete surplus calculation for trade {}",
                            trade.uid()
                        );
                        num::zero()
                    })
            })
            .sum()
    }

    /// Total fee taken for all the trades in the settlement.
    pub fn fee_in_ether(&self) -> eth::Ether {
        self.trades
            .iter()
            .map(|trade| {
                trade
                    .fee_in_ether(&self.auction.prices)
                    .unwrap_or_else(|err| {
                        tracing::warn!(
                            ?err,
                            "possible incomplete fee calculation for trade {}",
                            trade.uid()
                        );
                        num::zero()
                    })
            })
            .sum()
    }

    /// Per order fees breakdown. Contains all orders from the settlement
    pub fn order_fees(&self) -> HashMap<domain::OrderUid, Option<trade::ExecutedFee>> {
        self.trades
            .iter()
            .map(|trade| {
                (*trade.uid(), {
                    let total = trade.fee_in_sell_token();
                    let protocol = trade.protocol_fees_in_sell_token(&self.auction);
                    match (total, protocol) {
                        (Ok(total), Ok(protocol)) => {
                            let network =
                                total.saturating_sub(protocol.iter().map(|(fee, _)| *fee).sum());
                            Some(trade::ExecutedFee { protocol, network })
                        }
                        _ => None,
                    }
                })
            })
            .collect()
    }

    /// Return all trades that are classified as Just-In-Time (JIT) orders.
    pub fn jit_orders(&self) -> Vec<&trade::Jit> {
        self.trades
            .iter()
            .filter_map(|trade| trade.as_jit())
            .collect()
    }

    pub async fn new(
        settled: Transaction,
        persistence: &infra::Persistence,
    ) -> Result<Self, Error> {
        if persistence
            .auction_has_settlement(settled.auction_id)
            .await?
        {
            // This settlement has already been processed by another environment.
            //
            // TODO: remove once https://github.com/cowprotocol/services/issues/2848 is resolved and ~270 days are passed since bumping.
            return Err(Error::WrongEnvironment);
        }

        let auction = persistence.get_auction(settled.auction_id).await?;
        let database_orders = persistence.orders_that_exist(&settled.order_uids()).await?;

        let trades = settled
            .trades
            .into_iter()
            .map(|trade| Trade::new(trade, &auction, &database_orders, settled.timestamp))
            .collect();

        Ok(Self {
            solver: settled.solver,
            block: settled.block,
            gas: settled.gas,
            effective_gas_price: settled.effective_gas_price,
            trades,
            auction,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed communication with the database: {0}")]
    Infra(anyhow::Error),
    #[error("failed to prepare the data fetched from database for domain: {0}")]
    InconsistentData(InconsistentData),
    #[error("settlement refers to an auction from a different environment")]
    WrongEnvironment,
}

/// Errors that can occur when fetching data from the persistence layer.
///
/// These errors cover missing data, conversion of data into domain objects etc.
///
/// This is a separate enum to allow for more specific error handling.
#[derive(Debug, thiserror::Error)]
pub enum InconsistentData {
    #[error("auction not found in the persistence layer")]
    AuctionNotFound,
    #[error("proposed solution not found in the persistence layer")]
    SolutionNotFound,
    #[error("invalid fee policy fetched from persistence layer: {0} for order: {1}")]
    InvalidFeePolicy(infra::persistence::dto::fee_policy::Error, domain::OrderUid),
    #[error("invalid fetched price from persistence layer for token: {0:?}")]
    InvalidPrice(eth::TokenAddress),
    #[error(
        "invalid score fetched from persistence layer for a coresponding competition solution, \
         err: {0}"
    )]
    InvalidScore(anyhow::Error),
    #[error("invalid solver competition data fetched from persistence layer: {0}")]
    InvalidSolverCompetition(anyhow::Error),
}

impl From<infra::persistence::error::Auction> for Error {
    fn from(err: infra::persistence::error::Auction) -> Self {
        match err {
            infra::persistence::error::Auction::DatabaseError(err) => Self::Infra(err.into()),
            infra::persistence::error::Auction::NotFound => {
                Self::InconsistentData(InconsistentData::AuctionNotFound)
            }
            infra::persistence::error::Auction::InvalidFeePolicy(err, order) => {
                Self::InconsistentData(InconsistentData::InvalidFeePolicy(err, order))
            }
            infra::persistence::error::Auction::InvalidPrice(token) => {
                Self::InconsistentData(InconsistentData::InvalidPrice(token))
            }
        }
    }
}

impl From<infra::persistence::error::Solution> for Error {
    fn from(err: infra::persistence::error::Solution) -> Self {
        match err {
            infra::persistence::error::Solution::DatabaseError(err) => Self::Infra(err.into()),
            infra::persistence::error::Solution::NotFound => {
                Self::InconsistentData(InconsistentData::SolutionNotFound)
            }
            infra::persistence::error::Solution::InvalidScore(err) => {
                Self::InconsistentData(InconsistentData::InvalidScore(err))
            }
            infra::persistence::error::Solution::InvalidSolverCompetition(err) => {
                Self::InconsistentData(InconsistentData::InvalidSolverCompetition(err))
            }
            infra::persistence::error::Solution::InvalidPrice(_) => todo!(),
        }
    }
}

impl From<infra::persistence::DatabaseError> for Error {
    fn from(err: infra::persistence::DatabaseError) -> Self {
        Self::Infra(err.0)
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
        std::collections::{HashMap, HashSet},
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
        let transaction = super::transaction::Transaction::new(
            &domain::eth::Transaction {
                input: calldata.into(),
                ..Default::default()
            },
            &domain_separator,
        )
        .unwrap();

        let order_uid = transaction.trades[0].uid;

        let auction = super::Auction {
            // prices read from https://solver-instances.s3.eu-central-1.amazonaws.com/prod/mainnet/legacy/8655372.json
            prices: auction::Prices::from([
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
            ]),
            surplus_capturing_jit_order_owners: Default::default(),
            id: 0,
            orders: HashMap::from([(order_uid, vec![])]),
        };
        let database_orders = HashSet::from([order_uid]);

        let trade =
            super::trade::Trade::new(transaction.trades[0].clone(), &auction, &database_orders, 0);

        // surplus (score) read from https://api.cow.fi/mainnet/api/v1/solver_competition/by_tx_hash/0xc48dc0d43ffb43891d8c3ad7bcf05f11465518a2610869b20b0b4ccb61497634
        assert_eq!(
            trade.surplus_in_ether(&auction.prices).unwrap().0,
            eth::U256::from(52937525819789126u128)
        );
        // fee read from "executedSurplusFee" https://api.cow.fi/mainnet/api/v1/orders/0x10dab31217bb6cc2ace0fe601c15d342f7626a1ee5ef0495449800e73156998740a50cf069e992aa4536211b23f286ef88752187ffffffff
        assert_eq!(
            trade.fee_in_ether(&auction.prices).unwrap().0,
            eth::U256::from(6890975030480504u128)
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
        let transaction = super::transaction::Transaction::new(
            &domain::eth::Transaction {
                input: calldata.into(),
                ..Default::default()
            },
            &domain_separator,
        )
        .unwrap();

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

        let order_uid = transaction.trades[0].uid;
        let auction = super::Auction {
            prices,
            surplus_capturing_jit_order_owners: Default::default(),
            id: 0,
            orders: HashMap::from([(
                order_uid,
                vec![domain::fee::Policy::Surplus {
                    factor: 0.5f64.try_into().unwrap(),
                    max_volume_factor: 0.01.try_into().unwrap(),
                }],
            )]),
        };
        let database_orders = HashSet::from([order_uid]);
        let trade =
            super::trade::Trade::new(transaction.trades[0].clone(), &auction, &database_orders, 0);

        assert_eq!(
            trade.surplus_in_ether(&auction.prices).unwrap().0,
            eth::U256::from(384509480572312u128)
        );

        assert_eq!(
            trade.score(&auction).unwrap().0,
            eth::U256::from(769018961144624u128) // 2 x surplus
        );
    }
}
