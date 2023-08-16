pub use shared::token_info::TokenInfo;
use {
    crate::{boundary, domain::eth, infra::Ethereum},
    shared::token_info::{
        CachedTokenInfoFetcher,
        TokenInfoFetcher as BaseFetcher,
        TokenInfoFetching,
    },
    std::collections::HashMap,
};

pub struct Fetcher(CachedTokenInfoFetcher);

impl Fetcher {
    pub fn new(eth: &Ethereum) -> Self {
        let web3 = boundary::web3(eth);
        let fetcher = BaseFetcher { web3 };
        let cached = CachedTokenInfoFetcher::new(Box::new(fetcher));
        Self(cached)
    }

    pub async fn get_token_infos(
        &self,
        addresses: &[eth::TokenAddress],
    ) -> HashMap<eth::TokenAddress, TokenInfo> {
        let mapped: Vec<_> = addresses.iter().map(|addr| addr.0 .0).collect();
        let infos = self.0.get_token_infos(&mapped).await;
        infos
            .into_iter()
            .map(|(address, info)| (eth::TokenAddress(eth::ContractAddress(address)), info))
            .collect()
    }
}
