//! Wire types
//!
//! Re-exports of the `yellowstone-grpc-proto` message types the indexer
//! consumes as its wire-format surface.
pub use yellowstone_grpc_proto::{
    geyser::{SubscribeUpdateAccountInfo, SubscribeUpdateTransactionInfo},
    solana::storage::confirmed_block::{
        CompiledInstruction,
        InnerInstructions,
        Message,
        TokenBalance,
        Transaction,
        TransactionStatusMeta,
    },
};
