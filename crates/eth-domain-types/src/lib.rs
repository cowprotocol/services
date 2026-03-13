pub use {
    access_list::{AccessList, StorageKey},
    allowance::Allowance,
    alloy_primitives::{Address, B256, U256, U512},
    eip712::DomainSeparator,
    ether::Ether,
    gas::{EffectiveGasPrice, FeePerGas, Gas, GasPrice},
    number::nonzero::NonZeroU256,
    token_amount::{SellTokenAmount, SurplusTokenAmount, TokenAmount},
};
use {
    alloy_primitives::Bytes,
    alloy_rpc_types::TransactionRequest,
    derive_more::{
        From,
        Into,
        derive::Deref,
    },
};

mod access_list;
pub mod allowance;
mod eip712;
mod ether;
mod gas;
mod token_amount;

/// ERC20 token address for ETH. In reality, ETH is not an ERC20 token because
/// it does not implement the ERC20 interface, but this address is used by
/// convention across the Ethereum ecosystem whenever ETH is treated like an
/// ERC20 token.
pub const ETH_TOKEN: TokenAddress = TokenAddress(Address::repeat_byte(0xee));

// TODO This type should probably use Ethereum::is_contract to verify during
// construction that it does indeed point to a contract
/// A smart contract address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deref)]
pub struct ContractAddress(Address);

/// An ERC20 token address.
///
/// https://eips.ethereum.org/EIPS/eip-20
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deref)]
pub struct TokenAddress(Address);

impl TokenAddress {
    /// If the token is ETH, return WETH, thereby converting it to erc20.
    pub fn as_erc20(self, weth: WrappedNativeToken) -> Self {
        if self == ETH_TOKEN { weth.into() } else { self }
    }
}

/// ERC20 representation of the chain's native token (e.g. WETH on mainnet,
/// WXDAI on Gnosis Chain).
#[derive(Debug, Clone, Copy, From, Into)]
pub struct WrappedNativeToken(TokenAddress);

impl From<Address> for WrappedNativeToken {
    fn from(value: Address) -> Self {
        WrappedNativeToken(value.into())
    }
}

impl From<Address> for TokenAddress {
    fn from(value: Address) -> Self {
        Self(value)
    }
}

impl From<Address> for ContractAddress {
    fn from(value: Address) -> Self {
        Self(value)
    }
}

impl From<TokenAddress> for Address {
    fn from(value: TokenAddress) -> Self {
        value.0
    }
}

impl From<ContractAddress> for Address {
    fn from(value: ContractAddress) -> Self {
        value.0
    }
}

/// An asset on the Ethereum blockchain. Represents a particular amount of a
/// particular token.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Asset {
    pub amount: TokenAmount,
    pub token: TokenAddress,
}

/// Block number.
#[derive(Debug, Copy, Clone, From, PartialEq, PartialOrd, Default)]
pub struct BlockNo(pub u64);

/// Adding blocks to a block number.
impl std::ops::Add<u64> for BlockNo {
    type Output = BlockNo;

    fn add(self, rhs: u64) -> Self::Output {
        Self(self.0 + rhs)
    }
}

/// A transaction ID, AKA transaction hash.
#[derive(Debug, Copy, Clone, From, Default)]
pub struct TxId(pub B256);

/// An onchain transaction.
#[derive(derive_more::Debug, Clone)]
pub struct Tx {
    pub from: Address,
    pub to: Address,
    pub value: Ether,
    pub input: Bytes,
    #[debug(ignore)]
    pub access_list: AccessList,
}

impl Tx {
    pub fn set_access_list(self, access_list: AccessList) -> Self {
        Self {
            access_list,
            ..self
        }
    }
}

impl From<Tx> for TransactionRequest {
    fn from(value: Tx) -> Self {
        TransactionRequest::default()
            .from(value.from)
            .to(value.to)
            .value(value.value.0)
            .input(value.input.into())
            .access_list(value.access_list.into())
    }
}
