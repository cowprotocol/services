use {
    crate::{
        boundary::unbuffered_web3_client,
        domain::{eth, mempools},
        infra::{self, solver::Account},
    },
    alloy::{
        consensus::Transaction,
        eips::{BlockNumberOrTag, eip1559::Eip1559Estimation},
        primitives::Address,
        providers::{Provider, ext::TxPoolApi},
        rpc::types::TransactionRequest,
    },
    anyhow::Context,
    dashmap::DashMap,
    ethrpc::Web3,
    std::sync::Arc,
    url::Url,
};

#[derive(Debug, Clone)]
pub struct Config {
    pub min_priority_fee: eth::U256,
    pub gas_price_cap: eth::U256,
    pub target_confirm_time: std::time::Duration,
    pub retry_interval: std::time::Duration,
    /// Optional block number to use when fetching nonces. If None, uses the
    /// web3 lib's default behavior, which is `latest`.
    pub nonce_block_number: Option<BlockNumberOrTag>,
    pub url: Url,
    pub name: String,
    pub revert_protection: RevertProtection,
    pub max_additional_tip: eth::U256,
    pub additional_tip_percentage: f64,
    /// Number of blocks to wait before bumping priority fee. 0 = disabled.
    pub gas_bump_interval: u64,
    /// Factor to multiply priority fee by on each bump.
    pub gas_bump_factor: f64,
    /// Maximum number of gas bumps. 0 = unlimited.
    pub max_gas_bumps: u64,
}

#[cfg(test)]
impl Config {
    pub fn test_config(url: Url) -> Self {
        Self {
            min_priority_fee: Default::default(),
            gas_price_cap: eth::U256::from(1000000000000_u128),
            target_confirm_time: Default::default(),
            retry_interval: Default::default(),
            name: "default_rpc".to_string(),
            max_additional_tip: eth::U256::from(3000000000_u128),
            additional_tip_percentage: 0.,
            revert_protection: infra::mempool::RevertProtection::Disabled,
            nonce_block_number: None,
            url,
            gas_bump_interval: 2,
            gas_bump_factor: 1.125,
            max_gas_bumps: 5,
        }
    }
}

/// Don't submit transactions with high revert risk (i.e. transactions
/// that interact with on-chain AMMs) to the public mempool.
/// This can be enabled to avoid MEV when private transaction
/// submission strategies are available. If private submission strategies
/// are not available, revert protection is always disabled.
#[derive(Debug, Clone, Copy)]
pub enum RevertProtection {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone)]
pub struct Mempool {
    transport: Web3,
    config: Config,
    last_submissions: Arc<DashMap<Address, Submission>>,
}

#[derive(Debug, Clone)]
pub struct Submission {
    pub nonce: u64,
    pub gas_price: Eip1559Estimation,
}

impl std::fmt::Display for Mempool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Mempool({})", self.config.name)
    }
}

impl Mempool {
    pub fn new(config: Config, solver_accounts: Vec<Account>) -> Self {
        let transport = unbuffered_web3_client(&config.url);
        // Register the solver accounts into the wallet to submit txs on their behalf
        for account in solver_accounts {
            transport.wallet.register_signer(account);
        }
        Self {
            transport,
            config,
            last_submissions: Default::default(),
        }
    }

    /// Fetches the transaction count (nonce) for the given address at the
    /// specified block number. If no block number is provided in the config,
    /// uses the alloy's default behavior.
    pub async fn get_nonce(&self, address: eth::Address) -> Result<u64, mempools::Error> {
        let call = self.transport.alloy.get_transaction_count(address);
        match self.config.nonce_block_number {
            Some(BlockNumberOrTag::Latest) => call.latest(),
            Some(BlockNumberOrTag::Earliest) => call.earliest(),
            Some(BlockNumberOrTag::Finalized) => call.finalized(),
            Some(BlockNumberOrTag::Number(number)) => call.number(number),
            Some(BlockNumberOrTag::Pending) => call.pending(),
            Some(BlockNumberOrTag::Safe) => call.safe(),
            None => call,
        }
        .await
        .map_err(|err| {
            mempools::Error::Other(anyhow::Error::from(err).context("failed to fetch nonce"))
        })
    }

    /// Submits a transaction to the mempool. Returns optimistically as soon as
    /// the transaction is pending.
    pub async fn submit(
        &self,
        tx: eth::Tx,
        gas_price: Eip1559Estimation,
        gas_limit: eth::Gas,
        solver: &infra::Solver,
        nonce: u64,
    ) -> Result<eth::TxId, mempools::Error> {
        let max_fee_per_gas = gas_price.max_fee_per_gas;
        let max_priority_fee_per_gas = gas_price.max_priority_fee_per_gas;
        let gas_limit = gas_limit.0.try_into().map_err(anyhow::Error::from)?;

        let tx_request = TransactionRequest::default()
            .from(solver.address())
            .to(tx.to)
            .nonce(nonce)
            .max_fee_per_gas(max_fee_per_gas)
            .max_priority_fee_per_gas(max_priority_fee_per_gas)
            .gas_limit(gas_limit)
            .input(tx.input.0.into())
            .value(tx.value.0)
            .access_list(tx.access_list.into());

        let submission = self
            .transport
            .alloy
            .send_transaction(tx_request)
            .await
            .map_err(anyhow::Error::from);

        match submission {
            Ok(tx) => {
                tracing::debug!(
                    ?nonce,
                    ?gas_price,
                    ?gas_limit,
                    solver = ?solver.address(),
                    "successfully submitted tx to mempool"
                );
                self.last_submissions
                    .insert(solver.address(), Submission { nonce, gas_price });
                Ok(eth::TxId(*tx.tx_hash()))
            }
            Err(err) => {
                // log pending tx in case we failed to replace a pending tx
                let last_submission = self.last_submission(solver.address());

                tracing::debug!(
                    ?err,
                    new_gas_price = ?gas_price,
                    ?nonce,
                    ?last_submission,
                    ?gas_limit,
                    solver = ?solver.address(),
                    "failed to submit tx to mempool"
                );
                Err(mempools::Error::Other(err))
            }
        }
    }

    /// Queries the mempool for a pending transaction of the given solver and
    /// nonce.
    pub async fn find_pending_tx_in_mempool(
        &self,
        signer: eth::Address,
        nonce: u64,
    ) -> anyhow::Result<Option<alloy::rpc::types::Transaction>> {
        let tx_pool_content = self
            .transport
            .alloy
            .txpool_content_from(signer)
            .await
            .context("failed to query pending transactions")?;

        // find the one with the specified nonce
        let pending_tx = tx_pool_content
            .pending
            .into_iter()
            .chain(tx_pool_content.queued)
            .find(|(_signer, tx)| tx.nonce() == nonce)
            .map(|(_, tx)| tx);
        Ok(pending_tx)
    }

    /// Looks up the last tx that was submitted for that signer.
    pub fn last_submission(&self, signer: eth::Address) -> Option<Submission> {
        self.last_submissions
            .get(&signer)
            .map(|entry| entry.value().clone())
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn reverts_can_get_mined(&self) -> bool {
        matches!(
            self.config.revert_protection,
            infra::mempool::RevertProtection::Disabled
        )
    }
}
