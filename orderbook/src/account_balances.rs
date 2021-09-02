use anyhow::{bail, Result};
use contracts::{BalancerV2Vault, ERC20};
use ethcontract::{batch::CallBatch, Account};
use futures::{
    future::{self, join_all, BoxFuture},
    FutureExt as _,
};
use model::order::SellTokenSource;
use primitive_types::{H160, U256};
use shared::{Web3, Web3Transport};
use std::{collections::HashMap, sync::Mutex};
use web3::types::{BlockId, BlockNumber, CallRequest};

const MAX_BATCH_SIZE: usize = 100;

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait BalanceFetching: Send + Sync {
    // Register owner and address for balance updates in the background
    async fn register(&self, owner: H160, token: H160, source: SellTokenSource);

    // Register multiple owner and addresses for balance updates in the background
    async fn register_many(&self, owner_token_list: Vec<(H160, H160, SellTokenSource)>);

    // Returns the latest balance available to the allowance manager for the given owner and token.
    // Should be non-blocking. Returns None if balance has never been fetched.
    fn get_balance(&self, owner: H160, token: H160, source: SellTokenSource) -> Option<U256>;

    // Called periodically to perform potential updates on registered balances
    async fn update(&self);

    // Check that the settlement contract can make use of this user's token balance. This check
    // could fail if the user does not have enough balance, has not given the allowance to the
    // allowance manager or if the token does not allow freely transferring amounts around for
    // for example if it is paused or takes a fee on transfer.
    // If the node supports the trace_callMany we can perform more extensive tests.
    async fn can_transfer(
        &self,
        token: H160,
        from: H160,
        amount: U256,
        source: SellTokenSource,
    ) -> Result<bool>;
}

pub struct Web3BalanceFetcher {
    web3: Web3,
    vault: Option<BalancerV2Vault>,
    vault_relayer: H160,
    settlement_contract: H160,
    // Mapping of address, token to balance, allowance
    balances: Mutex<HashMap<SubscriptionKey, SubscriptionValue>>,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
struct SubscriptionKey {
    owner: H160,
    token: H160,
    source: SellTokenSource,
}

#[derive(Clone, Debug, Default)]
struct SubscriptionValue {
    balance: Option<U256>,
    allowance: Option<U256>,
}

impl Web3BalanceFetcher {
    pub fn new(
        web3: Web3,
        vault: Option<BalancerV2Vault>,
        vault_relayer: H160,
        settlement_contract: H160,
    ) -> Self {
        Self {
            web3,
            vault,
            vault_relayer,
            settlement_contract,
            balances: Default::default(),
        }
    }

    async fn _register_many(&self, subscriptions: Vec<SubscriptionKey>) -> Result<()> {
        let mut batch = CallBatch::new(self.web3.transport());

        // Make sure subscriptions are registered for next update even if batch call fails.
        // Note that we only add an entry if one does not already exist, this allows calls
        // to `get_balance` to immediately return the previously cached value during the
        // update.
        {
            let mut guard = self.balances.lock().expect("thread holding mutex panicked");
            for subscription in &subscriptions {
                guard.entry(*subscription).or_default();
            }
        }

        let calls = subscriptions
            .into_iter()
            .map(|subscription| {
                let token = ERC20::at(&self.web3, subscription.token);
                let value = match (subscription.source, &self.vault) {
                    (SellTokenSource::Erc20, _) => erc20_balance_query(
                        &mut batch,
                        token,
                        subscription.owner,
                        self.vault_relayer,
                    ),
                    (SellTokenSource::External, Some(vault)) => vault_external_balance_query(
                        &mut batch,
                        vault.clone(),
                        token,
                        subscription.owner,
                        self.vault_relayer,
                    ),
                    _ => async { SubscriptionValue::default() }.boxed(),
                };
                future::join(future::ready(subscription), value)
            })
            .collect::<Vec<_>>();

        batch.execute_all(usize::MAX).await;

        let call_results = join_all(calls).await;
        self.balances
            .lock()
            .expect("thread holding mutex panicked")
            .extend(call_results);

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

    async fn can_manage_user_balance_call(&self, token: H160, from: H160, amount: U256) -> bool {
        let vault = match self.vault.as_ref() {
            Some(vault) => vault,
            None => return false,
        };

        const USER_BALANCE_OP_TRANSFER_EXTERNAL: u8 = 3;
        vault
            .manage_user_balance(vec![(
                USER_BALANCE_OP_TRANSFER_EXTERNAL,
                token,
                amount,
                from,
                self.settlement_contract,
            )])
            .from(Account::Local(from, None))
            .call()
            .await
            .is_ok()
    }
}

fn erc20_balance_query<'a>(
    batch: &mut CallBatch<&'a Web3Transport>,
    token: ERC20,
    owner: H160,
    spender: H160,
) -> BoxFuture<'a, SubscriptionValue> {
    let balance = token.balance_of(owner).batch_call(batch);
    let allowance = token.allowance(owner, spender).batch_call(batch);

    async move {
        let (balance, allowance) = futures::join!(balance, allowance);

        let mut value = SubscriptionValue::default();
        match balance {
            Ok(balance) => value.balance = Some(balance),
            Err(_) => tracing::warn!(
                "Couldn't fetch {:?} balance for {:?}",
                token.address(),
                owner
            ),
        }
        match allowance {
            Ok(allowance) => value.allowance = Some(allowance),
            Err(_) => tracing::warn!(
                "Couldn't fetch {:?} allowance from {:?} to {:?}",
                token.address(),
                owner,
                spender
            ),
        }

        value
    }
    .boxed()
}

fn vault_external_balance_query<'a>(
    batch: &mut CallBatch<&'a Web3Transport>,
    vault: BalancerV2Vault,
    token: ERC20,
    owner: H160,
    relayer: H160,
) -> BoxFuture<'a, SubscriptionValue> {
    let erc20 = erc20_balance_query(batch, token, owner, vault.address());
    let approval = vault.has_approved_relayer(owner, relayer).batch_call(batch);

    async move {
        let (value, approval) = futures::join!(erc20, approval);
        match approval {
            Ok(true) => value,
            Ok(false) => SubscriptionValue {
                allowance: Some(0.into()),
                ..value
            },
            Err(_) => {
                tracing::warn!(
                    "Couldn't fetch vault approval from {:?} to {:?}",
                    owner,
                    relayer
                );
                SubscriptionValue::default()
            }
        }
    }
    .boxed()
}

#[async_trait::async_trait]
impl BalanceFetching for Web3BalanceFetcher {
    async fn register(&self, owner: H160, token: H160, source: SellTokenSource) {
        self.register_many(vec![(owner, token, source)]).await;
    }

    async fn register_many(&self, owner_token_list: Vec<(H160, H160, SellTokenSource)>) {
        for chunk in owner_token_list.chunks(MAX_BATCH_SIZE) {
            let subscriptions = chunk
                .iter()
                .map(|(owner, token, source)| SubscriptionKey {
                    owner: *owner,
                    token: *token,
                    source: *source,
                })
                .collect();
            let _ = self._register_many(subscriptions).await;
        }
    }

    fn get_balance(&self, owner: H160, token: H160, source: SellTokenSource) -> Option<U256> {
        let subscription = SubscriptionKey {
            owner,
            token,
            source,
        };
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
        AccountBalanceMetrics::instance()
            .queries
            .set(subscriptions.len() as _);
        let _ = self._register_many(subscriptions).await;
    }

    async fn can_transfer(
        &self,
        token: H160,
        from: H160,
        amount: U256,
        source: SellTokenSource,
    ) -> Result<bool> {
        let success = match source {
            SellTokenSource::Erc20 => self.can_transfer_call(token, from, amount).await,
            SellTokenSource::External => {
                self.can_manage_user_balance_call(token, from, amount).await
            }
            SellTokenSource::Internal => bail!("internal Vault balances not supported"),
        };
        Ok(success)
    }
}

fn is_empty_or_truthy(bytes: &[u8]) -> bool {
    match bytes.len() {
        0 => true,
        32 => bytes.iter().any(|byte| *byte > 0),
        _ => false,
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "account_balance")]
struct AccountBalanceMetrics {
    /// Number accounts whose balances are being tracked.
    queries: prometheus::IntGauge,
}

impl AccountBalanceMetrics {
    fn instance() -> &'static Self {
        lazy_static::lazy_static! {
            static ref INSTANCE: AccountBalanceMetrics =
                AccountBalanceMetrics::new(shared::metrics::get_metrics_registry()).unwrap();
        }

        &INSTANCE
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::{vault, BalancerV2Authorizer, ERC20Mintable};
    use hex_literal::hex;
    use shared::transport::create_env_test_transport;

    #[tokio::test]
    #[ignore]
    async fn mainnet_can_transfer() {
        let http = create_env_test_transport();
        let web3 = Web3::new(http);
        let settlement = contracts::GPv2Settlement::deployed(&web3).await.unwrap();
        let vault_relayer = settlement.vault_relayer().call().await.unwrap();
        let fetcher = Web3BalanceFetcher::new(web3, None, vault_relayer, settlement.address());
        let owner = H160(hex!("07c2af75788814BA7e5225b2F5c951eD161cB589"));
        let token = H160(hex!("dac17f958d2ee523a2206206994597c13d831ec7"));

        fetcher.register(owner, token, SellTokenSource::Erc20).await;
        assert!(
            fetcher
                .get_balance(owner, token, SellTokenSource::Erc20)
                .unwrap()
                >= U256::from(1000)
        );

        let call_result = fetcher.can_transfer_call(token, owner, 1000.into()).await;
        assert!(call_result);
    }

    #[tokio::test]
    #[ignore]
    async fn mainnet_cannot_transfer() {
        let http = create_env_test_transport();
        let web3 = Web3::new(http);
        let settlement = contracts::GPv2Settlement::deployed(&web3).await.unwrap();
        let vault_relayer = settlement.vault_relayer().call().await.unwrap();
        let fetcher = Web3BalanceFetcher::new(web3, None, vault_relayer, settlement.address());
        let owner = H160(hex!("78045485dc4ad96f60937dad4b01b118958761ae"));
        // Token takes a fee.
        let token = H160(hex!("bae5f2d8a1299e5c4963eaff3312399253f27ccb"));

        fetcher.register(owner, token, SellTokenSource::Erc20).await;
        assert!(
            fetcher
                .get_balance(owner, token, SellTokenSource::Erc20)
                .unwrap()
                >= U256::from(1000)
        );

        let call_result = fetcher.can_transfer_call(token, owner, 1000.into()).await;
        // The non trace method is less accurate and thinks the transfer is ok even though it isn't.
        assert!(call_result);
    }

    #[tokio::test]
    #[ignore]
    async fn watch_testnet_erc20_balance() {
        let http = create_env_test_transport();
        let web3 = Web3::new(http);

        let accounts: Vec<H160> = web3.eth().accounts().await.expect("get accounts failed");
        let trader = Account::Local(accounts[0], None);

        let allowance_target = Account::Local(accounts[1], None);

        let token = ERC20Mintable::builder(&web3)
            .deploy()
            .await
            .expect("MintableERC20 deployment failed");

        let fetcher = Web3BalanceFetcher::new(
            web3,
            None,
            allowance_target.address(),
            H160::from_low_u64_be(1),
        );

        // Not available until registered
        assert_eq!(
            fetcher.get_balance(trader.address(), token.address(), SellTokenSource::Erc20),
            None,
        );

        fetcher
            .register(trader.address(), token.address(), SellTokenSource::Erc20)
            .await;

        assert_eq!(
            fetcher.get_balance(trader.address(), token.address(), SellTokenSource::Erc20),
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
            fetcher.get_balance(trader.address(), token.address(), SellTokenSource::Erc20),
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
            fetcher.get_balance(trader.address(), token.address(), SellTokenSource::Erc20),
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
            fetcher.get_balance(trader.address(), token.address(), SellTokenSource::Erc20),
            Some(U256::zero()),
        );
    }

    #[tokio::test]
    #[ignore]
    async fn can_transfer_testnet_vault_external_balance() {
        let http = create_env_test_transport();
        let web3 = Web3::new(http);

        let accounts: Vec<H160> = web3.eth().accounts().await.expect("get accounts failed");
        let admin = Account::Local(accounts[0], None);
        let trader = Account::Local(accounts[1], None);
        let allowance_target = Account::Local(accounts[2], None);

        let authorizer = BalancerV2Authorizer::builder(&web3, admin.address())
            .deploy()
            .await
            .expect("BalancerV2Authorizer deployment failed");
        let vault = BalancerV2Vault::builder(
            &web3,
            authorizer.address(),
            H160([0xef; 20]), // WETH address - not important for this test.
            0.into(),
            0.into(),
        )
        .deploy()
        .await
        .expect("BalancerV2Vault deployment failed");

        let token = ERC20Mintable::builder(&web3)
            .deploy()
            .await
            .expect("MintableERC20 deployment failed");

        let fetcher = Web3BalanceFetcher::new(
            web3,
            Some(vault.clone()),
            allowance_target.address(),
            H160::from_low_u64_be(1),
        );

        assert!(!fetcher
            .can_transfer(
                token.address(),
                trader.address(),
                100.into(),
                SellTokenSource::External
            )
            .await
            .unwrap());

        // Set authorization for allowance target to act as a Vault relayer
        vault::grant_required_roles(authorizer, vault.address(), allowance_target.address())
            .await
            .unwrap();
        // Give the trader some balance
        token
            .mint(trader.address(), 100.into())
            .send()
            .await
            .unwrap();
        // Approve the Vault
        token
            .approve(vault.address(), 200.into())
            .from(trader.clone())
            .send()
            .await
            .unwrap();
        // Set relayer approval for the allowance target
        vault
            .set_relayer_approval(trader.address(), allowance_target.address(), true)
            .from(trader.clone())
            .send()
            .await
            .unwrap();

        assert!(fetcher
            .can_transfer(
                token.address(),
                trader.address(),
                100.into(),
                SellTokenSource::External
            )
            .await
            .unwrap());
        assert!(!fetcher
            .can_transfer(
                token.address(),
                trader.address(),
                1_000_000.into(),
                SellTokenSource::External
            )
            .await
            .unwrap());
    }

    #[tokio::test]
    #[ignore]
    async fn watch_testnet_vault_external_balance() {
        let http = create_env_test_transport();
        let web3 = Web3::new(http);

        let accounts: Vec<H160> = web3.eth().accounts().await.expect("get accounts failed");
        let admin = Account::Local(accounts[0], None);
        let trader = Account::Local(accounts[1], None);
        let allowance_target = Account::Local(accounts[2], None);

        let authorizer = BalancerV2Authorizer::builder(&web3, admin.address())
            .deploy()
            .await
            .expect("BalancerV2Authorizer deployment failed");
        let vault = BalancerV2Vault::builder(
            &web3,
            authorizer.address(),
            H160([0xef; 20]), // WETH address - not important for this test.
            0.into(),
            0.into(),
        )
        .deploy()
        .await
        .expect("BalancerV2Vault deployment failed");

        let token = ERC20Mintable::builder(&web3)
            .deploy()
            .await
            .expect("MintableERC20 deployment failed");

        let fetcher = Web3BalanceFetcher::new(
            web3,
            Some(vault.clone()),
            allowance_target.address(),
            H160::from_low_u64_be(1),
        );

        // Not available until registered
        assert_eq!(
            fetcher.get_balance(trader.address(), token.address(), SellTokenSource::External),
            None,
        );

        fetcher
            .register(trader.address(), token.address(), SellTokenSource::External)
            .await;

        assert_eq!(
            fetcher.get_balance(trader.address(), token.address(), SellTokenSource::External),
            Some(U256::zero()),
        );

        // Balance without allowance and approval should not affect available balance
        token
            .mint(trader.address(), 100.into())
            .send()
            .await
            .unwrap();
        fetcher.update().await;
        assert_eq!(
            fetcher.get_balance(trader.address(), token.address(), SellTokenSource::External),
            Some(U256::zero()),
        );

        // Balance without approval should not affect available balance
        token
            .approve(vault.address(), 200.into())
            .from(trader.clone())
            .send()
            .await
            .unwrap();
        fetcher.update().await;
        assert_eq!(
            fetcher.get_balance(trader.address(), token.address(), SellTokenSource::External),
            Some(U256::zero()),
        );

        // Approving allowance_target as a relayer increase available balance
        vault
            .set_relayer_approval(trader.address(), allowance_target.address(), true)
            .from(trader.clone())
            .send()
            .await
            .unwrap();
        fetcher.update().await;
        assert_eq!(
            fetcher.get_balance(trader.address(), token.address(), SellTokenSource::External),
            Some(100.into()),
        );

        // Spending balance should decrease available balance
        token
            .transfer(allowance_target.address(), 50.into())
            .from(trader.clone())
            .send()
            .await
            .unwrap();
        fetcher.update().await;
        assert_eq!(
            fetcher.get_balance(trader.address(), token.address(), SellTokenSource::External),
            Some(50.into()),
        );
    }
}
