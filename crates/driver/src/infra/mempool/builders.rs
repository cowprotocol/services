//! Submission of settlements to block builders over `eth_sendBundle`.
//!
//! A settlement goes out as a bundle of one transaction without
//! `revertingTxHashes`, so a builder drops it instead of mining it when it
//! reverts.

use {
    crate::{
        boundary::unbuffered_web3,
        infra::{observe, solver::Account},
    },
    alloy::{
        consensus::TxEnvelope,
        eips::eip2718::Encodable2718,
        primitives::Bytes,
        providers::ext::MevApi,
        rpc::types::mev::EthSendBundle,
    },
    anyhow::{Context, anyhow},
    eth_domain_types as eth,
    ethrpc::AlloyProvider,
    futures::future::join_all,
    std::{
        collections::HashMap,
        ops::RangeInclusive,
        time::{Duration, Instant},
    },
    url::Url,
};

/// A builder that stops answering must not hold up the others: submission is
/// on the critical path of the block.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);

/// A block builder that accepts bundles over `eth_sendBundle`.
#[derive(Debug, Clone)]
pub struct Builder {
    pub name: String,
    pub url: Url,
}

/// Sends settlements to all the block builders of one mempool.
#[derive(Debug, Clone)]
pub struct Builders {
    /// Name of the mempool these builders belong to, for logs and metrics.
    mempool: String,
    endpoints: Vec<Endpoint>,
    /// The accounts that sign the settlements, by address. Each one also signs
    /// the `X-Flashbots-Signature` header of its bundles, the identity that
    /// builders like BuilderNet pay refunds to.
    accounts: HashMap<eth::Address, Account>,
}

#[derive(Debug, Clone)]
struct Endpoint {
    name: String,
    provider: AlloyProvider,
}

impl Builders {
    pub fn new(
        mempool: String,
        builders: &[Builder],
        accounts: HashMap<eth::Address, Account>,
    ) -> Self {
        let endpoints = builders
            .iter()
            .map(|builder| Endpoint {
                name: builder.name.clone(),
                provider: unbuffered_web3(&builder.url).provider,
            })
            .collect();
        Self {
            mempool,
            endpoints,
            accounts,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.endpoints.is_empty()
    }

    /// Sends the signed transaction to every builder, as one bundle for each
    /// block in `blocks`. Succeeds as long as one builder accepted a bundle.
    pub async fn send(
        &self,
        tx: &TxEnvelope,
        signer: eth::Address,
        blocks: RangeInclusive<u64>,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(!blocks.is_empty(), "no block left before the deadline");
        let account = self
            .accounts
            .get(&signer)
            .with_context(|| format!("no account registered for {signer}"))?;
        let raw = Bytes::from(tx.encoded_2718());

        // Bundles are per-block, all with the same nonce, if one tx fails it
        // gets dropped. Next block comes around, same thing happens, if
        // the tx passes it any subsequent tx will fail because of the nonce
        // if it fails, it gets dropped and tried again in the following block.
        let requests = blocks.flat_map(|block| {
            let bundle = EthSendBundle {
                txs: vec![raw.clone()],
                block_number: block,
                ..Default::default()
            };
            self.endpoints
                .iter()
                .map(move |endpoint| self.send_bundle(endpoint, bundle.clone(), account))
        });
        let results = join_all(requests).await;
        let accepted = results.iter().filter(|accepted| **accepted).count();

        tracing::debug!(
            hash = ?tx.tx_hash(),
            accepted,
            total = results.len(),
            "sent bundles to builders"
        );
        match accepted {
            0 => Err(anyhow!("no builder accepted the bundles")),
            _ => Ok(()),
        }
    }

    /// Sends one bundle to one builder and reports whether it was accepted.
    async fn send_bundle(
        &self,
        endpoint: &Endpoint,
        bundle: EthSendBundle,
        account: &Account,
    ) -> bool {
        let block = bundle.block_number;
        let start = Instant::now();
        let result = tokio::time::timeout(
            REQUEST_TIMEOUT,
            endpoint
                .provider
                .send_bundle(bundle)
                .with_auth(account.clone()),
        )
        .await
        .context("builder timed out")
        .and_then(|response| Ok(response?));
        observe::builder_response_time(&self.mempool, &endpoint.name, start.elapsed());
        observe::builder_submission(&self.mempool, &endpoint.name, block, &result);
        result.is_ok()
    }
}
