use anyhow::Result;
use contracts::ERC20;
use ethcontract::batch::CallBatch;
use futures::future::{join3, join_all};
use primitive_types::{H160, U256};
use shared::Web3;
use std::{collections::HashMap, sync::Mutex};
use web3::types::{BlockId, BlockNumber, CallRequest};

const MAX_BATCH_SIZE: usize = 100;

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait BalanceFetching: Send + Sync {
    // Register owner and address for balance updates in the background
    async fn register(&self, owner: H160, token: H160);

    // Register multiple owner and addresses for balance updates in the background
    async fn register_many(&self, owner_token_list: Vec<(H160, H160)>);

    // Returns the latest balance available to the allowance manager for the given owner and token.
    // Should be non-blocking. Returns None if balance has never been fetched.
    fn get_balance(&self, owner: H160, token: H160) -> Option<U256>;

    // Called periodically to perform potential updates on registered balances
    async fn update(&self);

    // Check if the allowance manager would be able to call transfer_from with these parameters.
    // This is useful for tokens that are not consistent about their internal checks and what they
    // report as balance and allowance. By checking whether the actual transfer would suceed we can
    // be more certain (but still not 100%) that the balance really is available to the settlement
    // contract.
    async fn can_transfer(&self, token: H160, from: H160, amount: U256) -> bool;
}

pub struct Web3BalanceFetcher {
    web3: Web3,
    allowance_manager: H160,
    settlement_contract: H160,
    // Mapping of address, token to balance, allowance
    balances: Mutex<HashMap<SubscriptionKey, SubscriptionValue>>,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
struct SubscriptionKey {
    owner: H160,
    token: H160,
}

#[derive(Clone, Default)]
struct SubscriptionValue {
    balance: Option<U256>,
    allowance: Option<U256>,
}

impl Web3BalanceFetcher {
    pub fn new(web3: Web3, allowance_manager: H160, settlement_contract: H160) -> Self {
        Self {
            web3,
            allowance_manager,
            settlement_contract,
            balances: Default::default(),
        }
    }

    async fn _register_many(
        &self,
        subscriptions: impl Iterator<Item = SubscriptionKey>,
    ) -> Result<()> {
        let mut batch = CallBatch::new(self.web3.transport());

        // Make sure subscriptions are registered for next update even if batch call fails
        let subscriptions: Vec<SubscriptionKey> = {
            let mut guard = self.balances.lock().expect("thread holding mutex panicked");
            subscriptions
                .map(|subscription| {
                    let _ = guard.entry(subscription).or_default();
                    subscription
                })
                .collect()
        };

        let calls = subscriptions
            .into_iter()
            .map(|subscription| {
                let instance = ERC20::at(&self.web3, subscription.token);
                join3(
                    instance
                        .balance_of(subscription.owner)
                        .batch_call(&mut batch),
                    instance
                        .allowance(subscription.owner, self.allowance_manager)
                        .batch_call(&mut batch),
                    std::future::ready(subscription),
                )
            })
            .collect::<Vec<_>>();

        batch.execute_all(usize::MAX).await;

        let call_results = join_all(calls).await;
        let mut guard = self.balances.lock().expect("thread holding mutex panicked");
        for (balance, allowance, subscription) in call_results {
            let entry = guard.entry(subscription).or_default();

            match balance {
                Ok(balance) => entry.balance = Some(balance),
                Err(_) => tracing::warn!("Couldn't fetch balance for {:?}", subscription),
            }
            match allowance {
                Ok(allowance) => entry.allowance = Some(allowance),
                Err(_) => tracing::warn!("Couldn't fetch allowance for {:?}", subscription),
            }
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl BalanceFetching for Web3BalanceFetcher {
    async fn register(&self, owner: H160, token: H160) {
        self.register_many(vec![(owner, token)]).await;
    }

    async fn register_many(&self, owner_token_list: Vec<(H160, H160)>) {
        for chunk in owner_token_list.chunks(MAX_BATCH_SIZE) {
            let subscriptions = chunk.iter().map(|(owner, token)| SubscriptionKey {
                owner: *owner,
                token: *token,
            });
            let _ = self._register_many(subscriptions).await;
        }
    }

    fn get_balance(&self, owner: H160, token: H160) -> Option<U256> {
        let subscription = SubscriptionKey { owner, token };
        let SubscriptionValue { balance, allowance } = self
            .balances
            .lock()
            .expect("thread holding mutex panicked")
            .get(&subscription)
            .cloned()
            .unwrap_or_default();
        Some(U256::min(balance?, allowance?))
    }

    async fn update(&self) {
        let subscriptions: Vec<_> = {
            let map = self.balances.lock().expect("mutex holding thread panicked");
            map.keys().cloned().collect()
        };
        let _ = self._register_many(subscriptions.into_iter()).await;
    }

    async fn can_transfer(&self, token: H160, from: H160, amount: U256) -> bool {
        let instance = ERC20::at(&self.web3, token);
        let calldata = instance
            .transfer_from(from, self.settlement_contract, amount)
            .tx
            .data
            .unwrap();
        let call_request = CallRequest {
            from: Some(self.allowance_manager),
            to: Some(token),
            data: Some(calldata),
            ..Default::default()
        };
        let block = Some(BlockId::Number(BlockNumber::Latest));
        let response = self.web3.eth().call(call_request, block).await;
        response
            .map(|bytes| is_empty_or_truthy(bytes.0.as_slice()))
            .unwrap_or(false)
    }
}

fn is_empty_or_truthy(bytes: &[u8]) -> bool {
    match bytes.len() {
        0 => true,
        32 => bytes.iter().any(|byte| *byte > 0),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::ERC20Mintable;
    use ethcontract::{prelude::Account, Http};
    use hex_literal::hex;
    use shared::transport::LoggingTransport;

    #[tokio::test]
    #[ignore]
    async fn rinkeby_can_transfer() {
        let http = LoggingTransport::new(
            Http::new("https://dev-openethereum.rinkeby.gnosisdev.com/").unwrap(),
        );
        let web3 = Web3::new(http);
        let settlement = contracts::GPv2Settlement::deployed(&web3).await.unwrap();
        let allowance = settlement.allowance_manager().call().await.unwrap();
        let fetcher = Web3BalanceFetcher::new(web3, allowance, settlement.address());
        let owner = H160(hex!("52DF85E9De71aa1C210873bcF37EC46d36c99dc2"));
        let token = H160(hex!("5592ec0cfb4dbc12d3ab100b257153436a1f0fea"));

        let result = fetcher.can_transfer(token, owner, 1000.into()).await;
        assert!(result);
    }

    #[tokio::test]
    #[ignore]
    async fn mainnet_cannot_transfer() {
        let http = LoggingTransport::new(
            Http::new("https://dev-openethereum.mainnet.gnosisdev.com/").unwrap(),
        );
        let web3 = Web3::new(http);
        let settlement = contracts::GPv2Settlement::deployed(&web3).await.unwrap();
        let allowance = settlement.allowance_manager().call().await.unwrap();
        let fetcher = Web3BalanceFetcher::new(web3, allowance, settlement.address());
        let owner = H160(hex!("c978f4364c03a00352e8c7d9619b42e26b6424ab"));
        let token = H160(hex!("c12d1c73ee7dc3615ba4e37e4abfdbddfa38907e"));

        // The owner has balance and approval but still the transfer fails.
        fetcher.register(owner, token).await;
        assert!(fetcher.get_balance(owner, token).unwrap() >= U256::from(1000));
        let result = fetcher.can_transfer(token, owner, 1000.into()).await;
        assert!(!result);
    }

    #[tokio::test]
    #[ignore]
    async fn watch_testnet_balance() {
        let http =
            LoggingTransport::new(Http::new("http://127.0.0.1:8545").expect("transport failure"));
        let web3 = Web3::new(http);

        let accounts: Vec<H160> = web3.eth().accounts().await.expect("get accounts failed");
        let trader = Account::Local(accounts[0], None);

        let allowance_target = Account::Local(accounts[1], None);

        let token = ERC20Mintable::builder(&web3)
            .deploy()
            .await
            .expect("MintableERC20 deployment failed");

        let fetcher =
            Web3BalanceFetcher::new(web3, allowance_target.address(), H160::from_low_u64_be(1));

        // Not available until registered
        assert_eq!(fetcher.get_balance(trader.address(), token.address()), None,);

        fetcher.register(trader.address(), token.address()).await;

        assert_eq!(
            fetcher.get_balance(trader.address(), token.address()),
            Some(U256::zero()),
        );

        // Balance without approval should not affect available balance
        token
            .mint(trader.address(), 100.into())
            .send()
            .await
            .unwrap();
        fetcher.update().await;
        assert_eq!(
            fetcher.get_balance(trader.address(), token.address()),
            Some(U256::zero()),
        );

        // Approving allowance_target should increase available balance
        token
            .approve(allowance_target.address(), 200.into())
            .send()
            .await
            .unwrap();
        fetcher.update().await;
        assert_eq!(
            fetcher.get_balance(trader.address(), token.address()),
            Some(100.into()),
        );

        // Spending balance should decrease available balance
        token
            .transfer(allowance_target.address(), 100.into())
            .send()
            .await
            .unwrap();
        fetcher.update().await;
        assert_eq!(
            fetcher.get_balance(trader.address(), token.address()),
            Some(U256::zero()),
        );
    }
}
