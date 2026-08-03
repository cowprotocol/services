use {
    alloy_consensus::{TxEnvelope, TypedTransaction},
    alloy_network::{Ethereum, EthereumWallet, Network, NetworkWallet, TxSigner},
    alloy_primitives::Address,
    alloy_signer::Signature,
    std::{
        ops::Deref,
        sync::{Arc, RwLock},
    },
};

/// A mutable version of [`EthereumWallet`], cheaply cloneable (through
/// [`Arc`]).
// We also wrap the inner [`EthereumWallet`] in an [`Arc`] because
// we don't want to deep clone the entire thing every time we need to
// sign something.
#[derive(Debug, Clone, Default)]
pub struct MutWallet(Arc<RwLock<Arc<EthereumWallet>>>);

impl MutWallet {
    pub fn new(wallet: EthereumWallet) -> Self {
        Self(Arc::new(RwLock::new(Arc::new(wallet))))
    }

    /// Calls the inner [`EthereumWallet`]'s
    /// [`register_signer`](EthereumWallet::register_signer), if no default
    /// signer has been setup (i.e. the wallet was created using
    /// [`MutWallet::default`]) it will register one.
    pub fn register_signer<S>(&self, signer: S)
    where
        S: TxSigner<Signature> + Send + Sync + 'static,
    {
        // If the wallet is created using MutWallet::default(), there will not be
        // default signer; this stops us from *not* using `.from` (since it
        // is filled with the default signer). At the same time, we can't
        // constantly register new default signers, because it breaks the caller's
        // expectations. As such, if the current default signer address is
        // the default address (0x000...000) we register the signer as the
        // default one.
        let mut w_lock = self.0.write().unwrap();
        let default_address =
            <EthereumWallet as NetworkWallet<Ethereum>>::default_signer_address(&w_lock);

        // Using `Arc::make_mut()` here will never lead to a scenario
        // where multiple clones of the original [`MutWallet`] have
        // different sets of signers.
        // We use a write lock to ensure that only 1 caller can add
        // a signer at a time avoiding race conditions.
        // Also we only ever implicitly give out clones of the inner
        // `Arc<EthereumWallet>` in `sign_transaction_from()` but it's
        // impossible to extract the `Arc<EthereumWallet>` out of the
        // returned future.
        // That means the worst that could happen is that `Arc::make_mut()`
        // makes a deep clone instead of just modifying the only `Arc`
        // instance in existence.
        // But in practice this will never happen because we generally
        // first add all signers to the [`MutWallet`] before we start
        // signing transactions.
        if default_address.is_zero() {
            Arc::make_mut(&mut w_lock).register_default_signer(signer);
        } else {
            Arc::make_mut(&mut w_lock).register_signer(signer);
        }
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
        let r_lock = self.0.read().unwrap();
        <EthereumWallet as NetworkWallet<N>>::default_signer_address(&r_lock)
    }

    /// Return true if the signer contains a credential for the given address.
    fn has_signer_for(&self, address: &Address) -> bool {
        let r_lock = self.0.read().unwrap();
        <EthereumWallet as NetworkWallet<N>>::has_signer_for(&r_lock, address)
    }

    /// Return an iterator of all signer addresses.
    fn signer_addresses(&self) -> impl Iterator<Item = Address> {
        let r_lock = self.0.read().unwrap();
        <EthereumWallet as NetworkWallet<N>>::signer_addresses(&r_lock)
            .collect::<Vec<_>>()
            .into_iter()
    }

    /// Asynchronously sign an unsigned transaction, with a specified
    /// credential.
    #[doc(alias = "sign_tx_from")]
    async fn sign_transaction_from(
        &self,
        sender: Address,
        tx: N::UnsignedTx,
    ) -> alloy_signer::Result<N::TxEnvelope> {
        let wallet = Arc::clone(self.0.read().unwrap().deref());
        <EthereumWallet as NetworkWallet<N>>::sign_transaction_from(&wallet, sender, tx).await
    }
}
