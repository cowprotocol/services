use {
    crate::logic::eth,
    ethcontract::{transport::DynTransport, Web3},
    thiserror::Error,
};

pub mod contracts;

const MAX_BATCH_SIZE: usize = 100;

#[derive(Debug, Error)]
pub enum Error {
    #[error("method error: {0:?}")]
    Method(#[from] ethcontract::errors::MethodError),
    #[error("deploy error: {0:?}")]
    Deploy(#[from] ethcontract::errors::DeployError),
}

/// The Ethereum node.
#[derive(Debug)]
pub struct EthNode(Web3<DynTransport>);

impl EthNode {
    pub async fn settlement_contract(&self) -> Result<eth::Address, Error> {
        Ok(contracts::GPv2Settlement::deployed(&self.0)
            .await?
            .address()
            .into())
    }

    /// Fetch the ERC20 allowances for each spender. The allowances are returned
    /// in the same order as the input spenders.
    pub async fn allowances(
        &self,
        // TODO For my use case, this should be the settlement contract address. The fact that the
        // solution module needs to know this is a clear indication that this filtering should
        // happen in the settlement module, not the solution module. I think that Approvals should
        // ensure that the approvals are normalized and sorted, while the filtering should happen
        // in the settlement module, probably in settlement::encode since it is a detail of the
        // encoding process
        owner: eth::Address,
        spenders: impl Iterator<Item = eth::Spender>,
    ) -> Result<Vec<eth::Allowance>, Error> {
        let mut batch = ethcontract::batch::CallBatch::new(self.0.transport());
        let calls: Vec<_> = spenders
            .map(|spender| {
                (
                    spender,
                    contracts::ERC20::at(&self.0, spender.token.0)
                        .allowance(owner.0, spender.address.0)
                        .batch_call(&mut batch),
                )
            })
            .collect();
        batch.execute_all(MAX_BATCH_SIZE).await;
        let mut allowances = Vec::new();
        for (spender, call) in calls {
            match call.await {
                Ok(amount) => allowances.push(eth::Allowance { spender, amount }),
                Err(err) if Self::is_batch_error(&err.inner) => return Err(err.into()),
                Err(err) => {
                    tracing::warn!(
                        "error retrieving allowance for {spender:?} and owner {owner:?}: {err:?}"
                    );
                    continue;
                }
            };
        }
        Ok(allowances)
    }

    fn is_batch_error(err: &ethcontract::errors::ExecutionError) -> bool {
        match &err {
            ethcontract::errors::ExecutionError::Web3(web3::Error::Transport(
                web3::error::TransportError::Message(message),
            )) => {
                // Currently, there is no reliable way to determine if a Web3 error
                // is caused because of a failing batch request, or some call
                // specific error, so test that the message starts with "Batch"
                // as a best guess.
                //
                // https://github.com/gnosis/ethcontract-rs/issues/550
                message.starts_with("Batch")
            }
            _ => false,
        }
    }
}

pub struct Contracts<'a>(&'a Web3<DynTransport>);

impl Contracts<'_> {
    pub fn erc20(self, token: eth::Token) -> contracts::ERC20 {
        contracts::ERC20::at(self.0, token.0)
    }
}
