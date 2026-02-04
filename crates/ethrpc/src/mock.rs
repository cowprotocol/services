//! Mockable Web3 transport implementation.

use {
    crate::{Web3, alloy::MutWallet},
    alloy::providers::{Provider, ProviderBuilder, mock::Asserter},
};

impl Web3 {
    pub fn with_asserter(asserter: Asserter) -> Self {
        Web3 {
            // this will not behave like the original mock transport but it's only used
            // in one place so let's keep this for now and fix it when we switch to
            // alloy in the 1 place that uses the mock provider.
            provider: ProviderBuilder::new()
                .connect_mocked_client(asserter)
                .erased(),
            wallet: MutWallet::default(),
        }
    }
}

pub fn web3() -> Web3 {
    Web3 {
        provider: ProviderBuilder::new()
            .connect_mocked_client(Asserter::new())
            .erased(),
        wallet: MutWallet::default(),
    }
}
