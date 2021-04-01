use super::EncodedSettlement;
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

fn is_transaction_error(error: &ExecutionError) -> bool {
    // This is the error as we've seen it on openethereum nodes. The code and error messages can
    // be found in openethereum's source code in `rpc/src/v1/helpers/errors.rs`.
    // TODO: check how this looks on geth and infura. Not recognizing the error is not a serious
    // problem but it will make us sometimes log an error when there actually was no problem.
    matches!(error, ExecutionError::Web3(Web3Error::Rpc(RpcError { code, .. })) if code.code() == -32010)
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
        settlement.encoded_trades,
        settlement.encoded_interactions,
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
    use jsonrpc_core::ErrorCode;

    #[test]
    fn test_submission_result_was_mined() {
        let transaction_error = ExecutionError::Web3(Web3Error::Rpc(RpcError {
            code: ErrorCode::from(-32010),
            message: "".into(),
            data: None,
        }));
        let result = SettleResult(Ok(()));
        assert!(result.was_mined());

        let result = SettleResult(Err(MethodError::from_parts(
            "".into(),
            ExecutionError::StreamEndedUnexpectedly,
        )));
        assert!(result.was_mined());

        let result = SettleResult(Err(MethodError::from_parts("".into(), transaction_error)));
        assert!(!result.was_mined());
    }
}
