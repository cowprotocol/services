//! Submission of settlements to block builders over `eth_sendBundle`.
//!
//! A settlement goes out as a bundle of one transaction without
//! `revertingTxHashes`, so a builder drops it instead of mining it when it
//! reverts.

use {
    crate::{
        domain::{
            blockchain::TxStatus,
            competition::solution::settlement::EncodedSettlement,
            mempools::{self, Outcome, SubmissionMode, SubmissionSuccess, TransactionReceiver},
            submission,
        },
        infra::{Ethereum, mempool, observe, solver::Account},
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
    ethrpc::block_stream::BlockInfo,
    futures::{StreamExt, future::join_all},
    std::{
        collections::HashMap,
        ops::RangeInclusive,
        time::{Duration, Instant},
    },
    tokio_stream::wrappers::WatchStream,
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
    builders: Vec<Builder>,
    /// Builders only take bundles: nonces, gas prices and receipts come from
    /// the node.
    ethereum: Ethereum,
    /// The accounts that sign the settlements, by address. Each one also signs
    /// the `X-Flashbots-Signature` header of its bundles, the identity that
    /// builders like BuilderNet pay refunds to.
    accounts: HashMap<eth::Address, Account>,
}

/// One block builder that takes bundles over `eth_sendBundle`.
#[derive(Debug, Clone)]
struct Builder {
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
    pub fn new(configs: &[Config], solver_accounts: Vec<Account>, ethereum: Ethereum) -> Self {
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
        let builders = configs
            .iter()
            .map(|config| Builder {
                name: config.name.clone(),
                url: config.url.clone(),
                client: client.clone(),
            })
            .collect();
        Self {
            builders,
            ethereum,
            accounts,
        }
    }

    /// Sends the settlement as one bundle for each block until the deadline,
    /// then waits until it is mined or the deadline passes. Builders drop
    /// reverting bundles, so there is nothing to simulate first.
    async fn send_bundles(
        &self,
        settlement: &EncodedSettlement,
        submission_deadline: BlockNo,
        mode: &SubmissionMode,
    ) -> Result<SubmissionSuccess, mempools::Error> {
        let ethereum = &self.ethereum;
        let tx = mempools::prepare_submission(&settlement.internalized, mode);
        let signer = tx.from;
        let block_stream = submission::next_blocks(ethereum).await;

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
        if !blocks.is_empty() {
            return Err(anyhow!("no block left before the deadline").into());
        };

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

        self.wait_until_mined(block_stream, hash, submission_block, submission_deadline)
            .await
    }

    /// Watches each new block until the settlement is mined or the deadline
    /// passes. The stream must start before the bundles go out, so that no
    /// block is missed.
    async fn wait_until_mined(
        &self,
        mut block_stream: WatchStream<BlockInfo>,
        hash: eth::TxId,
        submission_block: BlockNo,
        submission_deadline: BlockNo,
    ) -> Result<SubmissionSuccess, mempools::Error> {
        // Builders drop bundles that revert, so the settlement either lands in
        // one of the blocks or expires with the last bundle.
        while let Some(block) = block_stream.next().await {
            let block = BlockNo(block.number);
            let status = self
                .ethereum
                .transaction_status(&hash)
                .await
                .unwrap_or_else(|err| {
                    tracing::warn!(?hash, ?err, "failed to get transaction status");
                    TxStatus::Pending
                });
            match status {
                TxStatus::Executed { block_number } => {
                    return Ok(SubmissionSuccess {
                        tx_hash: hash,
                        submitted_at_block: submission_block,
                        included_in_block: block_number,
                    });
                }
                TxStatus::Reverted { block_number } => {
                    return Err(mempools::Error::Revert {
                        tx_id: hash,
                        submitted_at_block: submission_block,
                        reverted_at_block: block_number,
                    });
                }
                TxStatus::Pending if block >= submission_deadline => {
                    // The receipt can lag behind the block, the settlement
                    // event cannot.
                    if let Ok(Some(included_in_block)) = self
                        .ethereum
                        .successful_settlement_block(hash, submission_block.0)
                        .await
                    {
                        return Ok(SubmissionSuccess {
                            tx_hash: hash,
                            submitted_at_block: submission_block,
                            included_in_block,
                        });
                    }
                    return Err(mempools::Error::Expired {
                        tx_id: hash,
                        submitted_at_block: submission_block,
                        submission_deadline,
                    });
                }
                TxStatus::Pending => {}
            }
        }
        Err(mempools::Error::Other(anyhow!(
            "Block stream finished unexpectedly"
        )))
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
        // `eth_sendBundle` takes the bytes `eth_sendRawTransaction` takes: the
        // EIP-2718 envelope (`type || rlp(fields)`) of the signed EIP-1559 tx,
        // not the RLP-wrapped form that block bodies use.
        let raw = &Bytes::from(tx.encoded_2718());

        // One bundle for each block, all with the same tx. Once one lands, the
        // others become invalid because of the nonce. A bundle that reverts is
        // dropped, and the bundle for the next block tries again.
        // Wait for every block and builder (each request has a timeout), so
        // the bundles for the later blocks are not cancelled.
        let results = join_all(blocks.map(|block| async move {
            let request = sign_request(raw, block, account)
                .await
                .inspect_err(|err| tracing::warn!(block, ?err, "failed to sign the bundle"))?;
            let accepted =
                join_all(self.builders.iter().map(|builder| builder.send(&request))).await;
            Ok(accepted.contains(&true))
        }))
        .await;
        // A bundle went out, so the tx can land even if other blocks failed.
        if results.iter().any(|result| matches!(result, Ok(true))) {
            return Ok(());
        }
        match results.into_iter().find_map(Result::err) {
            Some(err) => Err(err),
            None => Err(anyhow!("no builder accepted the bundles")),
        }
    }
}

#[async_trait::async_trait]
impl TransactionReceiver for Builders {
    async fn execute(
        &self,
        settlement: &EncodedSettlement,
        submission_deadline: BlockNo,
        mode: &SubmissionMode,
    ) -> Result<eth::TxId, mempools::Error> {
        let result = self
            .send_bundles(settlement, submission_deadline, mode)
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

impl Builder {
    /// Sends one bundle and reports whether the builder accepted it.
    async fn send(&self, request: &SignedRequest) -> bool {
        let start = Instant::now();
        let result = tokio::time::timeout(REQUEST_TIMEOUT, self.post(request))
            .await
            .context("builder timed out")
            .and_then(|result| result);
        observe::builder_response_time(&self.name, start.elapsed());
        observe::builder_submission(&self.name, request.block, &result);
        result.is_ok()
    }

    async fn post(&self, request: &SignedRequest) -> anyhow::Result<Option<EthBundleHash>> {
        let response = self
            .client
            .post(self.url.clone())
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
}
