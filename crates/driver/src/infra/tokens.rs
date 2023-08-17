use {
    crate::{domain::eth, infra::Ethereum},
    anyhow::Result,
    std::{
        collections::{HashMap, HashSet},
        sync::RwLock,
    },
};

#[derive(Clone, Debug)]
pub struct Metadata {
    pub decimals: Option<u8>,
    pub symbol: Option<String>,
    /// Current balance of the smart contract.
    pub balance: eth::TokenAmount,
}

/// Provides metadata of tokens.
pub struct Fetcher {
    eth: Ethereum,
    cache: RwLock<HashMap<eth::TokenAddress, Metadata>>,
}

impl Fetcher {
    pub fn new(eth: Ethereum) -> Self {
        Self {
            eth,
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Returns the token's decimals. Returns `None` if the token does not
    /// implement this optional method.
    async fn decimals(&self, token: eth::TokenAddress) -> Result<Option<u8>> {
        let erc20 = self.eth.contract_at::<contracts::ERC20>(token.0);
        match erc20.methods().decimals().call().await {
            // the token does not implement the optional `decimals()` method
            Err(ethcontract::errors::MethodError {
                inner: ethcontract::errors::ExecutionError::Revert(_),
                ..
            }) => Ok(None),
            Err(err) => Err(err.into()),
            Ok(decimals) => Ok(Some(decimals)),
        }
    }

    /// Returns the token's symbol. Returns `None` if the token does not
    /// implement this optional method.
    async fn symbol(&self, token: eth::TokenAddress) -> Result<Option<String>> {
        let erc20 = self.eth.contract_at::<contracts::ERC20>(token.0);
        match erc20.methods().symbol().call().await {
            // the token does not implement the optional `symbol()` method
            Err(ethcontract::errors::MethodError {
                inner: ethcontract::errors::ExecutionError::Revert(_),
                ..
            }) => Ok(None),
            Err(err) => Err(err.into()),
            Ok(decimals) => Ok(Some(decimals)),
        }
    }

    /// Returns the current [`eth::TokenAmount`] balance of the specified
    /// account for a given token.
    async fn balance(
        &self,
        holder: eth::Address,
        token: eth::TokenAddress,
    ) -> Result<eth::TokenAmount> {
        let erc20 = self.eth.contract_at::<contracts::ERC20>(token.0);
        erc20
            .methods()
            .balance_of(holder.0)
            .call()
            .await
            .map(Into::into)
            .map_err(Into::into)
    }

    /// Fetches `Metadata` of the requested tokens from a node.
    async fn fetch_token_infos(
        &self,
        tokens: HashSet<eth::TokenAddress>,
    ) -> Vec<Result<(eth::TokenAddress, Metadata)>> {
        let settlement = self.eth.contracts().settlement().address().into();
        let futures = tokens.into_iter().map(|token| async move {
            // Use `try_join` because these calls get batched under the hood
            // so if one of them fails the others will as well.
            // Also this way we won't get incomplete data for a token.
            let (decimals, symbol, balance) = futures::future::try_join3(
                self.decimals(token),
                self.symbol(token),
                self.balance(settlement, token),
            )
            .await?;
            Ok((
                token,
                Metadata {
                    decimals,
                    symbol,
                    balance,
                },
            ))
        });
        futures::future::join_all(futures).await
    }

    /// Returns the `Metadata` for the given tokens. Note that the result will
    /// not contain data for tokens that encountered errors while fetching
    /// the data.
    pub async fn get(
        &self,
        addresses: &[eth::TokenAddress],
    ) -> HashMap<eth::TokenAddress, Metadata> {
        let to_fetch: HashSet<_> = {
            let cache = self.cache.read().unwrap();

            // Compute set of requested addresses that are not in cache.
            addresses
                .iter()
                .filter(|address| !cache.contains_key(*address))
                .cloned()
                .collect()
        };

        // Fetch token infos not yet in cache.
        if !to_fetch.is_empty() {
            let fetched = self.fetch_token_infos(to_fetch).await;

            // Add valid token infos to cache.
            self.cache
                .write()
                .unwrap()
                .extend(fetched.into_iter().flatten());
        };

        let cache = self.cache.read().unwrap();
        // Return token infos from the cache.
        addresses
            .iter()
            .filter_map(|address| Some((*address, cache.get(address)?.clone())))
            .collect()
    }
}
