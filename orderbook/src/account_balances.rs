use anyhow::{Context, Result};
use ethcontract::{batch::CallBatch, Http, Web3};
use futures::future::{join3, join_all};
use std::collections::HashMap;
use std::sync::Mutex;

use contracts::IERC20;
use primitive_types::{H160, U256};

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
}

pub struct Web3BalanceFetcher {
    web3: Web3<Http>,
    allowance_manager: H160,
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
    pub fn new(web3: Web3<Http>, allowance_manager: H160) -> Self {
        Self {
            web3,
            allowance_manager,
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
                let instance = IERC20::at(&self.web3, subscription.token);
                join3(
                    instance
                        .balance_of(subscription.owner)
                        .view()
                        .batch_call(&mut batch),
                    instance
                        .allowance(subscription.owner, self.allowance_manager)
                        .view()
                        .batch_call(&mut batch),
                    std::future::ready(subscription),
                )
            })
            .collect::<Vec<_>>();

        batch
            .execute_all()
            .await
            .context("Batch call to fetch balances failed")?;

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::ERC20Mintable;
    use ethcontract::prelude::Account;

    #[tokio::test]
    #[ignore]
    async fn watch_testnet_balance() {
        let http = Http::new("http://127.0.0.1:8545").expect("transport failure");
        let web3 = Web3::new(http);

        let accounts: Vec<H160> = web3.eth().accounts().await.expect("get accounts failed");
        let trader = Account::Local(accounts[0], None);

        let allowance_target = Account::Local(accounts[1], None);

        let token = ERC20Mintable::builder(&web3)
            .gas(8_000_000u32.into())
            .deploy()
            .await
            .expect("MintableERC20 deployment failed");

        let fetcher = Web3BalanceFetcher::new(web3, allowance_target.address());

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
