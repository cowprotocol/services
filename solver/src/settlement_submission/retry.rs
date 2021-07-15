use crate::encoding::EncodedSettlement;
use anyhow::Result;
use contracts::GPv2Settlement;
use ethcontract::{
    dyns::DynMethodBuilder,
    errors::{ExecutionError, MethodError},
    jsonrpc::types::Error as RpcError,
    transaction::{confirm::ConfirmParams, ResolveCondition},
    web3::error::Error as Web3Error,
    GasPrice,
};
use primitive_types::U256;
use transaction_retry::{TransactionResult, TransactionSending};

/// Failure indicating that some aspect about the signed transaction was wrong (e.g. wrong nonce, gas limit to high)
/// and that the transaction could not be mined. This doesn't mean the transaction reverted.
fn is_transaction_error(error: &ExecutionError) -> bool {
    // TODO: check how this looks on turbogeth and other clients. Not recognizing the error is not a serious
    // problem but it will make us sometimes log an error when there actually was no problem.
    match error {
        // Cf. openethereum's source code in `rpc/src/v1/helpers/errors.rs`. This code is not used by geth
        ExecutionError::Web3(Web3Error::Rpc(RpcError { code, .. })) if code.code() == -32010 => {
            true
        }
        // Geth uses error code -32000 for all kinds of RPC errors (maps to UNSUPPORTED_REQUEST in open ethereum)
        // cf. https://github.com/ethereum/go-ethereum/blob/9357280fce5c5d57111d690a336cca5f89e34da6/rpc/errors.go#L59
        // Therefore we also require to match on known error message
        ExecutionError::Web3(Web3Error::Rpc(RpcError { code, message, .. }))
            if code.code() == -32000 && message == "nonce too low" =>
        {
            true
        }
        _ => false,
    }
}

/// Failure indicating the transaction reverted for some reason
pub fn is_transaction_failure(error: &ExecutionError) -> bool {
    matches!(error, ExecutionError::Failure(_))
        || matches!(error, ExecutionError::Revert(_))
        || matches!(error, ExecutionError::InvalidOpcode)
}

pub struct SettleResult(pub Result<(), MethodError>);
impl TransactionResult for SettleResult {
    fn was_mined(&self) -> bool {
        if let Err(err) = &self.0 {
            !is_transaction_error(&err.inner)
        } else {
            true
        }
    }
}

pub struct SettlementSender<'a> {
    pub contract: &'a GPv2Settlement,
    pub nonce: U256,
    pub gas_limit: f64,
    pub settlement: EncodedSettlement,
}
#[async_trait::async_trait]
impl<'a> TransactionSending for SettlementSender<'a> {
    type Output = SettleResult;
    async fn send(&self, gas_price: f64) -> Self::Output {
        tracing::info!("submitting solution transaction at gas price {}", gas_price);
        let mut method = settle_method_builder(self.contract, self.settlement.clone())
            .nonce(self.nonce)
            .gas_price(GasPrice::Value(U256::from_f64_lossy(gas_price)))
            .gas(U256::from_f64_lossy(self.gas_limit));
        method.tx.resolve = Some(ResolveCondition::Confirmed(ConfirmParams::mined()));
        let result = method.send().await.map(|_| ());
        SettleResult(result)
    }
}

pub fn settle_method_builder(
    contract: &GPv2Settlement,
    settlement: EncodedSettlement,
) -> DynMethodBuilder<()> {
    contract.settle(
        settlement.tokens,
        settlement.clearing_prices,
        settlement.trades,
        settlement.interactions,
    )
}

// We never send cancellations but we still need to have types that implement the traits.
pub struct CancelResult;
impl TransactionResult for CancelResult {
    fn was_mined(&self) -> bool {
        unreachable!()
    }
}

pub struct CancelSender;
#[async_trait::async_trait]
impl TransactionSending for CancelSender {
    type Output = CancelResult;
    async fn send(&self, _gas_price: f64) -> Self::Output {
        unreachable!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context;
    use jsonrpc_core::ErrorCode;
    use primitive_types::H256;

    #[test]
    fn test_submission_result_was_mined() {
        let transaction_error_open_ethereum = ExecutionError::Web3(Web3Error::Rpc(RpcError {
            code: ErrorCode::from(-32010),
            message: "".into(),
            data: None,
        }));
        let transaction_error_geth = ExecutionError::Web3(Web3Error::Rpc(RpcError {
            code: ErrorCode::from(-32000),
            message: "nonce too low".into(),
            data: None,
        }));

        let result = SettleResult(Ok(()));
        assert!(result.was_mined());

        let result = SettleResult(Err(MethodError::from_parts(
            "".into(),
            ExecutionError::StreamEndedUnexpectedly,
        )));
        assert!(result.was_mined());

        let result = SettleResult(Err(MethodError::from_parts(
            "".into(),
            transaction_error_open_ethereum,
        )));
        assert!(!result.was_mined());

        let result = SettleResult(Err(MethodError::from_parts(
            "".into(),
            transaction_error_geth,
        )));
        assert!(!result.was_mined());
    }

    #[test]
    fn test_is_transaction_failure() {
        // Positives
        assert!(is_transaction_failure(&ExecutionError::Failure(
            Default::default()
        )),);
        assert!(is_transaction_failure(&ExecutionError::Revert(None)));
        assert!(is_transaction_failure(&ExecutionError::InvalidOpcode));

        // Sample negative
        assert!(!is_transaction_failure(&ExecutionError::ConfirmTimeout(
            Box::new(ethcontract::transaction::TransactionResult::Hash(
                H256::default()
            ))
        )));

        let method_error =
            MethodError::from_parts("foo".into(), ExecutionError::Failure(Default::default()));
        let settle_result = SettleResult(Err(method_error));
        let returned = settle_result.0.context("foo");

        assert!(returned
            .expect_err("expected error")
            .downcast_ref::<MethodError>()
            .map(|e| is_transaction_failure(&e.inner))
            .unwrap_or(false));
    }
}
