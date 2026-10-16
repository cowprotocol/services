mod builders;

pub use builders::Builder;
use {
    crate::{
        boundary::{Web3, unbuffered_web3},
        domain::mempools,
        infra::{self, solver::Account},
    },
    alloy::{
        consensus::Transaction,
        eips::{BlockNumberOrTag, eip1559::Eip1559Estimation},
        network::{Ethereum, NetworkWallet, TxSigner},
        primitives::Address,
        providers::{Provider, ext::TxPoolApi},
        rpc::types::TransactionRequest,
        transports::TransportError,
    },
    anyhow::Context,
    builders::Builders,
    dashmap::DashMap,
    eth_domain_types as eth,
    std::{collections::HashMap, ops::RangeInclusive, sync::Arc},
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
    /// Block builders to send the settlement to as bundles. When empty the
    /// settlement goes to `url` instead.
    pub builders: Vec<Builder>,
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
            builders: Default::default(),
            url,
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
    /// Empty when the settlement goes to `config.url` instead.
    builders: Builders,
    /// Chain id of the transactions signed here for the builders.
    chain_id: u64,
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
    pub fn new(config: Config, solver_accounts: Vec<Account>, chain_id: u64) -> Self {
        let transport = unbuffered_web3(&config.url);
        // Register the solver accounts into the wallet to submit txs on their
        // behalf
        let mut accounts = HashMap::new();
        for account in solver_accounts {
            accounts.insert(TxSigner::address(&account), account.clone());
            transport.wallet.register_signer(account);
        }
        if !config.builders.is_empty() {
            // Builders get txs signed here, which an address-only account
            // leaves to the node.
            let address_only: Vec<_> = accounts
                .iter()
                .filter(|(_, account)| matches!(account, Account::Address(_)))
                .map(|(address, _)| address)
                .collect();
            assert!(
                address_only.is_empty(),
                "mempool {} submits to builders but accounts {address_only:?} cannot sign",
                config.name
            );
        }
        let builders = Builders::new(config.name.clone(), &config.builders, accounts);
        Self {
            transport,
            builders,
            chain_id,
            config,
            last_submissions: Default::default(),
        }
    }

    /// Fetches the transaction count (nonce) for the given address at the
    /// specified block number. If no block number is provided in the config,
    /// uses the alloy's default behavior.
    pub async fn get_nonce(&self, address: eth::Address) -> Result<u64, mempools::Error> {
        let call = self.transport.provider.get_transaction_count(address);
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
    /// the transaction is pending. `signer` is the address that signs and pays
    /// for gas (may differ from the solver address in EIP-7702 mode). `blocks`
    /// are the blocks the transaction may land in, which only builders use.
    pub async fn submit(
        &self,
        tx: eth::Tx,
        gas_price: Eip1559Estimation,
        gas_limit: eth::Gas,
        signer: eth::Address,
        nonce: u64,
        blocks: RangeInclusive<u64>,
    ) -> Result<eth::TxId, mempools::Error> {
        let max_fee_per_gas = gas_price.max_fee_per_gas;
        let max_priority_fee_per_gas = gas_price.max_priority_fee_per_gas;
        let gas_limit = gas_limit.0.try_into().map_err(anyhow::Error::from)?;

        let tx_request = TransactionRequest::default()
            .from(signer)
            .to(tx.to)
            .nonce(nonce)
            .max_fee_per_gas(max_fee_per_gas)
            .max_priority_fee_per_gas(max_priority_fee_per_gas)
            .gas_limit(gas_limit)
            .input(tx.input.into())
            .value(tx.value.0)
            .access_list(tx.access_list.into());

        let submission = match self.submits_to_builders() {
            true => self.send_to_builders(tx_request, signer, blocks).await,
            false => self.send_to_node(tx_request).await,
        };

        match submission {
            Ok(hash) => {
                tracing::debug!(
                    ?nonce,
                    ?gas_price,
                    ?gas_limit,
                    ?signer,
                    "successfully submitted tx to mempool"
                );
                self.last_submissions
                    .insert(signer, Submission { nonce, gas_price });
                Ok(hash)
            }
            Err(err) => {
                // log pending tx in case we failed to replace a pending tx
                let last_submission = self.last_submission(signer);

                tracing::debug!(
                    ?err,
                    new_gas_price = ?gas_price,
                    ?nonce,
                    ?last_submission,
                    ?gas_limit,
                    ?signer,
                    "failed to submit tx to mempool"
                );
                Err(mempools::Error::Other(err))
            }
        }
    }

    /// Sends the transaction to the node, which signs it with the registered
    /// wallet and forwards it to its own mempool.
    async fn send_to_node(&self, tx: TransactionRequest) -> anyhow::Result<eth::TxId> {
        let pending = self
            .transport
            .provider
            .send_transaction(tx)
            .await
            .inspect_err(log_rpc_error_code)?;
        Ok(eth::TxId(*pending.tx_hash()))
    }

    /// Signs the transaction here and sends it to the builders as one bundle
    /// for each of `blocks`, the window a mempool tx would stay valid for.
    async fn send_to_builders(
        &self,
        mut tx: TransactionRequest,
        signer: eth::Address,
        blocks: RangeInclusive<u64>,
    ) -> anyhow::Result<eth::TxId> {
        // Without a chain id the signed tx silently falls back to mainnet.
        tx.chain_id = Some(self.chain_id);
        let envelope = NetworkWallet::<Ethereum>::sign_request(&self.transport.wallet, tx)
            .await
            .context("failed to sign the tx for the builders")?;
        self.builders.send(&envelope, signer, blocks).await?;
        Ok(eth::TxId(*envelope.tx_hash()))
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
            .provider
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

    /// Whether the settlement goes to block builders as bundles instead of
    /// to the mempool of `config.url`.
    pub fn submits_to_builders(&self) -> bool {
        !self.builders.is_empty()
    }

    pub fn reverts_can_get_mined(&self) -> bool {
        matches!(
            self.config.revert_protection,
            infra::mempool::RevertProtection::Disabled
        )
    }
}

/// Logs a hint for known JSON-RPC error codes. Codes in -32768..=-32000 mean
/// different things per chain, so only map new codes once they're understood.
fn log_rpc_error_code(err: &TransportError) {
    match err.as_error_resp().map(|resp| resp.code) {
        // Standard error, e.g. "replacement transaction underpriced".
        None | Some(-32000) => {}
        // A proxy in front of the sequencer rejected the tx (seen on Ink).
        // https://github.com/ethereum-optimism/infra/blob/a4d73e6c8afed6b2dcb007d1c6f26455df45599f/proxyd/backend.go#L140-L144
        Some(-32100) => tracing::warn!(
            "transaction rejected upstream (JSON-RPC -32100): the RPC's proxy refused it before \
             the sequencer"
        ),
        Some(code) => tracing::warn!("unexpected error code {code}"),
    }
}
