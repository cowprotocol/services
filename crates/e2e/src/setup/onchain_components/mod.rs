use {
    crate::{
        nodes::forked_node::ForkedNodeApi,
        setup::{DeployedContracts, deploy::Contracts},
    },
    ::alloy::{
        network::{Ethereum, NetworkWallet, TransactionBuilder},
        primitives::Address,
        providers::{Provider, ext::AnvilApi},
        rpc::types::TransactionRequest,
        signers::local::PrivateKeySigner,
    },
    app_data::Hook,
    contracts::alloy::{
        ERC20Mintable,
        GPv2AllowListAuthentication::GPv2AllowListAuthentication,
        test::CowProtocolToken,
    },
    ethcontract::{Account, H160, PrivateKey, U256},
    ethrpc::alloy::{
        CallBuilderExt,
        ProviderSignerExt,
        conversions::{IntoAlloy, IntoLegacy},
    },
    hex_literal::hex,
    model::{
        DomainSeparator,
        signature::{EcdsaSignature, EcdsaSigningScheme},
    },
    number::units::EthUnit,
    shared::ethrpc::Web3,
    std::{borrow::BorrowMut, ops::Deref},
    web3::signing::{self},
};

pub mod alloy;
pub mod safe;

#[derive(Clone, Debug)]
pub struct TestAccount {
    account: Account,
    private_key: [u8; 32],
}

impl TestAccount {
    pub fn account(&self) -> &Account {
        &self.account
    }

    pub fn address(&self) -> Address {
        self.account.address().into_alloy()
    }

    pub fn private_key(&self) -> &[u8; 32] {
        &self.private_key
    }

    pub fn sign_typed_data(
        &self,
        domain_separator: &DomainSeparator,
        struct_hash: &[u8; 32],
    ) -> EcdsaSignature {
        EcdsaSignature::sign(
            EcdsaSigningScheme::Eip712,
            domain_separator,
            struct_hash,
            &PrivateKeySigner::from_slice(self.private_key()).unwrap(),
        )
    }

    pub async fn nonce(&self, web3: &Web3) -> u64 {
        web3.alloy
            .get_transaction_count(self.address())
            .await
            .unwrap()
    }
}

struct AccountGenerator {
    id: usize,
}

impl Default for AccountGenerator {
    fn default() -> Self {
        // Start from a high number to avoid conflicts with existing accounts which may
        // have clowny delegation contracts deployed (e.g. preventing to send ETH to
        // that address)
        AccountGenerator { id: 100500 }
    }
}

impl Iterator for AccountGenerator {
    type Item = TestAccount;

    fn next(&mut self) -> Option<Self::Item> {
        let mut buffer = [0; 32];

        loop {
            self.id = self.id.checked_add(1)?;

            buffer[24..].copy_from_slice(&self.id.to_be_bytes());
            let Ok(pk) = PrivateKey::from_raw(buffer) else {
                continue;
            };

            break Some(TestAccount {
                account: Account::Offline(pk, None),
                private_key: buffer,
            });
        }
    }
}

#[derive(Debug)]
pub struct MintableToken {
    contract: ERC20Mintable::Instance,
    minter: Account,
}

impl MintableToken {
    pub async fn mint(&self, to: Address, amount: ::alloy::primitives::U256) {
        self.contract
            .mint(to, amount)
            .from(self.minter.address().into_alloy())
            .send_and_watch()
            .await
            .unwrap();
    }
}

impl Deref for MintableToken {
    type Target = ERC20Mintable::Instance;

    fn deref(&self) -> &Self::Target {
        &self.contract
    }
}

#[derive(Debug)]
pub struct CowToken {
    contract: CowProtocolToken::Instance,
    holder: Account,
}

impl CowToken {
    pub async fn fund(&self, to: Address, amount: ::alloy::primitives::U256) {
        self.contract
            .transfer(to, amount)
            .from(self.holder.address().into_alloy())
            .send_and_watch()
            .await
            .unwrap();
    }

    pub async fn permit(&self, owner: &TestAccount, spender: H160, value: U256) -> Hook {
        let domain = self.contract.DOMAIN_SEPARATOR().call().await.unwrap();
        let nonce = self
            .contract
            .nonces(owner.address())
            .call()
            .await
            .unwrap()
            .into_legacy();
        let deadline = U256::max_value();

        let struct_hash = {
            let mut buffer = [0_u8; 192];
            buffer[0..32].copy_from_slice(&hex!(
                "6e71edae12b1b97f4d1f60370fef10105fa2faae0126114a169c64845d6126c9"
            ));
            buffer[44..64].copy_from_slice(owner.address().as_slice());
            buffer[76..96].copy_from_slice(spender.as_bytes());
            value.to_big_endian(&mut buffer[96..128]);
            nonce.to_big_endian(&mut buffer[128..160]);
            deadline.to_big_endian(&mut buffer[160..192]);

            signing::keccak256(&buffer)
        };

        let signature = owner.sign_typed_data(&DomainSeparator(domain.0), &struct_hash);

        let permit = self.contract.permit(
            owner.address(),
            spender.into_alloy(),
            value.into_alloy(),
            deadline.into_alloy(),
            signature.v,
            signature.r.0.into(),
            signature.s.0.into(),
        );

        Hook {
            target: *self.contract.address(),
            call_data: permit.calldata().to_vec(),
            gas_limit: permit.estimate_gas().await.unwrap(),
        }
    }
}

impl Deref for CowToken {
    type Target = CowProtocolToken::Instance;

    fn deref(&self) -> &Self::Target {
        &self.contract
    }
}

/// Wrapper over deployed [Contracts].
/// Exposes various utility methods for tests.
/// Deterministically generates unique accounts.
pub struct OnchainComponents {
    web3: Web3,
    contracts: Contracts,
    accounts: AccountGenerator,
}

impl OnchainComponents {
    pub async fn deploy(web3: Web3) -> Self {
        let contracts = Contracts::deploy(&web3).await;

        Self {
            web3,
            contracts,
            accounts: Default::default(),
        }
    }

    pub async fn deployed(web3: Web3) -> Self {
        let contracts = Contracts::deployed_with(&web3, DeployedContracts::default()).await;

        Self {
            web3,
            contracts,
            accounts: Default::default(),
        }
    }

    pub async fn deployed_with(web3: Web3, deployed: DeployedContracts) -> Self {
        let contracts = Contracts::deployed_with(&web3, deployed).await;

        Self {
            web3,
            contracts,
            accounts: Default::default(),
        }
    }

    /// Generate next `N` accounts with the given initial balance.
    pub async fn make_accounts<const N: usize>(
        &mut self,
        with_wei: ::alloy::primitives::U256,
    ) -> [TestAccount; N] {
        let res = self.accounts.borrow_mut().take(N).collect::<Vec<_>>();
        assert_eq!(res.len(), N);

        for account in &res {
            let signer = PrivateKeySigner::from_slice(account.private_key()).unwrap();
            self.web3.wallet.register_signer(signer);

            self.send_wei(account.address(), with_wei).await;
        }

        res.try_into().unwrap()
    }

    /// Generate next `N` accounts with the given initial balance and
    /// authenticate them as solvers.
    pub async fn make_solvers<const N: usize>(
        &mut self,
        with_wei: ::alloy::primitives::U256,
    ) -> [TestAccount; N] {
        let solvers = self.make_accounts::<N>(with_wei).await;

        for solver in &solvers {
            self.web3
                .wallet
                .register_signer(PrivateKeySigner::from_slice(solver.private_key()).unwrap());

            self.contracts
                .gp_authenticator
                .addSolver(solver.address())
                .send_and_watch()
                .await
                .expect("failed to add solver");
        }

        solvers
    }

    pub async fn set_solver_allowed(&self, solver: Address, allowed: bool) {
        if allowed {
            self.contracts
                .gp_authenticator
                .addSolver(solver)
                .send_and_watch()
                .await
                .expect("failed to add solver");
        } else {
            self.contracts
                .gp_authenticator
                .removeSolver(solver)
                .send_and_watch()
                .await
                .expect("failed to remove solver");
        }
    }

    /// Generate next `N` accounts with the given initial balance and
    /// authenticate them as solvers on a forked network.
    pub async fn make_solvers_forked<const N: usize>(
        &mut self,
        with_wei: ::alloy::primitives::U256,
    ) -> [TestAccount; N] {
        let authenticator = &self.contracts.gp_authenticator;

        let auth_manager = authenticator.manager().call().await.unwrap().into_legacy();

        let forked_node_api = self.web3.api::<ForkedNodeApi<_>>();

        forked_node_api
            .set_balance(&auth_manager, 100u64.eth().into_legacy())
            .await
            .expect("could not set auth_manager balance");

        let impersonated_authenticator = {
            forked_node_api
                .impersonate(&auth_manager)
                .await
                .expect("could not impersonate auth_manager");

            // we create a new provider without a wallet so that
            // alloy does not try to sign the tx with it and instead
            // forwards the tx to the node for signing. This will
            // work because we told anvil to impersonate that address.
            let provider = authenticator.provider().clone().without_wallet();
            GPv2AllowListAuthentication::new(*authenticator.address(), provider)
        };

        let solvers = self.make_accounts::<N>(with_wei).await;

        for solver in &solvers {
            impersonated_authenticator
                .addSolver(solver.address())
                .from(auth_manager.into_alloy())
                .send_and_watch()
                .await
                .expect("failed to add solver");
        }

        if let Some(router) = &self.contracts.flashloan_router {
            impersonated_authenticator
                .addSolver(*router.address())
                .from(auth_manager.into_alloy())
                .send_and_watch()
                .await
                .expect("failed to add flashloan wrapper");
        }

        solvers
    }

    /// Deploy `N` tokens without any onchain liquidity
    pub async fn deploy_tokens<const N: usize>(&self, minter: &Account) -> [MintableToken; N] {
        let mut res = Vec::with_capacity(N);

        for _ in 0..N {
            let contract_address = ERC20Mintable::Instance::deploy_builder(self.web3.alloy.clone())
                // We can't escape the .from here because we need to ensure Minter permissions later on
                .from(minter.address().into_alloy())
                .deploy()
                .await
                .expect("ERC20Mintable deployment failed");
            let contract = ERC20Mintable::Instance::new(contract_address, self.web3.alloy.clone());

            res.push(MintableToken {
                contract,
                minter: minter.clone(),
            });
        }

        res.try_into().unwrap()
    }

    /// Deploy `N` tokens with WETH Uniswap pools.
    pub async fn deploy_tokens_with_weth_uni_v2_pools<const N: usize>(
        &self,
        token_amount: U256,
        weth_amount: U256,
    ) -> [MintableToken; N] {
        let minter = Account::Local(
            self.web3
                .alloy
                .get_accounts()
                .await
                .expect("getting accounts failed")[0]
                .into_legacy(),
            None,
        );
        let tokens = self.deploy_tokens::<N>(&minter).await;
        self.seed_weth_uni_v2_pools(tokens.iter(), token_amount, weth_amount)
            .await;
        tokens
    }

    pub async fn seed_weth_uni_v2_pools(
        &self,
        tokens: impl IntoIterator<Item = &MintableToken>,
        token_amount: U256,
        weth_amount: U256,
    ) {
        for MintableToken { contract, minter } in tokens {
            contract
                .mint(minter.address().into_alloy(), token_amount.into_alloy())
                .from(minter.address().into_alloy())
                .send_and_watch()
                .await
                .unwrap();

            self.contracts
                .weth
                .deposit()
                .value(weth_amount.into_alloy())
                .from(minter.address().into_alloy())
                .send_and_watch()
                .await
                .unwrap();

            self.contracts
                .uniswap_v2_factory
                .createPair(*contract.address(), *self.contracts.weth.address())
                .from(minter.address().into_alloy())
                .send_and_watch()
                .await
                .unwrap();

            contract
                .approve(
                    *self.contracts.uniswap_v2_router.address(),
                    token_amount.into_alloy(),
                )
                .from(minter.address().into_alloy())
                .send_and_watch()
                .await
                .unwrap();

            self.contracts
                .weth
                .approve(
                    *self.contracts.uniswap_v2_router.address(),
                    weth_amount.into_alloy(),
                )
                .from(minter.address().into_alloy())
                .send_and_watch()
                .await
                .unwrap();

            self.contracts
                .uniswap_v2_router
                .addLiquidity(
                    *contract.address(),
                    *self.contracts.weth.address(),
                    token_amount.into_alloy(),
                    weth_amount.into_alloy(),
                    ::alloy::primitives::U256::ZERO,
                    ::alloy::primitives::U256::ZERO,
                    minter.address().into_alloy(),
                    ::alloy::primitives::U256::MAX,
                )
                .from(minter.address().into_alloy())
                .send_and_watch()
                .await
                .unwrap();
        }
    }

    pub async fn seed_uni_v2_pool(
        &self,
        asset_a: (&MintableToken, ::alloy::primitives::U256),
        asset_b: (&MintableToken, ::alloy::primitives::U256),
    ) {
        let lp = &asset_a.0.minter;
        asset_a.0.mint(lp.address().into_alloy(), asset_a.1).await;
        asset_b.0.mint(lp.address().into_alloy(), asset_b.1).await;

        self.contracts
            .uniswap_v2_factory
            .createPair(*asset_a.0.address(), *asset_b.0.address())
            .from(lp.address().into_alloy())
            .send_and_watch()
            .await
            .unwrap();

        asset_a
            .0
            .approve(*self.contracts.uniswap_v2_router.address(), asset_a.1)
            .from(lp.address().into_alloy())
            .send_and_watch()
            .await
            .unwrap();

        asset_b
            .0
            .approve(*self.contracts.uniswap_v2_router.address(), asset_b.1)
            .from(lp.address().into_alloy())
            .send_and_watch()
            .await
            .unwrap();
        self.contracts
            .uniswap_v2_router
            .addLiquidity(
                *asset_a.0.address(),
                *asset_b.0.address(),
                asset_a.1,
                asset_b.1,
                ::alloy::primitives::U256::ZERO,
                ::alloy::primitives::U256::ZERO,
                lp.address().into_alloy(),
                ::alloy::primitives::U256::MAX,
            )
            .from(lp.address().into_alloy())
            .send_and_watch()
            .await
            .unwrap();
    }

    /// Mints `amount` tokens to its `token`-WETH Uniswap V2 pool.
    ///
    /// This can be used to modify the pool reserves during a test.
    pub async fn mint_token_to_weth_uni_v2_pool(
        &self,
        token: &MintableToken,
        amount: ::alloy::primitives::U256,
    ) {
        let pair = contracts::alloy::IUniswapLikePair::Instance::new(
            self.contracts
                .uniswap_v2_factory
                .getPair(*self.contracts.weth.address(), *token.address())
                .call()
                .await
                .expect("failed to get Uniswap V2 pair"),
            self.web3.alloy.clone(),
        );
        assert!(!pair.address().is_zero(), "Uniswap V2 pair is not deployed");

        // Mint amount + 1 to the pool, and then swap out 1 of the minted token
        // in order to force it to update its K-value.
        token
            .mint(*pair.address(), amount + ::alloy::primitives::U256::ONE)
            .await;
        let (out0, out1) = if self.contracts.weth.address() < token.address() {
            (1, 0)
        } else {
            (0, 1)
        };
        pair.swap(
            ::alloy::primitives::U256::from(out0),
            ::alloy::primitives::U256::from(out1),
            token.minter.address().into_alloy(),
            Default::default(),
        )
        .from(token.minter.address().into_alloy())
        .send_and_watch()
        .await
        .expect("Uniswap V2 pair couldn't mint");
    }

    pub async fn deploy_cow_token(&self, supply: U256) -> CowToken {
        let holder = NetworkWallet::<Ethereum>::default_signer_address(&self.web3().wallet);
        let holder = Account::Local(holder.into_legacy(), None);
        let contract = CowProtocolToken::CowProtocolToken::deploy(
            self.web3.alloy.clone(),
            holder.address().into_alloy(),
            holder.address().into_alloy(),
            supply.into_alloy(),
        )
        .await
        .expect("CowProtocolToken deployment failed");
        CowToken { contract, holder }
    }

    pub async fn deploy_cow_weth_pool(
        &self,
        cow_supply: U256,
        cow_amount: U256,
        weth_amount: U256,
    ) -> CowToken {
        let cow = self.deploy_cow_token(cow_supply).await;

        self.contracts
            .weth
            .deposit()
            .value(weth_amount.into_alloy())
            .from(cow.holder.address().into_alloy())
            .send_and_watch()
            .await
            .unwrap();

        self.contracts
            .uniswap_v2_factory
            .createPair(*cow.address(), *self.contracts.weth.address())
            .from(cow.holder.address().into_alloy())
            .send_and_watch()
            .await
            .unwrap();
        cow.approve(
            *self.contracts.uniswap_v2_router.address(),
            cow_amount.into_alloy(),
        )
        .from(cow.holder.address().into_alloy())
        .send_and_watch()
        .await
        .unwrap();
        self.contracts
            .weth
            .approve(
                *self.contracts.uniswap_v2_router.address(),
                weth_amount.into_alloy(),
            )
            .from(cow.holder.address().into_alloy())
            .send_and_watch()
            .await
            .unwrap();
        self.contracts
            .uniswap_v2_router
            .addLiquidity(
                *cow.address(),
                *self.contracts.weth.address(),
                cow_amount.into_alloy(),
                weth_amount.into_alloy(),
                ::alloy::primitives::U256::ZERO,
                ::alloy::primitives::U256::ZERO,
                cow.holder.address().into_alloy(),
                ::alloy::primitives::U256::MAX,
            )
            .from(cow.holder.address().into_alloy())
            .send_and_watch()
            .await
            .unwrap();

        cow
    }

    pub async fn send_wei(&self, to: Address, amount: ::alloy::primitives::U256) {
        let balance_before = self.web3.alloy.get_balance(to).await.unwrap();
        self.web3
            .alloy
            .send_transaction(TransactionRequest::default().with_to(to).with_value(amount))
            .await
            .unwrap()
            .watch()
            .await
            .unwrap();

        // There seems to be a bug in anvil where sending ETH does not work
        // reliably with a forked node. On some block numbers the transaction
        // supposedly succeeds but the balances still don't get changed.
        // If you hit this assert try using a different block number for your
        // forked test.
        let balance_after = self.web3.alloy.get_balance(to).await.unwrap();
        assert_eq!(balance_after, balance_before + amount);
    }

    pub async fn mint_block(&self) {
        tracing::info!("mining block");
        self.web3.alloy.evm_mine(None).await.unwrap();
    }

    pub fn contracts(&self) -> &Contracts {
        &self.contracts
    }

    pub fn web3(&self) -> &Web3 {
        &self.web3
    }
}
