use {
    crate::{
        boundary::unbuffered_web3_client,
        domain::{eth, mempools},
        infra,
    },
    alloy::{consensus::Transaction, providers::ext::TxPoolApi},
    anyhow::Context,
    ethrpc::{Web3, alloy::conversions::IntoAlloy},
};

#[derive(Debug, Clone)]
pub struct Config {
    pub min_priority_fee: eth::U256,
    pub gas_price_cap: eth::U256,
    pub target_confirm_time: std::time::Duration,
    pub retry_interval: std::time::Duration,
    pub kind: Kind,
    /// Optional block number to use when fetching nonces. If None, uses the
    /// web3 lib's default behavior, which is `latest`.
    pub nonce_block_number: Option<web3::types::BlockNumber>,
}

#[derive(Debug, Clone)]
pub enum Kind {
    /// The public mempool of the [`Ethereum`] node.
    Public {
        max_additional_tip: eth::U256,
        additional_tip_percentage: f64,
        revert_protection: RevertProtection,
    },
    /// The MEVBlocker private mempool.
    MEVBlocker {
        url: reqwest::Url,
        max_additional_tip: eth::U256,
        additional_tip_percentage: f64,
        use_soft_cancellations: bool,
    },
}

impl Kind {
    /// for instrumentization purposes
    pub fn format_variant(&self) -> &'static str {
        match self {
            Kind::Public { .. } => "PublicMempool",
            Kind::MEVBlocker { .. } => "MEVBlocker",
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
}

impl std::fmt::Display for Mempool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Mempool({})", self.config.kind.format_variant())
    }
}

impl Mempool {
    pub fn new(config: Config, transport: Web3) -> Self {
        let transport = match &config.kind {
            Kind::Public { .. } => transport,
            // Flashbots Protect RPC fallback doesn't support buffered transport
            Kind::MEVBlocker { url, .. } => unbuffered_web3_client(url),
        };
        Self { config, transport }
    }

    /// Fetches the transaction count (nonce) for the given address at the
    /// specified block number. If no block number is provided in the config,
    /// uses the web3 lib's default behavior.
    pub async fn get_nonce(&self, address: eth::Address) -> Result<eth::U256, mempools::Error> {
        self.transport
            .eth()
            .transaction_count(address.into(), self.config.nonce_block_number)
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
        gas_price: eth::GasPrice,
        gas_limit: eth::Gas,
        solver: &infra::Solver,
        nonce: eth::U256,
    ) -> Result<eth::TxId, mempools::Error> {
        let submission =
            ethcontract::transaction::TransactionBuilder::new(self.transport.legacy.clone())
                .from(solver.account().clone())
                .to(tx.to.into())
                .nonce(nonce)
                .gas_price(ethcontract::GasPrice::Eip1559 {
                    max_fee_per_gas: gas_price.max().into(),
                    max_priority_fee_per_gas: gas_price.tip().into(),
                })
                .data(tx.input.into())
                .value(tx.value.0)
                .gas(gas_limit.0)
                .access_list(web3::types::AccessList::from(tx.access_list))
                .resolve(ethcontract::transaction::ResolveCondition::Pending)
                .send()
                .await;

        match submission {
            Ok(receipt) => {
                tracing::debug!(
                    ?nonce,
                    ?gas_price,
                    ?gas_limit,
                    solver = ?solver.address(),
                    "successfully submitted tx to mempool"
                );
                Ok(eth::TxId(receipt.hash()))
            }
            Err(err) => {
                // log pending tx in case we failed to replace a pending tx
                let pending_tx = self
                    .find_pending_tx_in_mempool(solver.address(), nonce)
                    .await;

                tracing::debug!(
                    ?err,
                    new_gas_price = ?gas_price,
                    ?nonce,
                    ?pending_tx,
                    ?gas_limit,
                    solver = ?solver.address(),
                    "failed to submit tx to mempool"
                );
                Err(mempools::Error::Other(err.into()))
            }
        }
    }

    /// Queries the mempool for a pending transaction of the given solver and
    /// nonce.
    pub async fn find_pending_tx_in_mempool(
        &self,
        signer: eth::Address,
        nonce: eth::U256,
    ) -> anyhow::Result<Option<alloy::rpc::types::Transaction>> {
        let tx_pool_content = self
            .transport
            .alloy
            .txpool_content_from(signer.0.into_alloy())
            .await
            .context("failed to query pending transactions")?;

        // find the one with the specified nonce
        let pending_tx = tx_pool_content
            .pending
            .into_iter()
            .chain(tx_pool_content.queued)
            .find(|(_signer, tx)| eth::U256::from(tx.nonce()) == nonce)
            .map(|(_, tx)| tx);
        Ok(pending_tx)
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn may_revert(&self) -> bool {
        match &self.config.kind {
            Kind::Public { .. } => true,
            Kind::MEVBlocker { .. } => false,
        }
    }
}
