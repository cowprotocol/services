use {
    crate::{
        domain::{mempools, tx_receiver::SignedTx},
        infra::solver::Account,
    },
    alloy::{
        eips::{eip1559::Eip1559Estimation, eip2718::Encodable2718},
        network::{EthereumWallet, NetworkTransactionBuilder, TransactionBuilder},
        rpc::types::TransactionRequest,
    },
    anyhow::Context,
    eth_domain_types as eth,
};

/// The accounts that sign settlement transactions. With a KMS account each
/// signature is an AWS round trip (slow DNS made these slow before), so sign
/// a tx once here and send the same bytes to every receiver.
#[derive(Debug, Clone)]
pub struct Wallet {
    wallet: EthereumWallet,
    chain_id: u64,
}

impl Wallet {
    pub fn new(accounts: impl IntoIterator<Item = Account>, chain_id: u64) -> Self {
        let mut wallet = EthereumWallet::default();
        for account in accounts {
            wallet.register_signer(account);
        }
        Self { wallet, chain_id }
    }

    /// Signs `tx` with the account of `tx.from`.
    pub async fn sign(
        &self,
        tx: &eth::Tx,
        nonce: u64,
        gas_price: Eip1559Estimation,
        gas_limit: eth::Gas,
    ) -> Result<SignedTx, mempools::Error> {
        let gas_limit = gas_limit.0.try_into().map_err(anyhow::Error::from)?;
        let envelope = TransactionRequest::default()
            .from(tx.from)
            .to(tx.to)
            .nonce(nonce)
            .max_fee_per_gas(gas_price.max_fee_per_gas)
            .max_priority_fee_per_gas(gas_price.max_priority_fee_per_gas)
            .gas_limit(gas_limit)
            .input(tx.input.clone().into())
            .value(tx.value.0)
            .access_list(tx.access_list.clone().into())
            // Without it the tx is signed for chain 1.
            .with_chain_id(self.chain_id)
            .build(&self.wallet)
            .await
            .context("failed to sign tx")?;
        Ok(SignedTx {
            raw: envelope.encoded_2718().into(),
            hash: eth::TxId(*envelope.tx_hash()),
            signer: tx.from,
            nonce,
            gas_price,
        })
    }
}
