use {
    crate::setup::deploy::Contracts,
    contracts::{
        CowProtocolToken,
        ERC20Mintable,
        GnosisSafe,
        GnosisSafeCompatibilityFallbackHandler,
    },
    ethcontract::{transaction::TransactionBuilder, Account, Bytes, PrivateKey, H160, H256, U256},
    hex_literal::hex,
    model::{
        interaction::InteractionData,
        signature::{EcdsaSignature, EcdsaSigningScheme},
        DomainSeparator,
    },
    secp256k1::SecretKey,
    shared::{ethrpc::Web3, interaction},
    std::{borrow::BorrowMut, ops::Deref},
    web3::{
        signing,
        signing::{Key, SecretKeyRef},
    },
};

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

#[macro_export]
macro_rules! tx_safe {
    ($acc:expr, $safe:ident, $call:expr) => {{
        let call = $call;
        $crate::tx!(
            $acc,
            $safe.exec_transaction(
                call.tx.to.unwrap(),
                call.tx.value.unwrap_or_default(),
                ::ethcontract::Bytes(call.tx.data.unwrap_or_default().0),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                $crate::setup::gnosis_safe_prevalidated_signature($acc.address()),
            )
        );
    }};
}

/// Generate a Safe "pre-validated" signature.
///
/// This is a special "marker" signature that can be used if the account that
/// is executing the transaction is an owner. For single owner safes, this is
/// the easiest way to execute a transaction as it does not involve any ECDSA
/// signing.
///
/// See:
/// - Documentation: <https://docs.gnosis-safe.io/contracts/signatures#pre-validated-signatures>
/// - Code: <https://github.com/safe-global/safe-contracts/blob/c36bcab46578a442862d043e12a83fec41143dec/contracts/GnosisSafe.sol#L287-L291>
pub fn gnosis_safe_prevalidated_signature(owner: H160) -> Bytes<Vec<u8>> {
    let mut signature = vec![0; 65];
    signature[12..32].copy_from_slice(owner.as_bytes());
    signature[64] = 1;
    Bytes(signature)
}

/// Generate an owner signature for EIP-1271.
///
/// The Gnosis Safe uses off-chain ECDSA signatures from its owners as the
/// signature bytes when validating EIP-1271 signatures. Specifically, it
/// expects a signed EIP-712 `SafeMessage(bytes message)` (where `message` is
/// the 32-byte hash of the data being verified).
///
/// See:
/// - Code: <https://github.com/safe-global/safe-contracts/blob/c36bcab46578a442862d043e12a83fec41143dec/contracts/handler/CompatibilityFallbackHandler.sol#L66-L70>
pub async fn gnosis_safe_eip1271_signature(
    key: SecretKeyRef<'_>,
    safe: &GnosisSafe,
    message_hash: H256,
) -> Vec<u8> {
    let handler =
        GnosisSafeCompatibilityFallbackHandler::at(&safe.raw_instance().web3(), safe.address());

    let signing_hash = handler
        .get_message_hash(Bytes(message_hash.as_bytes().to_vec()))
        .call()
        .await
        .unwrap();

    let signature = key.sign(&signing_hash.0, None).unwrap();

    let mut bytes = vec![0u8; 65];
    bytes[0..32].copy_from_slice(signature.r.as_bytes());
    bytes[32..64].copy_from_slice(signature.s.as_bytes());
    bytes[64] = signature.v as _;

    bytes
}

pub fn to_wei(base: u32) -> U256 {
    U256::from(base) * U256::exp10(18)
}

#[derive(Debug)]
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

    pub async fn permit(&self, owner: &TestAccount, spender: H160, value: U256) -> InteractionData {
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

        let signature = EcdsaSignature::sign(
            EcdsaSigningScheme::Eip712,
            &DomainSeparator(domain.0),
            &struct_hash,
            SecretKeyRef::from(&SecretKey::from_slice(owner.private_key()).unwrap()),
        );

        let permit = self.contract.permit(
            owner.address(),
            spender,
            value,
            deadline,
            signature.v,
            Bytes(signature.r.0),
            Bytes(signature.s.0),
        );

        // double check the signatures are correct...
        permit
            .clone()
            .call()
            .await
            .expect("permit signature issue; good luck figuring this one out!");

        interaction::for_transaction(permit.tx)
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

    async fn deploy_tokens<const N: usize>(&self, minter: Account) -> [MintableToken; N] {
        let mut res = Vec::with_capacity(N);
        for _ in 0..N {
            let contract = ERC20Mintable::builder(&self.web3)
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
        let tokens = self.deploy_tokens::<N>(minter).await;

        for MintableToken { contract, minter } in &tokens {
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

        tokens
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

    pub fn contracts(&self) -> &Contracts {
        &self.contracts
    }
}
