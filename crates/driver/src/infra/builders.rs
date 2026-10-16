//! Submission of settlements to block builders over `eth_sendBundle`.
//!
//! A settlement goes out as a bundle of one transaction without
//! `revertingTxHashes`, so a builder drops it instead of mining it when it
//! reverts.

use {
    crate::{
        domain::{
            competition::solution::settlement::EncodedSettlement,
            mempools::{self, Outcome, SubmissionMode, SubmissionSuccess},
        },
        infra::{self, mempool, observe, solver::Account},
    },
    alloy::{
        consensus::TxEnvelope,
        eips::eip2718::Encodable2718,
        network::{EthereumWallet, NetworkTransactionBuilder, TxSigner},
        primitives::Bytes,
        providers::{Provider, ext::sign_flashbots_payload},
        rpc::{
            json_rpc::{Id, Request, Response},
            types::{
                TransactionRequest,
                mev::{EthBundleHash, EthSendBundle},
            },
        },
    },
    anyhow::{Context, anyhow},
    eth_domain_types::{self as eth, BlockNo},
    ethrpc::block_stream::into_stream,
    futures::{StreamExt, future::try_join_all, stream::FuturesUnordered},
    std::{
        collections::HashMap,
        ops::{ControlFlow, RangeInclusive},
        time::{Duration, Instant},
    },
    tracing::Instrument,
    url::Url,
};

/// A builder that stops answering must not hold up the others: submission is
/// on the critical path of the block.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);

/// A block builder that takes settlement bundles.
#[derive(Debug, Clone)]
pub struct Config {
    /// Name of the builder in logs and metrics.
    pub name: String,
    pub url: Url,
}

/// Signs settlements and sends them to every configured block builder.
#[derive(Debug, Clone)]
pub struct Builders {
    endpoints: Vec<Endpoint>,
    /// The accounts that sign the settlements, by address. Each one also signs
    /// the `X-Flashbots-Signature` header of its bundles, the identity that
    /// builders like BuilderNet pay refunds to.
    accounts: HashMap<eth::Address, Account>,
}

#[derive(Debug, Clone)]
struct Endpoint {
    name: String,
    url: Url,
    client: reqwest::Client,
}

/// An `eth_sendBundle` request and its `X-Flashbots-Signature` header.
#[derive(Clone)]
struct SignedRequest {
    block: u64,
    body: String,
    signature: String,
}

impl Builders {
    pub fn new(configs: &[Config], solver_accounts: Vec<Account>) -> Self {
        let mut accounts = HashMap::new();
        for account in solver_accounts {
            let address = TxSigner::address(&account);
            // Builders get txs signed here, which an address-only account
            // leaves to the node.
            assert!(
                !matches!(account, Account::Address(_)),
                "builders need txs signed by the driver, but account {address} cannot sign"
            );
            accounts.insert(address, account);
        }
        let client = reqwest::Client::new();
        let endpoints = configs
            .iter()
            .map(|config| Endpoint {
                name: config.name.clone(),
                url: config.url.clone(),
                client: client.clone(),
            })
            .collect();
        Self {
            endpoints,
            accounts,
        }
    }

    /// Signs the transaction and sends it to every builder, as one bundle for
    /// each block in `blocks`. Succeeds as long as one builder accepted a
    /// bundle.
    async fn submit(
        &self,
        tx: TransactionRequest,
        signer: eth::Address,
        blocks: RangeInclusive<u64>,
    ) -> Result<eth::TxId, mempools::Error> {
        let account = self
            .accounts
            .get(&signer)
            .with_context(|| format!("no account registered for {signer}"))?;
        let envelope = tx
            .build(&EthereumWallet::from(account.clone()))
            .await
            .context("failed to sign the tx for the builders")?;
        self.send(&envelope, account, blocks).await?;
        Ok(eth::TxId(*envelope.tx_hash()))
    }

    async fn send(
        &self,
        tx: &TxEnvelope,
        account: &Account,
        blocks: RangeInclusive<u64>,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(!blocks.is_empty(), "no block left before the deadline");
        // `eth_sendBundle` takes the bytes `eth_sendRawTransaction` takes: the
        // EIP-2718 envelope (`type || rlp(fields)`) of the signed EIP-1559 tx,
        // not the RLP-wrapped form that block bodies use.
        let raw = Bytes::from(tx.encoded_2718());

        // One bundle for each block, all with the same tx. Once one lands, the
        // others become invalid because of the nonce. A bundle that reverts is
        // dropped, and the bundle for the next block tries again.
        let signed = try_join_all(blocks.map(|block| sign_request(&raw, block, account))).await?;
        let mut requests: FuturesUnordered<_> = signed
            .iter()
            .flat_map(|request| {
                self.endpoints
                    .iter()
                    .map(|endpoint| send_bundle(endpoint.clone(), request.clone()))
            })
            .collect();

        while let Some(accepted) = requests.next().await {
            if accepted {
                // Dropping the other requests would cancel them, and with them
                // the bundles for the later blocks.
                tokio::spawn(requests.collect::<Vec<_>>().in_current_span());
                return Ok(());
            }
        }
        Err(anyhow!("no builder accepted the bundles"))
    }
}

#[async_trait::async_trait]
impl mempools::Submitter for Builders {
    async fn execute(
        &self,
        ethereum: &infra::Ethereum,
        settlement: &EncodedSettlement,
        submission_deadline: BlockNo,
        mode: &SubmissionMode,
    ) -> Result<eth::TxId, mempools::Error> {
        let result = self
            .send_bundles(ethereum, settlement, submission_deadline, mode)
            .instrument(tracing::info_span!("builders"))
            .await;
        observe::builders_log(settlement, &result);
        let outcome = Outcome::from(&result);
        observe::mempool_submission_result(
            "builders",
            outcome.metric_label(),
            outcome.blocks_passed(),
        );
        Ok(result?.tx_hash)
    }
}

impl Builders {
    /// Sends the settlement as one bundle for each block until the deadline,
    /// then waits until it is mined or the deadline passes. Builders drop
    /// reverting bundles and bundles expire with their block, so unlike a
    /// mempool tx there is nothing to simulate first or to cancel.
    async fn send_bundles(
        &self,
        ethereum: &infra::Ethereum,
        settlement: &EncodedSettlement,
        submission_deadline: BlockNo,
        mode: &SubmissionMode,
    ) -> Result<SubmissionSuccess, mempools::Error> {
        let tx = mempools::prepare_submission(&settlement.internalized, mode);
        let signer = tx.from;

        // Skip the current block, the bundles can only land in later ones.
        let mut block_stream = into_stream(ethereum.current_block().clone());
        block_stream.next().await;

        let (nonce, current_gas_price) = futures::try_join!(
            async {
                ethereum
                    .web3()
                    .provider
                    .get_transaction_count(signer)
                    .await
                    .context("failed to fetch nonce")
            },
            async {
                ethereum
                    .gas_price()
                    .await
                    .context("failed to compute current gas price")
            },
        )?;
        let gas_price =
            mempools::apply_gas_fee_override(current_gas_price, settlement.gas_fee_override, None);
        let submission_block: BlockNo = ethereum.current_block().borrow().number.into();
        // A mempool tx stays valid until the deadline. A bundle names a single
        // block, so builders get one bundle for each block in this range.
        let blocks = submission_block.0 + 1..=submission_deadline.0;

        tracing::debug!(
            ?submission_block,
            ?submission_deadline,
            ?gas_price,
            ?signer,
            "sending settlement bundles"
        );
        let gas_limit = settlement
            .gas
            .limit
            .0
            .try_into()
            .map_err(anyhow::Error::from)?;
        let mut tx_request = mempool::tx_request(tx, gas_price, gas_limit, signer, nonce);
        // Without a chain id the signed tx silently falls back to mainnet.
        tx_request.chain_id = Some(ethereum.chain().id());
        let hash = self.submit(tx_request, signer, blocks).await?;

        // No builder reports a dropped bundle in time (Titan's bundle stats
        // lag ~5 min), so a settlement that starts to revert ends as
        // `Expired`, not `SimulationRevert`.
        mempools::wait_for_inclusion(
            ethereum,
            block_stream,
            hash,
            submission_block,
            submission_deadline,
            |_| async { ControlFlow::Continue(()) },
        )
        .await
    }
}

/// Signs the `eth_sendBundle` request for `block` once for all builders.
/// `MevApi::with_auth` would sign each request: with KMS that is one network
/// call per builder and block, and slow DNS has made KMS slow before.
async fn sign_request(raw: &Bytes, block: u64, account: &Account) -> anyhow::Result<SignedRequest> {
    let bundle = EthSendBundle {
        txs: vec![raw.clone()],
        block_number: block,
        ..Default::default()
    };
    let body = serde_json::to_string(&Request::new("eth_sendBundle", Id::Number(1), (bundle,)))?;
    let signature = sign_flashbots_payload(body.clone(), account)
        .await
        .context("failed to sign the bundle request")?;
    Ok(SignedRequest {
        block,
        body,
        signature,
    })
}

/// Sends one bundle to one builder and reports whether it was accepted.
async fn send_bundle(endpoint: Endpoint, request: SignedRequest) -> bool {
    let start = Instant::now();
    let result = tokio::time::timeout(REQUEST_TIMEOUT, post(&endpoint, &request))
        .await
        .context("builder timed out")
        .and_then(|result| result);
    observe::builder_response_time(&endpoint.name, start.elapsed());
    observe::builder_submission(&endpoint.name, request.block, &result);
    result.is_ok()
}

async fn post(
    endpoint: &Endpoint,
    request: &SignedRequest,
) -> anyhow::Result<Option<EthBundleHash>> {
    let response = endpoint
        .client
        .post(endpoint.url.clone())
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header("X-Flashbots-Signature", &request.signature)
        .body(request.body.clone())
        .send()
        .await?;
    let status = response.status();
    let body = response.text().await?;
    let response: Response<Option<EthBundleHash>> = serde_json::from_str(&body)
        .with_context(|| format!("unexpected response, status {status}: {body}"))?;
    response
        .payload
        .try_into_success()
        .map_err(|err| anyhow!("{err}"))
}
