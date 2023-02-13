use {
    crate::deploy::Contracts,
    contracts::{ERC20Mintable, GnosisSafe, GnosisSafeCompatibilityFallbackHandler},
    ethcontract::{Account, Bytes, H160, H256, U256},
    shared::{
        ethrpc::Web3,
        sources::uniswap_v2::{pair_provider::PairProvider, UNISWAP_INIT},
    },
    web3::signing::{Key as _, SecretKeyRef},
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
    ($acc:ident, $safe:ident, $call:expr) => {{
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
                $crate::onchain_components::gnosis_safe_prevalidated_signature($acc.address()),
            )
        );
    }};
}

pub fn to_wei(base: u32) -> U256 {
    U256::from(base) * U256::from(10).pow(18.into())
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

pub async fn deploy_mintable_token(web3: &Web3) -> ERC20Mintable {
    ERC20Mintable::builder(web3)
        .deploy()
        .await
        .expect("MintableERC20 deployment failed")
}

pub struct WethPoolConfig {
    pub token_amount: U256,
    pub weth_amount: U256,
}

pub struct MintableToken {
    pub contract: ERC20Mintable,
    minter: Account,
}

impl MintableToken {
    pub async fn mint(&self, to: H160, amount: U256) {
        tx!(self.minter, self.contract.mint(to, amount));
    }
}

pub async fn deploy_token_with_weth_uniswap_pool(
    web3: &Web3,
    deployed_contracts: &Contracts,
    pool_config: WethPoolConfig,
) -> MintableToken {
    let token = deploy_mintable_token(web3).await;
    let minter = Account::Local(
        web3.eth().accounts().await.expect("get accounts failed")[0],
        None,
    );

    let WethPoolConfig {
        weth_amount,
        token_amount,
    } = pool_config;

    tx!(minter, token.mint(minter.address(), token_amount));
    tx_value!(minter, weth_amount, deployed_contracts.weth.deposit());

    tx!(
        minter,
        deployed_contracts
            .uniswap_factory
            .create_pair(token.address(), deployed_contracts.weth.address())
    );
    tx!(
        minter,
        token.approve(deployed_contracts.uniswap_router.address(), token_amount)
    );
    tx!(
        minter,
        deployed_contracts
            .weth
            .approve(deployed_contracts.uniswap_router.address(), weth_amount)
    );
    tx!(
        minter,
        deployed_contracts.uniswap_router.add_liquidity(
            token.address(),
            deployed_contracts.weth.address(),
            token_amount,
            weth_amount,
            0_u64.into(),
            0_u64.into(),
            minter.address(),
            U256::max_value(),
        )
    );

    MintableToken {
        contract: token,
        minter,
    }
}

pub fn uniswap_pair_provider(contracts: &Contracts) -> PairProvider {
    PairProvider {
        factory: contracts.uniswap_factory.address(),
        init_code_digest: UNISWAP_INIT,
    }
}
