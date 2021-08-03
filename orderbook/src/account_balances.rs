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

    // Check that the settlement contract can make use of this user's token balance. This check
    // could fail if the user does not have enough balance, has not given the allowance to the
    // allowance manager or if the token does not allow freely transferring amounts around for
    // for example if it is paused or takes a fee on transfer.
    // If the node supports the trace_callMany we can perform more extensive tests.
    async fn can_transfer(&self, token: H160, from: H160, amount: U256) -> Result<bool>;
}

pub struct Web3BalanceFetcher {
    web3: Web3,
    vault_relayer: H160,
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
    pub fn new(web3: Web3, vault_relayer: H160, settlement_contract: H160) -> Self {
        Self {
            web3,
            vault_relayer,
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
                        .allowance(subscription.owner, self.vault_relayer)
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

    async fn can_transfer_call(&self, token: H160, from: H160, amount: U256) -> bool {
        let instance = ERC20::at(&self.web3, token);
        let calldata = instance
            .transfer_from(from, self.settlement_contract, amount)
            .tx
            .data
            .unwrap();
        let call_request = CallRequest {
            from: Some(self.vault_relayer),
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

    async fn can_transfer(&self, token: H160, from: H160, amount: U256) -> Result<bool> {
        Ok(self.can_transfer_call(token, from, amount).await)
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
    use ethcontract::prelude::Account;
    use hex_literal::hex;
    use shared::transport::create_env_test_transport;

    #[tokio::test]
    #[ignore]
    async fn mainnet_can_transfer() {
        let http = create_env_test_transport();
        let web3 = Web3::new(http);
        let settlement = contracts::GPv2Settlement::deployed(&web3).await.unwrap();
        let allowance = settlement.vault_relayer().call().await.unwrap();
        let fetcher = Web3BalanceFetcher::new(web3, allowance, settlement.address());
        let owner = H160(hex!("07c2af75788814BA7e5225b2F5c951eD161cB589"));
        let token = H160(hex!("dac17f958d2ee523a2206206994597c13d831ec7"));

        fetcher.register(owner, token).await;
        assert!(fetcher.get_balance(owner, token).unwrap() >= U256::from(1000));

        let call_result = fetcher.can_transfer_call(token, owner, 1000.into()).await;
        assert!(call_result);
    }

    #[tokio::test]
    #[ignore]
    async fn mainnet_cannot_transfer() {
        let http = create_env_test_transport();
        let web3 = Web3::new(http);
        let settlement = contracts::GPv2Settlement::deployed(&web3).await.unwrap();
        let allowance = settlement.vault_relayer().call().await.unwrap();
        let fetcher = Web3BalanceFetcher::new(web3, allowance, settlement.address());
        let owner = H160(hex!("78045485dc4ad96f60937dad4b01b118958761ae"));
        // Token takes a fee.
        let token = H160(hex!("bae5f2d8a1299e5c4963eaff3312399253f27ccb"));

        fetcher.register(owner, token).await;
        assert!(fetcher.get_balance(owner, token).unwrap() >= U256::from(1000));

        let call_result = fetcher.can_transfer_call(token, owner, 1000.into()).await;
        // The non trace method is less accurate and thinks the transfer is ok even though it isn't.
        assert!(call_result);
    }

    #[tokio::test]
    #[ignore]
    async fn watch_testnet_balance() {
        let http = create_env_test_transport();
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
