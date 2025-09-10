use {
    alloy::{
        consensus::{TxEnvelope, TypedTransaction},
        network::{EthereumWallet, Network, NetworkWallet, TxSigner},
        primitives::Address,
        signers::{
            Signature,
            local::{MnemonicBuilder, coins_bip39::English},
        },
        transports::impl_future,
    },
    std::{sync::Arc, thread},
    tokio::sync::RwLock,
};

#[derive(Debug, Clone)]
pub struct MutWallet(Arc<RwLock<EthereumWallet>>);

impl MutWallet {
    pub fn new(wallet: EthereumWallet) -> Self {
        Self(Arc::new(RwLock::new(wallet)))
    }
}

impl MutWallet {
    pub fn anvil_wallet() -> Self {
        let phrase = "test test test test test test test test test test test junk";
        let mut signers = (0..10).map(|i| {
            MnemonicBuilder::<English>::default()
                .phrase(phrase)
                .index(i)
                .unwrap()
                .build()
                .unwrap()
        });

        let mut wallet = EthereumWallet::new(signers.next().unwrap());
        for signer in signers {
            wallet.register_signer(signer);
        }

        Self::new(wallet)
    }

    pub fn register_signer<S>(&mut self, signer: S)
    where
        S: TxSigner<Signature> + Send + Sync + 'static,
    {
        let wallet = self.0.clone();
        thread::spawn(move || {
            let mut w_lock = wallet.blocking_write();
            w_lock.register_signer(signer);
        })
        .join()
        .expect("failed to join spawned thread")
    }
}

impl<N> NetworkWallet<N> for MutWallet
where
    N: Network<UnsignedTx = TypedTransaction, TxEnvelope = TxEnvelope>,
{
    /// Get the default signer address. This address should be used
    /// in [`NetworkWallet::sign_transaction_from`] when no specific signer is
    /// specified.
    fn default_signer_address(&self) -> Address {
        let wallet = self.0.clone();
        thread::spawn(move || {
            let r_lock = wallet.blocking_read();
            <EthereumWallet as NetworkWallet<N>>::default_signer_address(&r_lock)
        })
        .join()
        .expect("failed to join the thread")
    }

    /// Return true if the signer contains a credential for the given address.
    fn has_signer_for(&self, address: &Address) -> bool {
        let address = *address;
        let wallet = self.0.clone();
        thread::spawn(move || {
            let r_lock = wallet.blocking_read();
            <EthereumWallet as NetworkWallet<N>>::has_signer_for(&r_lock, &address)
        })
        .join()
        .expect("failed to join the thread")
    }

    /// Return an iterator of all signer addresses.
    fn signer_addresses(&self) -> impl Iterator<Item = Address> {
        let wallet = self.0.clone();
        thread::spawn(move || {
            let r_lock = wallet.blocking_read();
            <EthereumWallet as NetworkWallet<N>>::signer_addresses(&r_lock).collect::<Vec<_>>()
        })
        .join()
        .expect("failed to join the thread")
        .into_iter()
    }

    /// Asynchronously sign an unsigned transaction, with a specified
    /// credential.
    #[doc(alias = "sign_tx_from")]
    fn sign_transaction_from(
        &self,
        sender: Address,
        tx: N::UnsignedTx,
    ) -> impl_future!(<Output = alloy::signers::Result<N::TxEnvelope>>) {
        async move {
            let r_lock = self.0.read().await;
            <EthereumWallet as NetworkWallet<N>>::sign_transaction_from(&r_lock, sender, tx).await
        }
    }
}
