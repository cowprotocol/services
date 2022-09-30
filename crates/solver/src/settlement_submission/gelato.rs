use super::SubmissionError;
use crate::{
    settlement::Settlement,
    settlement_simulation::{settle_method_builder, tenderly_link},
};
use anyhow::{anyhow, Result};
use contracts::{GPv2Settlement, SolverTrampoline};
use ethcontract::{web3::signing, Account, Bytes, H160, H256, U256};
use hex_literal::hex;
use model::{
    signature::{hashed_eip712_message, EcdsaSignature, EcdsaSigningScheme},
    u256_decimal::DecimalU256,
    DomainSeparator,
};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, skip_serializing_none};
use shared::http_client::HttpClientFactory;
use std::{env, time::Duration};
use web3::{signing::SecretKeyRef, types::TransactionReceipt};

pub async fn relay_settlement(
    contract: &GPv2Settlement,
    settlement: Settlement,
    account: &Account,
) -> Result<TransactionReceipt, SubmissionError> {
    let web3 = contract.raw_instance().web3();

    let key = match account {
        Account::Offline(key, _) => SecretKeyRef::new(key),
        _ => return Err(anyhow!("offline account required for relayed submission").into()),
    };

    let calldata = settle_method_builder(contract, settlement.into(), account.clone())
        .tx
        .data
        .unwrap()
        .0;

    let trampoline = SolverTrampoline::deployed(&web3).await?;
    let domain_separator = DomainSeparator(trampoline.domain_separator().call().await?.0);
    let nonce = trampoline.nonce().call().await?;
    let (solution_hash, signature) =
        sign_trampoline_message(key, domain_separator, &calldata, nonce);

    let relay = trampoline
        .settle(
            Bytes(calldata),
            Bytes(signature.r.0),
            Bytes(signature.s.0),
            signature.v,
        )
        .from(account.clone());

    let current_block = web3.eth().block_number().await?;
    let network = web3.net().version().await?;
    let simulation_link = tenderly_link(current_block.as_u64(), &network, relay.tx.clone());
    tracing::debug!(%simulation_link, "submitting transaction in Gelato relay mode");

    let gelato = GelatoClient::from_env()?;
    let chain_id = web3.eth().chain_id().await?.as_u64();
    gelato
        .sponsored_call(Call {
            chain_id,
            target: relay.tx.to.unwrap(),
            data: relay.tx.data.unwrap().0,
            ..Default::default()
        })
        .await?;

    // Eventually we need a way to monitor the Gelato transaction and wait for
    // it to mine so that we can get a transaction hash to return. For now, just
    // wait a bit and return a "place-holder" transaction receipt.
    tokio::time::sleep(Duration::from_secs(30)).await;
    Ok(TransactionReceipt {
        transaction_hash: solution_hash,
        block_hash: Some(Default::default()),
        block_number: Some(current_block),
        ..Default::default()
    })
}

fn sign_trampoline_message(
    key: SecretKeyRef,
    domain_separator: DomainSeparator,
    calldata: &[u8],
    nonce: U256,
) -> (H256, EcdsaSignature) {
    const SOLUTION_TYPE_HASH: [u8; 32] =
        hex!("7014cf19af88c8fc5ee7e2c42ed71b7b8f804064f82f63de38c0d59473ce7d7c");

    let struct_hash = signing::keccak256(&{
        let mut buffer = [0_u8; 96];
        buffer[0..32].copy_from_slice(&SOLUTION_TYPE_HASH);
        buffer[32..64].copy_from_slice(&signing::keccak256(calldata));
        nonce.to_big_endian(&mut buffer[64..96]);
        buffer
    });

    let solution_hash = hashed_eip712_message(&domain_separator, &struct_hash);
    let signature = EcdsaSignature::sign(
        EcdsaSigningScheme::Eip712,
        &domain_separator,
        &struct_hash,
        key,
    );

    (H256(solution_hash), signature)
}

struct GelatoClient {
    client: reqwest::Client,
    base: Url,
    api_key: String,
}

impl GelatoClient {
    fn from_env() -> Result<Self> {
        Ok(Self {
            client: HttpClientFactory::default().create(),
            base: Url::parse("https://relay.gelato.digital/")?,
            api_key: env::var("GELATO_API_KEY")?,
        })
    }

    async fn sponsored_call(&self, call: Call) -> Result<()> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct CallWithKey<'a> {
            #[serde(flatten)]
            call: &'a Call,
            sponsor_api_key: &'a str,
        }

        self.client
            .post(self.base.join("relays/v2/sponsored-call")?)
            .json(&CallWithKey {
                call: &call,
                sponsor_api_key: &self.api_key,
            })
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}

#[serde_as]
#[skip_serializing_none]
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Call {
    chain_id: u64,
    target: H160,
    #[serde(with = "model::bytes_hex")]
    data: Vec<u8>,
    #[serde_as(as = "Option<DecimalU256>")]
    gas_limit: Option<U256>,
    retries: Option<usize>,
}
