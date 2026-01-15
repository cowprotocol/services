use {
    alloy::transports::{RpcError, TransportErrorKind},
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to submit bid: {0}")]
    SubmitBidFailed(String),
    #[error("failed to fetch bids: {0}")]
    FailedToSubscribe(String),
    #[error("failed to connect to pod: {0}")]
    FailedToConnect(String),
    #[error("pod web3 error: {0}")]
    Web3(#[from] web3::Error),
    #[error("pod contract error: {0}")]
    Method(#[from] ethcontract::errors::MethodError),
    #[error("pod event error: {0}")]
    EventError(String),
    #[error("pod websocket rpc error: {0}")]
    WebsocketError(#[from] RpcError<TransportErrorKind>),
    #[error("pod transaction error: {0}")]
    TransactionError(#[from] ethcontract::errors::ExecutionError),
    #[error("invalid deadline: {0}")]
    InvalidDeadline(u64),
    #[error("invalid auction id: {0}")]
    InvalidAuctionId(i64),
    #[error("pod pending transaction error: {0}")]
    PendingTransactionError(#[from] alloy::providers::PendingTransactionError),
}
