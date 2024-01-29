//! Module containing domain specific event structures per contract

/// GPv2 Settlement contract events.
pub mod settlement {
    use crate::domain::{self, eth};

    /// An order was fully or partially traded.
    pub struct Trade {
        pub block_number: u64,
        /// The index of the event in the block.
        pub log_index: usize,
        pub uid: domain::OrderUid,
        pub sell_amount_including_fee: eth::U256,
        pub buy_amount: eth::U256,
        pub fee_amount: eth::U256,
    }

    /// An order was cancelled on-chain.
    pub struct Cancellation {
        pub block_number: u64,
        /// The index of the event in the block.
        pub log_index: usize,
        pub uid: domain::OrderUid,
    }

    /// A settlement was executed on-chain.
    pub struct Settlement {
        pub block_number: u64,
        /// The index of the event in the block.
        pub log_index: usize,
        pub tx_hash: eth::H256,
        pub solver: eth::Address,
    }

    /// An order was signed on-chain or signature for an on-chain signed order
    /// has been revoked.
    pub struct PreSignature {
        pub block_number: u64,
        /// The index of the event in the block.
        pub log_index: usize,
        pub uid: domain::OrderUid,
        pub owner: eth::Address,
        pub signed: bool,
    }
}
