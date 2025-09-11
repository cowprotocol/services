use {
    alloy::{
        consensus::{TxEnvelope, TypedTransaction},
        network::{Ethereum, EthereumWallet, Network, NetworkWallet, TxSigner},
        primitives::Address,
        signers::Signature,
        transports::impl_future,
    },
    std::{sync::Arc, thread},
    tokio::sync::RwLock,
};

/// A mutable version of [`EthereumWallet`], cheaply cloneable (through
/// [`Arc`]).
///
/// Requires a tokio runtime to be present, otherwise operations will panic.
#[derive(Debug, Clone, Default)]
pub struct MutWallet(Arc<RwLock<EthereumWallet>>);

impl MutWallet {
    pub fn new(wallet: EthereumWallet) -> Self {
        Self(Arc::new(RwLock::new(wallet)))
    }
}

impl MutWallet {
    /// Calls the inner [`EthereumWallet`]'s
    /// [`register_signer`](EthereumWallet::register_signer), if no default
    /// signer has been setup (i.e. the wallet was created using
    /// [`MutWallet::default`]) it will register one.
    pub fn register_signer<S>(&mut self, signer: S)
    where
        S: TxSigner<Signature> + Send + Sync + 'static,
    {
        self.handle_blocking_operation(move |wallet| {
            // If the wallet is created using MutWallet::default(), there will not be
            // default signer; this stops us from *not* using `.from` (since it
            // is filled with the default signer). At the same time, we can't
            // constantly register new default signers, because it breaks the caller's
            // expectations. As such, if the current default signer address is
            // the default address (0x000...000) we register the signer as the
            // default one.
            let register_default = {
                let r_lock = wallet.0.blocking_read();
                let default_address =
                    <EthereumWallet as NetworkWallet<Ethereum>>::default_signer_address(&r_lock);

                default_address == Address::default()
            };

            let mut w_lock = wallet.0.blocking_write();
            if register_default {
                w_lock.register_default_signer(signer);
            } else {
                w_lock.register_signer(signer);
            }
        });
    }

    /// Handles blocking operations such as the
    /// [`blocking_read`](RwLock::blocking_read)
    /// and [`blocking_write`](RwLock::blocking_write).
    ///
    /// This function *will panic* in case there is no runtime present, or the
    /// runtime flavor is not `current_thread` or `multi_thread`.
    // This function is necessary to handle the blocking lock operations under
    // required by synchronous function calls which are problematic when the runtime
    // flavour is `current_thread` (which will panic when blocked by certain
    // operations).
    fn handle_blocking_operation<F, R>(&self, f: F) -> R
    where
        F: FnOnce(Self) -> R + Send + 'static,
        R: Send + 'static,
    {
        let wallet = self.clone();
        let rt = tokio::runtime::Handle::current();

        match rt.runtime_flavor() {
            tokio::runtime::RuntimeFlavor::CurrentThread => thread::spawn(move || f(wallet))
                .join()
                .expect("failed to join thread"),
            tokio::runtime::RuntimeFlavor::MultiThread => {
                tokio::task::block_in_place(move || f(wallet))
            }
            _ => panic!("unsupported runtime flavor"),
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
        self.handle_blocking_operation(|wallet| {
            let r_lock = wallet.0.blocking_read();
            <EthereumWallet as NetworkWallet<N>>::default_signer_address(&r_lock)
        })
    }

    /// Return true if the signer contains a credential for the given address.
    fn has_signer_for(&self, address: &Address) -> bool {
        let address = *address;
        self.handle_blocking_operation(move |wallet| {
            let r_lock = wallet.0.blocking_read();
            <EthereumWallet as NetworkWallet<N>>::has_signer_for(&r_lock, &address)
        })
    }

    /// Return an iterator of all signer addresses.
    fn signer_addresses(&self) -> impl Iterator<Item = Address> {
        self.handle_blocking_operation(move |wallet| {
            let r_lock = wallet.0.blocking_read();
            <EthereumWallet as NetworkWallet<N>>::signer_addresses(&r_lock).collect::<Vec<_>>()
        })
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
