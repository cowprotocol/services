use ethcontract::{futures::FutureExt, Account, Address, U256};
use lazy_static::lazy_static;
use shared::ethrpc::{create_test_transport, Web3};
use std::{
    fmt::Debug,
    future::Future,
    panic::{self, AssertUnwindSafe},
    sync::Mutex,
};
use web3::{api::Namespace, helpers::CallFuture, Transport};

lazy_static! {
    static ref NODE_MUTEX: Mutex<()> = Mutex::new(());
}

const NODE_HOST: &str = "http://127.0.0.1:8545";

/// *Testing* function that takes a closure and executes it on Ganache.
/// Before each test, it creates a snapshot of the current state of the chain.
/// The saved state is restored at the end of the test.
///
/// Note that tests calling with this function will not be run aymultaneously.
pub async fn test<F, Fut>(f: F)
where
    F: FnOnce(Web3) -> Fut,
    Fut: Future<Output = ()>,
{
    // The mutex guarantees that no more than a test at a time is running on
    // Ganache.
    // Note that the mutex is expected to become poisoned if a test panics. This
    // is not relevant for us as we are not interested in the data stored in
    // it but rather in the locked state.
    let _lock = NODE_MUTEX.lock();

    let http = create_test_transport(NODE_HOST);
    let web3 = Web3::new(http);
    let resetter = Resetter::new(&web3).await;

    // Hack: the closure may actually be unwind unsafe; moreover, `catch_unwind`
    // does not catch some types of panics. In this cases, the state of the node
    // is not restored. This is not considered an issue since this function
    // is supposed to be used in a test environment.
    let result = AssertUnwindSafe(f(web3.clone())).catch_unwind().await;

    resetter.reset().await;

    if let Err(err) = result {
        panic::resume_unwind(err);
    }
}

struct Resetter<T> {
    ganache: EvmApi<T>,
    snapshot_id: U256,
}

impl<T: Transport> Resetter<T> {
    async fn new(web3: &web3::Web3<T>) -> Self {
        let ganache = web3.api::<EvmApi<_>>();
        let snapshot_id = ganache
            .snapshot()
            .await
            .expect("Test network must support evm_snapshot");
        Self {
            ganache,
            snapshot_id,
        }
    }

    async fn reset(&self) {
        self.ganache
            .revert(&self.snapshot_id)
            .await
            .expect("Test network must support evm_revert");
    }
}

#[derive(Debug, Clone)]
pub struct EvmApi<T> {
    transport: T,
}

impl<T: Transport> Namespace<T> for EvmApi<T> {
    fn new(transport: T) -> Self
    where
        Self: Sized,
    {
        EvmApi { transport }
    }

    fn transport(&self) -> &T {
        &self.transport
    }
}

impl<T: Transport> EvmApi<T> {
    pub fn snapshot(&self) -> CallFuture<U256, T::Out> {
        CallFuture::new(self.transport.execute("evm_snapshot", vec![]))
    }

    pub fn revert(&self, snapshot_id: &U256) -> CallFuture<bool, T::Out> {
        let value_id = serde_json::json!(snapshot_id);
        CallFuture::new(self.transport.execute("evm_revert", vec![value_id]))
    }
}

pub struct AccountAssigner {
    pub default_deployer: Account,
    free_accounts: Vec<Account>,
}

impl AccountAssigner {
    pub async fn new(web3: &Web3) -> Self {
        let addresses: Vec<Address> = web3.eth().accounts().await.expect("get accounts failed");
        let mut accounts = addresses.into_iter().map(|addr| Account::Local(addr, None));
        AccountAssigner {
            default_deployer: accounts.next().expect("No accounts available"),
            free_accounts: accounts.collect(),
        }
    }

    pub fn assign_free_account(&mut self) -> Account {
        self.free_accounts.pop().expect("No testing accounts available, consider increasing the number of testing account in the test node")
    }
}
