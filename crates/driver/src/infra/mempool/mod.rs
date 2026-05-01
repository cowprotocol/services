use {
    crate::{
        boundary::{Web3, unbuffered_web3},
        domain::mempools,
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
    eth_domain_types as eth,
    std::{
        sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        },
        time::{Duration, SystemTime, UNIX_EPOCH},
    },
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
    /// Best-known gas floor per signer at their currently-pending nonce.
    /// Source may be a prior `submit()` or a `txpool_content_from` probe
    /// ([`Mempool::record_observed`]). Updates are gated by
    /// [`Submission::supersedes`] so an out-of-order or stale write cannot
    /// regress a fresher entry under intra-process races.
    last_submissions: Arc<DashMap<Address, Submission>>,
}

#[derive(Debug, Clone)]
pub struct Submission {
    pub nonce: u64,
    pub gas_price: Eip1559Estimation,
}

impl Submission {
    /// Returns `true` if `self` should replace `other` in `last_submissions`.
    ///
    /// Ordering is tuple-monotonic on `(nonce, max_priority_fee_per_gas)`:
    /// - a higher nonce always wins;
    /// - same nonce with a higher priority fee wins (meaning a speed-up);
    /// - otherwise no-op.
    ///
    /// The ordering is safe against intra-process races where two `submit()`
    /// or `record_observed()` calls arrive out of order: only a higher nonce
    /// or a higher priority fee can overwrite an existing entry.
    fn supersedes(&self, other: &Submission) -> bool {
        (self.nonce, self.gas_price.max_priority_fee_per_gas)
            > (other.nonce, other.gas_price.max_priority_fee_per_gas)
    }
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
    /// the transaction is pending. `signer` is the address that signs and pays
    /// for gas (may differ from the solver address in EIP-7702 mode).
    pub async fn submit(
        &self,
        tx: eth::Tx,
        gas_price: Eip1559Estimation,
        gas_limit: eth::Gas,
        signer: eth::Address,
        nonce: u64,
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
                    ?signer,
                    "successfully submitted tx to mempool"
                );
                self.update_submission(signer, Submission { nonce, gas_price });
                Ok(eth::TxId(*tx.tx_hash()))
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

    /// Probes the node for a pending tx at `(signer, nonce)` and returns its
    /// gas price. Returns `None` if no pending tx exists at that nonce or if
    /// the probe times out, errors, or encounters a non-EIP-1559 tx.
    ///
    /// A slow node cannot block `/settle` — the probe has a 500ms hard
    /// deadline. If the probe fails (timeout or RPC error), a process-wide
    /// cooldown skips further probes for 30 seconds; `Ok(None)` (no pending
    /// tx at that nonce) is not a failure and does not trigger the cooldown.
    pub async fn probe_pending_gas(
        &self,
        signer: eth::Address,
        nonce: u64,
    ) -> Option<Eip1559Estimation> {
        /// Hard latency cap on the probe so a slow node cannot block `/settle`.
        const TIMEOUT: Duration = Duration::from_millis(500);
        /// How long to suppress probes after a failure (timeout / RPC error).
        /// `txpool_*` support is an RPC-level property, so a single
        /// process-wide cooldown stops us paying the timeout cost on every
        /// `/settle` against an endpoint that doesn't expose the namespace.
        const COOLDOWN: Duration = Duration::from_secs(30);
        /// Unix-ms of the last probe failure; `0` means none recorded yet.
        static LAST_FAILURE_MS: AtomicU64 = AtomicU64::new(0);

        let now_ms = now_unix_ms();
        let last_failure_ms = LAST_FAILURE_MS.load(Ordering::Relaxed);
        if last_failure_ms != 0
            && now_ms.saturating_sub(last_failure_ms) < COOLDOWN.as_millis() as u64
        {
            tracing::debug!("skipping mempool probe due to recent failure");
            return None;
        }

        let pending_tx =
            match tokio::time::timeout(TIMEOUT, self.find_pending_tx_in_mempool(signer, nonce))
                .await
            {
                Err(_) => {
                    LAST_FAILURE_MS.store(now_unix_ms(), Ordering::Relaxed);
                    tracing::debug!("mempool probe timed out");
                    return None;
                }
                Ok(Err(err)) => {
                    LAST_FAILURE_MS.store(now_unix_ms(), Ordering::Relaxed);
                    tracing::debug!(?err, "could not inspect tx mempool");
                    return None;
                }
                Ok(Ok(None)) => return None,
                Ok(Ok(Some(tx))) => tx,
            };

        Some(Eip1559Estimation {
            max_fee_per_gas: pending_tx.max_fee_per_gas(),
            max_priority_fee_per_gas: pending_tx.max_priority_fee_per_gas().or_else(|| {
                tracing::error!(tx = ?pending_tx.inner.tx_hash(), "pending tx is not EIP 1559");
                None
            })?,
        })
    }

    /// Looks up the last tx that was submitted for that signer.
    pub fn last_submission(&self, signer: eth::Address) -> Option<Submission> {
        self.last_submissions
            .get(&signer)
            .map(|entry| entry.value().clone())
    }

    /// Records a pending tx observed externally (from a `txpool_content_from`
    /// probe) so subsequent lookups for `(signer, nonce)` skip the RPC probe.
    /// The map tracks the best-known gas floor for each signer at their
    /// current pending nonce, regardless of source.
    pub fn record_observed(&self, signer: eth::Address, nonce: u64, gas_price: Eip1559Estimation) {
        self.update_submission(signer, Submission { nonce, gas_price });
    }

    /// Updates `last_submissions[signer]` to `new` only if `new` is fresher
    /// than the existing entry, per [`Submission::supersedes`].
    fn update_submission(&self, signer: eth::Address, new: Submission) {
        self.last_submissions
            .entry(signer)
            .and_modify(|cur| {
                if new.supersedes(cur) {
                    *cur = new.clone();
                }
            })
            .or_insert(new);
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

fn now_unix_ms() -> u64 {
    // `Err` only when the system clock is before 1970 — unreachable in
    // practice. `0` is the sentinel for \"no failure recorded\", so a broken
    // clock degrades to \"throttle disabled\" rather than panic.
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gas(max_priority_fee_per_gas: u128, max_fee_per_gas: u128) -> Eip1559Estimation {
        Eip1559Estimation {
            max_fee_per_gas,
            max_priority_fee_per_gas,
        }
    }

    fn sub(nonce: u64, prio: u128) -> Submission {
        Submission {
            nonce,
            gas_price: gas(prio, prio * 10),
        }
    }

    #[test]
    fn supersedes() {
        assert!(sub(11, 100).supersedes(&sub(10, 200)), "higher nonce wins");
        assert!(
            sub(10, 200).supersedes(&sub(10, 100)),
            "same nonce, higher priority fee wins"
        );
        assert!(
            !sub(9, 1_000).supersedes(&sub(10, 100)),
            "lower nonce never wins"
        );
        assert!(
            !sub(10, 50).supersedes(&sub(10, 100)),
            "same nonce, lower priority fee never wins"
        );
        assert!(
            !sub(10, 100).supersedes(&sub(10, 100)),
            "identical is a no-op"
        );
    }

    fn empty_mempool() -> Mempool {
        let url = Url::parse("http://localhost:0").unwrap();
        Mempool::new(Config::test_config(url), Vec::new())
    }

    #[test]
    fn update_submission_speed_up() {
        let mempool = empty_mempool();
        let signer = Address::repeat_byte(0xab);

        mempool.update_submission(signer, sub(10, 100));
        mempool.update_submission(signer, sub(10, 200));

        let stored = mempool.last_submission(signer).unwrap();
        assert_eq!(stored.nonce, 10);
        assert_eq!(stored.gas_price.max_priority_fee_per_gas, 200);
    }

    #[test]
    fn update_submission_advances_to_higher_nonce() {
        let mempool = empty_mempool();
        let signer = Address::repeat_byte(0xab);

        mempool.update_submission(signer, sub(10, 500));
        mempool.update_submission(signer, sub(11, 100));

        let stored = mempool.last_submission(signer).unwrap();
        assert_eq!(stored.nonce, 11);
        assert_eq!(stored.gas_price.max_priority_fee_per_gas, 100);
    }

    #[test]
    fn update_submission_rejects_stale_arrivals() {
        let mempool = empty_mempool();
        let signer = Address::repeat_byte(0xab);

        mempool.update_submission(signer, sub(11, 100));
        // Stale insert from an out-of-order submit lands later.
        mempool.update_submission(signer, sub(10, 9_999));

        let stored = mempool.last_submission(signer).unwrap();
        assert_eq!(stored.nonce, 11);
        assert_eq!(stored.gas_price.max_priority_fee_per_gas, 100);
    }

    #[test]
    fn update_submission_rejects_same_nonce_lower_fee() {
        let mempool = empty_mempool();
        let signer = Address::repeat_byte(0xab);

        mempool.update_submission(signer, sub(10, 200));
        mempool.update_submission(signer, sub(10, 100));

        let stored = mempool.last_submission(signer).unwrap();
        assert_eq!(stored.gas_price.max_priority_fee_per_gas, 200);
    }
}
