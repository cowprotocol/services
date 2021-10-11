use crate::encoding::EncodedSettlement;
use anyhow::Result;
use contracts::GPv2Settlement;
use ethcontract::{
    dyns::DynMethodBuilder,
    errors::{ExecutionError, MethodError},
    jsonrpc::types::Error as RpcError,
    transaction::{confirm::ConfirmParams, ResolveCondition},
    web3::error::Error as Web3Error,
    Account, GasPrice, TransactionHash,
};
use futures::FutureExt;
use primitive_types::U256;
use shared::Web3;
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

pub struct SettleResult(pub Result<TransactionHash, MethodError>);

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
    pub nodes: &'a [Web3],
    pub nonce: U256,
    pub gas_limit: f64,
    pub settlement: EncodedSettlement,
    pub account: Account,
}

#[async_trait::async_trait]
impl<'a> TransactionSending for SettlementSender<'a> {
    type Output = SettleResult;

    async fn send(&self, gas_price: f64) -> Self::Output {
        debug_assert!(!self.nodes.is_empty());
        tracing::info!(
            "submitting solution transaction at gas price {:.2} GWei",
            gas_price / 1e9
        );
        let send_with_node = |node| {
            let contract = GPv2Settlement::at(node, self.contract.address());
            let mut method =
                settle_method_builder(&contract, self.settlement.clone(), self.account.clone())
                    .nonce(self.nonce)
                    .gas_price(GasPrice::Value(U256::from_f64_lossy(gas_price)))
                    .gas(U256::from_f64_lossy(self.gas_limit));
            method.tx.resolve = Some(ResolveCondition::Confirmed(ConfirmParams::mined()));
            async { SettleResult(method.send().await.map(|tx| tx.hash())) }.boxed()
        };
        let mut futures = self.nodes.iter().map(send_with_node).collect::<Vec<_>>();
        loop {
            let (result, _index, rest) = futures::future::select_all(futures).await;
            match &result.0 {
                Ok(_) => return result,
                Err(_) if rest.is_empty() => return result,
                Err(err) => {
                    tracing::warn!(?err, "single node tx failed");
                    futures = rest;
                }
            }
        }
    }
}

pub fn settle_method_builder(
    contract: &GPv2Settlement,
    settlement: EncodedSettlement,
    from: Account,
) -> DynMethodBuilder<()> {
    contract
        .settle(
            settlement.tokens,
            settlement.clearing_prices,
            settlement.trades,
            settlement.interactions,
        )
        .from(from)
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
    use ethcontract::PrivateKey;
    use jsonrpc_core::ErrorCode;
    use primitive_types::H256;
    use shared::transport::create_test_transport;
    use tracing::level_filters::LevelFilter;

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

        let result = SettleResult(Ok(H256([0xcd; 32])));
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

    // env NODE0=... NODE1=... PRIVATE_KEY=... cargo test -p solver multi_node_rinkeby_test -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn multi_node_rinkeby_test() {
        shared::tracing::initialize("solver=debug,shared=debug", LevelFilter::OFF);
        let envs = ["NODE0", "NODE1"];
        let web3s: Vec<Web3> = envs
            .iter()
            .map(|key| {
                let value = std::env::var(key).unwrap();
                let transport = create_test_transport(&value);
                Web3::new(transport)
            })
            .collect();
        for web3 in &web3s {
            let network_id = web3.net().version().await.unwrap();
            assert_eq!(network_id, "4");
        }
        let contract = crate::get_settlement_contract(&web3s[0]).await.unwrap();
        let private_key: PrivateKey = std::env::var("PRIVATE_KEY").unwrap().parse().unwrap();
        let account = Account::Offline(private_key, Some(4));
        let nonce = web3s[0]
            .eth()
            .transaction_count(account.address(), None)
            .await
            .unwrap();
        let settlement = EncodedSettlement::default();
        let sender = SettlementSender {
            contract: &contract,
            nodes: &web3s,
            gas_limit: 1e5,
            settlement,
            account,
            nonce,
        };
        let result = sender.send(1e9).await;
        tracing::info!("finished with result {:?}", result.0);
    }
}
