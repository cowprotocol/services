//! Gelato settlement submission strategy.

mod trampoline;

use {
    self::trampoline::Trampoline,
    super::SubmissionError,
    crate::settlement::Settlement,
    anyhow::{anyhow, Context as _, Result},
    contracts::GPv2Settlement,
    ethcontract::{Account, H256},
    shared::{
        ethrpc::Web3,
        gelato_api::{GelatoClient, TaskId, TaskState},
    },
    std::time::Duration,
    web3::types::TransactionReceipt,
};

/// A Gelato submitter.
pub struct GelatoSubmitter {
    web3: Web3,
    client: GelatoClient,
    trampoline: Trampoline,
    poll_interval: Duration,
}

impl GelatoSubmitter {
    pub async fn new(
        web3: Web3,
        settlement: GPv2Settlement,
        client: GelatoClient,
        poll_interval: Duration,
    ) -> Result<Self> {
        let trampoline = Trampoline::initialize(settlement).await?;
        Ok(Self {
            web3,
            client,
            trampoline,
            poll_interval,
        })
    }

    pub async fn relay_settlement(
        &self,
        account: Account,
        settlement: Settlement,
    ) -> Result<TransactionReceipt, SubmissionError> {
        let call = self.trampoline.prepare_call(&account, &settlement).await?;
        let task_id = self.client.sponsored_call(&call).await?;
        let transaction_hash = self.wait_for_task(task_id).await?;
        self.wait_for_transaction(transaction_hash).await
    }

    async fn wait_for_task(&self, task_id: TaskId) -> Result<H256, SubmissionError> {
        loop {
            let task = self.client.task_status(&task_id).await?;
            match task.task_state {
                TaskState::CheckPending
                | TaskState::ExecPending
                | TaskState::WaitingForConfirmation => {
                    tracing::trace!(?task, "task pending...");
                    tokio::time::sleep(self.poll_interval).await;
                }
                TaskState::ExecSuccess | TaskState::ExecReverted => {
                    let transaction_hash = task
                        .transaction_hash
                        .context("missing transaction hash for confirmed Gelato task")?;
                    return Ok(transaction_hash);
                }
                TaskState::Blacklisted | TaskState::Cancelled | TaskState::NotFound => {
                    tracing::error!(?task, "unexpected Gelato task state");
                    return Err(anyhow!("error executing Gelato task {task_id}").into());
                }
            }
        }
    }

    async fn wait_for_transaction(
        &self,
        hash: H256,
    ) -> Result<TransactionReceipt, SubmissionError> {
        loop {
            let receipt = self.web3.eth().transaction_receipt(hash).await?;
            match receipt {
                Some(receipt) => return Ok(receipt),
                None => {
                    tracing::trace!(?hash, "waiting for transaction...");
                    tokio::time::sleep(self.poll_interval).await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        shared::ethrpc::{create_env_test_transport, Web3},
        std::env,
    };

    #[ignore]
    #[tokio::test]
    async fn execute_relayed_settlement() {
        let web3 = Web3::new(create_env_test_transport());
        let settlement = GPv2Settlement::deployed(&web3).await.unwrap();
        let client = GelatoClient::test_from_env().unwrap();

        let gelato = GelatoSubmitter::new(web3, settlement, client, Duration::from_secs(5))
            .await
            .unwrap();

        let solver = Account::Offline(env::var("SOLVER_ACCOUNT").unwrap().parse().unwrap(), None);
        let settlement = Settlement::default();

        let transaction = gelato.relay_settlement(solver, settlement).await.unwrap();
        println!("executed transaction {:?}", transaction.transaction_hash);

        assert_eq!(transaction.status, Some(1.into()));
    }
}
