use {
    super::{BalanceFetching, Query, TransferSimulationError},
    crate::ethrpc::{Web3, Web3Transport},
    anyhow::{anyhow, Context, Result},
    contracts::{BalancerV2Vault, ERC20},
    ethcontract::{batch::CallBatch, Account},
    futures::{FutureExt, StreamExt},
    model::order::SellTokenSource,
    primitive_types::{H160, U256},
    std::future::Future,
    web3::types::{BlockId, BlockNumber, CallRequest},
};

pub struct Web3BalanceFetcher {
    web3: Web3,
    vault: Option<BalancerV2Vault>,
    vault_relayer: H160,
    settlement_contract: H160,
}

impl Web3BalanceFetcher {
    pub fn new(
        web3: Web3,
        vault: Option<H160>,
        vault_relayer: H160,
        settlement_contract: H160,
    ) -> Self {
        let vault = vault.map(|address| contracts::BalancerV2Vault::at(&web3, address));
        Self {
            web3,
            vault,
            vault_relayer,
            settlement_contract,
        }
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

struct Balance {
    balance: U256,
    allowance: U256,
}

impl Balance {
    fn zero() -> Self {
        Self {
            balance: 0.into(),
            allowance: 0.into(),
        }
    }

    fn effective_balance(&self) -> U256 {
        self.balance.min(self.allowance)
    }
}

fn erc20_balance_query(
    batch: &mut CallBatch<Web3Transport>,
    token: ERC20,
    owner: H160,
    spender: H160,
) -> impl Future<Output = Result<Balance>> {
    let balance = token.balance_of(owner).batch_call(batch);
    let allowance = token.allowance(owner, spender).batch_call(batch);
    async move {
        let balance = balance.await.context("balance")?;
        let allowance = allowance.await.context("allowance")?;
        Ok(Balance { balance, allowance })
    }
}

fn vault_external_balance_query(
    batch: &mut CallBatch<Web3Transport>,
    vault: BalancerV2Vault,
    token: ERC20,
    owner: H160,
    relayer: H160,
) -> impl Future<Output = Result<Balance>> {
    let balance = erc20_balance_query(batch, token, owner, vault.address());
    let approval = vault.has_approved_relayer(owner, relayer).batch_call(batch);
    async move {
        Ok(match approval.await.context("allowance")? {
            true => balance.await.context("balance")?,
            false => Balance::zero(),
        })
    }
}

#[async_trait::async_trait]
impl BalanceFetching for Web3BalanceFetcher {
    async fn get_balances(&self, queries: &[Query]) -> Vec<Result<U256>> {
        let mut batch = CallBatch::new(self.web3.transport().clone());
        let futures = queries
            .iter()
            .map(|query| {
                if !query.interactions.is_empty() {
                    tracing::warn!(
                        ?query,
                        "fetching balances for orders with interactions is not fully supported"
                    );
                }

                let token = ERC20::at(&self.web3, query.token);
                match (query.source, &self.vault) {
                    (SellTokenSource::Erc20, _) => {
                        erc20_balance_query(&mut batch, token, query.owner, self.vault_relayer)
                            .boxed()
                    }
                    (SellTokenSource::External, Some(vault)) => vault_external_balance_query(
                        &mut batch,
                        vault.clone(),
                        token,
                        query.owner,
                        self.vault_relayer,
                    )
                    .boxed(),
                    (SellTokenSource::External, None) => {
                        async { Err(anyhow!("external balance but no vault")) }.boxed()
                    }
                    (SellTokenSource::Internal, _) => {
                        async { Err(anyhow!("internal balances are not supported")) }.boxed()
                    }
                }
            })
            .collect::<Vec<_>>();
        batch.execute_all(usize::MAX).await;
        futures::stream::iter(futures)
            .then(|future| async {
                let balance = future.await?;
                Ok(balance.effective_balance())
            })
            .collect()
            .await
    }

    async fn can_transfer(
        &self,
        query: &Query,
        amount: U256,
    ) -> Result<(), TransferSimulationError> {
        if !query.interactions.is_empty() {
            tracing::warn!(
                ?query,
                "fetching balances for orders with interactions is not fully supported"
            );
        }

        match (query.source, &self.vault) {
            (SellTokenSource::Erc20, _) => {
                // In the very likely case that we can transfer we only do one RPC call.
                // Only do more calls in case we need to closer assess why the transfer is
                // failing
                if self
                    .can_transfer_call(query.token, query.owner, amount)
                    .await
                {
                    return Ok(());
                }
                let mut batch = CallBatch::new(self.web3.transport().clone());
                let token = ERC20::at(&self.web3, query.token);
                let balance_future =
                    erc20_balance_query(&mut batch, token, query.owner, self.vault_relayer);
                // Batch needs to execute before we can await the query result
                batch.execute_all(usize::MAX).await;
                let Balance { balance, allowance } = balance_future.await?;
                if balance < amount {
                    return Err(TransferSimulationError::InsufficientBalance);
                }
                if allowance < amount {
                    return Err(TransferSimulationError::InsufficientAllowance);
                }
                return Err(TransferSimulationError::TransferFailed);
            }
            (SellTokenSource::External, Some(vault)) => {
                if self
                    .can_manage_user_balance_call(query.token, query.owner, amount)
                    .await
                {
                    return Ok(());
                }
                let mut batch = CallBatch::new(self.web3.transport().clone());
                let token = ERC20::at(&self.web3, query.token);
                let balance_future =
                    erc20_balance_query(&mut batch, token, query.owner, vault.address());
                // Batch needs to execute before we can await the query result
                batch.execute_all(usize::MAX).await;
                let Balance { balance, allowance } = balance_future.await?;
                if balance < amount {
                    return Err(TransferSimulationError::InsufficientBalance);
                }
                if allowance < amount {
                    return Err(TransferSimulationError::InsufficientAllowance);
                }
                return Err(TransferSimulationError::TransferFailed);
            }
            (SellTokenSource::External, None) => {
                return Err(TransferSimulationError::Other(anyhow!(
                    "External Vault balances require a deployed vault"
                )))
            }
            (SellTokenSource::Internal, _) => {
                return Err(TransferSimulationError::Other(anyhow!(
                    "internal Vault balances not supported"
                )))
            }
        };
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
    use {
        super::*,
        crate::ethrpc::create_env_test_transport,
        contracts::{vault, BalancerV2Authorizer, ERC20Mintable},
        hex_literal::hex,
    };

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

        let result = fetcher
            .get_balances(&[Query {
                owner,
                token,
                source: SellTokenSource::Erc20,
                interactions: vec![],
            }])
            .await
            .into_iter()
            .next()
            .unwrap()
            .unwrap();
        assert!(result >= U256::from(1000));

        let call_result = fetcher.can_transfer_call(token, owner, 1000.into()).await;
        assert!(call_result);
    }

    #[tokio::test]
    #[ignore]
    async fn mainnet_cannot_transfer() {
        // TODO: For this test to work we need to find a new address that has approved
        // the contract for a token that takes a fee on transfer and still has
        // balance nio that token.

        let http = create_env_test_transport();
        let web3 = Web3::new(http);
        let settlement = contracts::GPv2Settlement::deployed(&web3).await.unwrap();
        let vault_relayer = settlement.vault_relayer().call().await.unwrap();
        let fetcher = Web3BalanceFetcher::new(web3, None, vault_relayer, settlement.address());
        let owner = H160(hex!("401c51ebe418d2809921565e606b60851bace4ec"));
        // Token takes a fee.
        let token = H160(hex!("bae5f2d8a1299e5c4963eaff3312399253f27ccb"));

        let result = fetcher
            .get_balances(&[Query {
                owner,
                token,
                source: SellTokenSource::Erc20,
                interactions: vec![],
            }])
            .await
            .into_iter()
            .next()
            .unwrap()
            .unwrap();
        println!("{result}");
        assert!(result >= U256::from(811));

        let call_result = fetcher.can_transfer_call(token, owner, 811.into()).await;
        // The non trace method is less accurate and thinks the transfer is ok even
        // though it isn't.
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

        let get_balance = || async {
            fetcher
                .get_balances(&[Query {
                    owner: trader.address(),
                    token: token.address(),
                    source: SellTokenSource::Erc20,
                    interactions: vec![],
                }])
                .await
                .into_iter()
                .next()
                .unwrap()
                .unwrap()
        };

        assert_eq!(get_balance().await, U256::zero());

        // Balance without approval should not affect available balance
        token
            .mint(trader.address(), 100.into())
            .send()
            .await
            .unwrap();
        assert_eq!(get_balance().await, U256::zero());

        // Approving allowance_target should increase available balance
        token
            .approve(allowance_target.address(), 200.into())
            .send()
            .await
            .unwrap();
        assert_eq!(get_balance().await, 100.into());

        // Spending balance should decrease available balance
        token
            .transfer(allowance_target.address(), 100.into())
            .send()
            .await
            .unwrap();
        assert_eq!(get_balance().await, U256::zero());
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
            Some(vault.address()),
            allowance_target.address(),
            H160::from_low_u64_be(1),
        );

        assert!(matches!(
            fetcher
                .can_transfer(
                    &Query {
                        token: token.address(),
                        owner: trader.address(),
                        source: SellTokenSource::External,
                        interactions: vec![],
                    },
                    100.into(),
                )
                .await,
            Err(TransferSimulationError::InsufficientBalance)
        ));

        // Set authorization for allowance target to act as a Vault relayer
        vault::grant_required_roles(&authorizer, vault.address(), allowance_target.address())
            .await
            .unwrap();
        // Give the trader some balance
        token
            .mint(trader.address(), 1_000_000.into())
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

        assert!(matches!(
            fetcher
                .can_transfer(
                    &Query {
                        token: token.address(),
                        owner: trader.address(),
                        source: SellTokenSource::External,
                        interactions: vec![],
                    },
                    100.into(),
                )
                .await,
            Ok(_),
        ));
        assert!(matches!(
            fetcher
                .can_transfer(
                    &Query {
                        token: token.address(),
                        owner: trader.address(),
                        source: SellTokenSource::External,
                        interactions: vec![],
                    },
                    1_000_000.into(),
                )
                .await,
            Err(TransferSimulationError::InsufficientAllowance)
        ));
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
            Some(vault.address()),
            allowance_target.address(),
            H160::from_low_u64_be(1),
        );

        let get_balance = || async {
            fetcher
                .get_balances(&[Query {
                    owner: trader.address(),
                    token: token.address(),
                    source: SellTokenSource::External,
                    interactions: vec![],
                }])
                .await
                .into_iter()
                .next()
                .unwrap()
                .unwrap()
        };

        assert_eq!(get_balance().await, U256::zero());

        // Balance without allowance and approval should not affect available balance
        token
            .mint(trader.address(), 100.into())
            .send()
            .await
            .unwrap();
        assert_eq!(get_balance().await, U256::zero());

        // Balance without approval should not affect available balance
        token
            .approve(vault.address(), 50.into())
            .from(trader.clone())
            .send()
            .await
            .unwrap();
        assert_eq!(get_balance().await, U256::zero());

        // Approving allowance_target as a relayer increase available balance
        vault
            .set_relayer_approval(trader.address(), allowance_target.address(), true)
            .from(trader.clone())
            .send()
            .await
            .unwrap();
        assert_eq!(get_balance().await, 50.into());

        // Spending balance should decrease available balance
        token
            .transfer(allowance_target.address(), 50.into())
            .from(trader.clone())
            .send()
            .await
            .unwrap();
        assert_eq!(get_balance().await, 50.into());
    }
}
