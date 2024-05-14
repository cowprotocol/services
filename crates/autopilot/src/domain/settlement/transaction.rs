//! Domain types for on-chain transactions that settle settlements.

use {
    super::Settlement,
    crate::{
        domain::{auction, competition, eth},
        infra,
    },
};

/// A transaction that settles a settlement. Interacts with the settlement
/// contract `settle` function.
#[derive(Debug)]
pub struct Tx {
    settlement: Settlement,
    transaction: Transaction,
    #[allow(dead_code)]
    receipt: Receipt,
    #[allow(dead_code)]
    auction: super::Auction,
}

impl Tx {
    pub async fn new(
        tx: eth::TxId,
        eth: &infra::Ethereum,
        persistence: &infra::Persistence,
    ) -> Result<Self, Error> {
        let (transaction, receipt) =
            tokio::try_join!(eth.transaction(tx), eth.transaction_receipt(tx),)?;

        let domain_separator = eth.contracts().settlement_domain_separator();
        let settlement = Settlement::new(&transaction.input.0.clone().into(), domain_separator)?;
        let auction = persistence.get_settlement_auction(&settlement).await?;
        Ok(Self {
            settlement,
            transaction,
            receipt,
            auction,
        })
    }

    pub fn auction_id(&self) -> auction::Id {
        self.settlement.auction_id()
    }

    pub fn solver(&self) -> eth::Address {
        self.transaction.solver
    }

    /// Score identical to the one promised during the competition.
    pub fn check_score(&self) -> bool {
        if let Ok(score) = self.score() {
            score == self.auction.score
        } else {
            false
        }
    }

    fn score(&self) -> Result<competition::Score, super::error::Score> {
        self.settlement
            .score(&self.auction.prices, &self.auction.fee_policies)
    }
}

/// An on-chain transaction that settles a settlement.
#[derive(Debug)]
pub struct Transaction {
    /// The hash of the transaction.
    pub hash: eth::TxId,
    /// The address of the solver that submitted the transaction.
    pub solver: eth::Address,
    /// The call data of the transaction.
    pub input: eth::Calldata,
}

/// A receipt of a transaction that settles a settlement.
#[derive(Debug)]
pub struct Receipt {
    /// The hash of the transaction.
    pub hash: eth::TxId,
    /// The block number of the block that contains the transaction.
    pub block: eth::BlockNo,
    /// The gas used by the transaction.
    pub gas: eth::U256,
    /// The effective gas price of the transaction.
    pub effective_gas_price: eth::U256,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Blockchain(#[from] infra::blockchain::Error),
    #[error(transparent)]
    Settlement(#[from] super::Error),
    #[error(transparent)]
    Auction(#[from] infra::persistence::error::Auction),
}
