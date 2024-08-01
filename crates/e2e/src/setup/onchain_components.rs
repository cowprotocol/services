use {
    crate::{nodes::forked_node::ForkedNodeApi, setup::deploy::Contracts},
    app_data::Hook,
    contracts::{CowProtocolToken, ERC20Mintable},
    ethcontract::{transaction::TransactionBuilder, Account, Bytes, PrivateKey, H160, U256},
    hex_literal::hex,
    model::{
        signature::{EcdsaSignature, EcdsaSigningScheme},
        DomainSeparator,
        TokenPair,
    },
    secp256k1::SecretKey,
    shared::ethrpc::Web3,
    std::{borrow::BorrowMut, ops::Deref},
    web3::{signing, signing::SecretKeyRef, Transport},
};

pub mod safe;

#[macro_export]
macro_rules! tx_value {
    ($acc:expr, $value:expr, $call:expr) => {{
        const NAME: &str = stringify!($call);
        $call
            .from($acc.clone())
            .value($value)
            .send()
            .await
            .expect(&format!("{} failed", NAME))
    }};
}

#[macro_export]
macro_rules! tx {
    ($acc:expr, $call:expr) => {
        $crate::tx_value!($acc, U256::zero(), $call)
    };
}

pub fn to_wei_with_exp(base: u32, exp: usize) -> U256 {
    U256::from(base) * U256::exp10(exp)
}

pub fn to_wei(base: u32) -> U256 {
    to_wei_with_exp(base, 18)
}

pub async fn hook_for_transaction<T>(tx: TransactionBuilder<T>) -> Hook
where
    T: web3::Transport,
{
    let gas_limit = tx
        .clone()
        .estimate_gas()
        .await
        .expect("transaction reverted when estimating gas")
        .as_u64();
    Hook {
        target: tx.to.unwrap(),
        call_data: tx.data.unwrap().0,
        gas_limit,
    }
}

#[derive(Clone, Debug)]
pub struct TestAccount {
    account: Account,
    private_key: [u8; 32],
}

impl TestAccount {
    pub fn account(&self) -> &Account {
        &self.account
    }

    pub fn address(&self) -> H160 {
        self.account.address()
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
            SecretKeyRef::from(&SecretKey::from_slice(self.private_key()).unwrap()),
        )
    }

    pub async fn nonce(&self, web3: &Web3) -> U256 {
        web3.eth()
            .transaction_count(self.address(), None)
            .await
            .unwrap()
    }
}

#[derive(Default)]
struct AccountGenerator {
    id: usize,
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
    contract: ERC20Mintable,
    minter: Account,
}

impl MintableToken {
    pub async fn mint(&self, to: H160, amount: U256) {
        tx!(self.minter, self.contract.mint(to, amount));
    }
}

impl Deref for MintableToken {
    type Target = ERC20Mintable;

    fn deref(&self) -> &Self::Target {
        &self.contract
    }
}

#[derive(Debug)]
pub struct CowToken {
    contract: CowProtocolToken,
    holder: Account,
}

impl CowToken {
    pub async fn fund(&self, to: H160, amount: U256) {
        tx!(self.holder, self.contract.transfer(to, amount));
    }

    pub async fn permit(&self, owner: &TestAccount, spender: H160, value: U256) -> Hook {
        let domain = self.contract.domain_separator().call().await.unwrap();
        let nonce = self.contract.nonces(owner.address()).call().await.unwrap();
        let deadline = U256::max_value();

        let struct_hash = {
            let mut buffer = [0_u8; 192];
            buffer[0..32].copy_from_slice(&hex!(
                "6e71edae12b1b97f4d1f60370fef10105fa2faae0126114a169c64845d6126c9"
            ));
            buffer[44..64].copy_from_slice(owner.address().as_bytes());
            buffer[76..96].copy_from_slice(spender.as_bytes());
            value.to_big_endian(&mut buffer[96..128]);
            nonce.to_big_endian(&mut buffer[128..160]);
            deadline.to_big_endian(&mut buffer[160..192]);

            signing::keccak256(&buffer)
        };

        let signature = owner.sign_typed_data(&DomainSeparator(domain.0), &struct_hash);

        let permit = self.contract.permit(
            owner.address(),
            spender,
            value,
            deadline,
            signature.v,
            Bytes(signature.r.0),
            Bytes(signature.s.0),
        );

        hook_for_transaction(permit.tx).await
    }
}

impl Deref for CowToken {
    type Target = CowProtocolToken;

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
        let contracts = Contracts::deployed(&web3).await;

        Self {
            web3,
            contracts,
            accounts: Default::default(),
        }
    }

    /// Generate next `N` accounts with the given initial balance.
    pub async fn make_accounts<const N: usize>(&mut self, with_wei: U256) -> [TestAccount; N] {
        let res = self.accounts.borrow_mut().take(N).collect::<Vec<_>>();
        assert_eq!(res.len(), N);

        for account in &res {
            self.send_wei(account.address(), with_wei).await;
        }

        res.try_into().unwrap()
    }

    /// Generate next `N` accounts with the given initial balance and
    /// authenticate them as solvers.
    pub async fn make_solvers<const N: usize>(&mut self, with_wei: U256) -> [TestAccount; N] {
        let solvers = self.make_accounts::<N>(with_wei).await;

        for solver in &solvers {
            self.contracts
                .gp_authenticator
                .add_solver(solver.address())
                .send()
                .await
                .expect("failed to add solver");
        }

        solvers
    }

    /// Generate next `N` accounts with the given initial balance and
    /// authenticate them as solvers on a forked network.
    pub async fn make_solvers_forked<const N: usize>(
        &mut self,
        with_wei: U256,
    ) -> [TestAccount; N] {
        let auth_manager = self
            .contracts
            .gp_authenticator
            .manager()
            .call()
            .await
            .unwrap();

        let forked_node_api = self.web3.api::<ForkedNodeApi<_>>();

        forked_node_api
            .set_balance(&auth_manager, to_wei(100))
            .await
            .expect("could not set auth_manager balance");

        let auth_manager = forked_node_api
            .impersonate(&auth_manager)
            .await
            .expect("could not impersonate auth_manager");

        let solvers = self.make_accounts::<N>(with_wei).await;

        for solver in &solvers {
            self.contracts
                .gp_authenticator
                .add_solver(solver.address())
                .from(auth_manager.clone())
                .send()
                .await
                .expect("failed to add solver");
        }

        solvers
    }

    /// Deploy `N` tokens without any onchain liquidity
    pub async fn deploy_tokens<const N: usize>(&self, minter: &Account) -> [MintableToken; N] {
        let mut res = Vec::with_capacity(N);
        for _ in 0..N {
            let contract = ERC20Mintable::builder(&self.web3)
                .from(minter.clone())
                .deploy()
                .await
                .expect("MintableERC20 deployment failed");
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
                .eth()
                .accounts()
                .await
                .expect("getting accounts failed")[0],
            None,
        );
        let tokens = self.deploy_tokens::<N>(&minter).await;
        self.seed_weth_uni_v2_pools(tokens.iter(), token_amount, weth_amount)
            .await;
        tokens
    }

    pub async fn seed_weth_uni_v2_pools<'a, I: Iterator<Item = &'a MintableToken>>(
        &self,
        tokens: I,
        token_amount: U256,
        weth_amount: U256,
    ) {
        for MintableToken { contract, minter } in tokens {
            tx!(minter, contract.mint(minter.address(), token_amount));
            tx_value!(minter, weth_amount, self.contracts.weth.deposit());

            tx!(
                minter,
                self.contracts
                    .uniswap_v2_factory
                    .create_pair(contract.address(), self.contracts.weth.address())
            );
            tx!(
                minter,
                contract.approve(self.contracts.uniswap_v2_router.address(), token_amount)
            );
            tx!(
                minter,
                self.contracts
                    .weth
                    .approve(self.contracts.uniswap_v2_router.address(), weth_amount)
            );
            tx!(
                minter,
                self.contracts.uniswap_v2_router.add_liquidity(
                    contract.address(),
                    self.contracts.weth.address(),
                    token_amount,
                    weth_amount,
                    0_u64.into(),
                    0_u64.into(),
                    minter.address(),
                    U256::max_value(),
                )
            );
        }
    }

    /// Mints `amount` tokens to its `token`-WETH Uniswap V2 pool.
    ///
    /// This can be used to modify the pool reserves during a test.
    pub async fn mint_token_to_weth_uni_v2_pool(&self, token: &MintableToken, amount: U256) {
        let pair = contracts::IUniswapLikePair::at(
            &self.web3,
            self.contracts
                .uniswap_v2_factory
                .get_pair(self.contracts.weth.address(), token.address())
                .call()
                .await
                .expect("failed to get Uniswap V2 pair"),
        );
        assert!(!pair.address().is_zero(), "Uniswap V2 pair is not deployed");

        // Mint amount + 1 to the pool, and then swap out 1 of the minted token
        // in order to force it to update its K-value.
        token.mint(pair.address(), amount + 1).await;
        let (out0, out1) = if TokenPair::new(self.contracts.weth.address(), token.address())
            .unwrap()
            .get()
            .0
            == token.address()
        {
            (1, 0)
        } else {
            (0, 1)
        };
        pair.swap(
            out0.into(),
            out1.into(),
            token.minter.address(),
            Default::default(),
        )
        .from(token.minter.clone())
        .send()
        .await
        .expect("Uniswap V2 pair couldn't mint");
    }

    pub async fn deploy_cow_token(&self, holder: Account, supply: U256) -> CowToken {
        let contract =
            CowProtocolToken::builder(&self.web3, holder.address(), holder.address(), supply)
                .deploy()
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
        let holder = Account::Local(
            self.web3
                .eth()
                .accounts()
                .await
                .expect("getting accounts failed")[0],
            None,
        );
        let cow = self.deploy_cow_token(holder.clone(), cow_supply).await;

        tx_value!(holder, weth_amount, self.contracts.weth.deposit());

        tx!(
            holder,
            self.contracts
                .uniswap_v2_factory
                .create_pair(cow.address(), self.contracts.weth.address())
        );
        tx!(
            holder,
            cow.approve(self.contracts.uniswap_v2_router.address(), cow_amount)
        );
        tx!(
            holder,
            self.contracts
                .weth
                .approve(self.contracts.uniswap_v2_router.address(), weth_amount)
        );
        tx!(
            holder,
            self.contracts.uniswap_v2_router.add_liquidity(
                cow.address(),
                self.contracts.weth.address(),
                cow_amount,
                weth_amount,
                0_u64.into(),
                0_u64.into(),
                holder.address(),
                U256::max_value(),
            )
        );

        cow
    }

    pub async fn send_wei(&self, to: H160, amount: U256) {
        TransactionBuilder::new(self.web3.clone())
            .value(amount)
            .to(to)
            .send()
            .await
            .unwrap();
    }

    pub async fn mint_block(&self) {
        tracing::info!("mining block");
        self.web3
            .transport()
            .execute("evm_mine", vec![])
            .await
            .unwrap();
    }

    pub fn contracts(&self) -> &Contracts {
        &self.contracts
    }
}
