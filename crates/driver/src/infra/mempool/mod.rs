use {
    super::solver, crate::{
        boundary::{Web3, unbuffered_web3},
        domain::{eth, mempools},
        infra::{self, solver::Account},
    }, alloy::{
        consensus::{Transaction, TxEnvelope},
        eips::{BlockNumberOrTag, Encodable2718, eip1559::Eip1559Estimation},
        network::{Ethereum, NetworkWallet, TxSigner},
        primitives::{Address, Bytes, keccak256},
        providers::{Provider, ext::TxPoolApi},
        rpc::types::TransactionRequest,
        signers::Signer,
    }, anyhow::Context, const_hex::encode_prefixed, dashmap::DashMap, serde_json::json, std::sync::Arc, url::Url
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
        let transport = unbuffered_web3(&config.url);
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
    /// the transaction is pending.
    pub async fn submit(
        &self,
        tx: eth::Tx,
        gas_price: Eip1559Estimation,
        gas_limit: eth::Gas,
        solver_account: solver::Account,
        nonce: u64,
    ) -> Result<eth::TxId, mempools::Error> {
        let max_fee_per_gas = gas_price.max_fee_per_gas;
        let max_priority_fee_per_gas = gas_price.max_priority_fee_per_gas;
        let gas_limit = gas_limit.0.try_into().map_err(anyhow::Error::from)?;

        let tx_request = TransactionRequest::default()
            .from(solver_account.address())
            .to(tx.to)
            .nonce(nonce)
            .max_fee_per_gas(max_fee_per_gas)
            .max_priority_fee_per_gas(max_priority_fee_per_gas)
            .gas_limit(gas_limit)
            .input(tx.input.into())
            .value(tx.value.0)
            .access_list(tx.access_list.into());

        let submission = self
            .transport
            .provider
            .send_transaction(tx_request)
            .await
            .map_err(anyhow::Error::from);

        match submission {
            Ok(tx) => {
                tracing::debug!(
                    ?nonce,
                    ?gas_price,
                    ?gas_limit,
                    solver = ?solver_account.address(),
                    "successfully submitted tx to mempool"
                );
                self.last_submissions
                    .insert(solver_account.address(), Submission { nonce, gas_price });
                Ok(eth::TxId(*tx.tx_hash()))
            }
            Err(err) => {
                // log pending tx in case we failed to replace a pending tx
                let solver_address = solver_account.address();
                let last_submission = self.last_submission(solver_account);

                tracing::debug!(
                    ?err,
                    new_gas_price = ?gas_price,
                    ?nonce,
                    ?last_submission,
                    ?gas_limit,
                    solver = ?solver_address,
                    "failed to submit tx to mempool"
                );
                Err(mempools::Error::Other(err))
            }
        }
    }


    pub async fn submit_till_block(
        &self,
        tx: eth::Tx,
        gas_price: Eip1559Estimation,
        gas_limit: eth::Gas,
        solver_account: solver::Account,
        nonce: u64,
        current_block: u64,
        target_block: u64,
    ) -> Result<eth::TxId, mempools::Error> {
        let max_fee_per_gas = gas_price.max_fee_per_gas;
        let max_priority_fee_per_gas = gas_price.max_priority_fee_per_gas;
        let gas_limit = gas_limit.0.try_into().map_err(anyhow::Error::from)?;

        let tx_request = TransactionRequest::default()
            .from(solver_account.address())
            .to(tx.to)
            .nonce(nonce)
            .max_fee_per_gas(max_fee_per_gas)
            .max_priority_fee_per_gas(max_priority_fee_per_gas)
            .gas_limit(gas_limit)
            .input(tx.input.into())
            .value(tx.value.0)
            .access_list(tx.access_list.into());

        let envelope: TxEnvelope = NetworkWallet::<Ethereum>::sign_request(
            &self.transport.wallet,
            tx_request,
        )
        .await
            .map_err(anyhow::Error::from)
            .context("failed to sign relay transaction")
            .map_err(mempools::Error::Other)?;
        let hash = eth::TxId(*envelope.tx_hash());
        let raw_tx = Bytes::from(envelope.encoded_2718());

        if target_block <= current_block {
            return Err(mempools::Error::Other(anyhow::anyhow!(
                "target block is in the past"
            )));
        }

        let mut submitted = false;
        let mut last_error = None;
        for block in (current_block + 1)..=target_block {
            match self
                .send_relay_bundle(&solver_account, hash.clone(), raw_tx.clone(), block)
                .await
            {
                Ok(()) => submitted = true,
                Err(err) => {
                    tracing::warn!(
                        ?err,
                        ?hash,
                        ?nonce,
                        target_block = block,
                        solver = ?solver_account.address(),
                        "failed to submit tx bundle to relay"
                    );
                    last_error = Some(err);
                }
            }
        }

        if !submitted {
            return Err(mempools::Error::Other(last_error.unwrap_or_else(|| {
                anyhow::anyhow!("relay rejected bundle submission")
            })));
        }

        tracing::debug!(
            ?nonce,
            ?gas_price,
            ?gas_limit,
            target_block,
            solver = ?solver_account.address(),
            "successfully submitted tx bundle to relay"
        );
        self.last_submissions
            .insert(solver_account.address(), Submission { nonce, gas_price });
        Ok(hash)
    }

    async fn send_relay_bundle(
        &self,
        solver_account: &solver::Account,
        tx_hash: eth::TxId,
        raw_tx: Bytes,
        target_block: u64,
    ) -> anyhow::Result<()> {
        let body = json!({
            "id": 1,
            "jsonrpc": "2.0",
            "method": "eth_sendBundle",
            "params": [{
                "txs": [raw_tx],
                "blockNumber": format!("0x{:x}", target_block),
                "droppingTxHashes": [tx_hash.0],
            }]
        });
        let body_str = serde_json::to_string(&body).unwrap();
        let body_hash = encode_prefixed(keccak256(body_str.as_bytes()));
        let signature = match solver_account {
            solver::Account::PrivateKey(signer) => signer.sign_message(body_hash.as_bytes()).await?,
            solver::Account::Kms(signer) => signer.sign_message(body_hash.as_bytes()).await?,
            solver::Account::Address(_) => {
                return Err(anyhow::anyhow!("address-only solver account cannot sign relay bundles"));
            }
        };
        let flashbots_header_value = format!(
            "{}:{}",
            solver_account.address(),
            encode_prefixed(signature.as_bytes())
        );
        let response: serde_json::Value = reqwest::Client::new()
            .post(self.config.url.clone())
            .header("Content-Type", "application/json")
            .header("X-Flashbots-Signature", flashbots_header_value)
            .json(&body)
            .send()
            .await?
            .json()
            .await?;

        if let Some(error) = response.get("error") {
            return Err(anyhow::anyhow!("relay returned error: {error}"));
        }

        tracing::info!(?tx_hash, target_block, relay = %self.config.url, ?response, "relay accepted bundle");
        Ok(())
    }

    /// Queries the mempool for a pending transaction of the given solver and
    /// nonce.
    pub async fn find_pending_tx_in_mempool(
        &self,
        solver_account: solver::Account,
        nonce: u64,
    ) -> anyhow::Result<Option<alloy::rpc::types::Transaction>> {
        let tx_pool_content = self
            .transport
            .provider
            .txpool_content_from(solver_account.address())
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

    /// Looks up the last tx that was submitted for that solver account.
    pub fn last_submission(&self, solver_account: solver::Account) -> Option<Submission> {
        self.last_submissions
            .get(&solver_account.address())
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
