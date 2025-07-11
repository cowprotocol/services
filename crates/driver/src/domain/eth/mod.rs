use {
    crate::util::{Bytes, conv::u256::U256Ext},
    derive_more::{From, Into},
    itertools::Itertools,
    solvers_dto::auction::FlashloanHint,
    std::{
        collections::{HashMap, HashSet},
        ops::{Div, Mul, Sub},
    },
    web3::types::CallRequest,
};

pub mod allowance;
mod eip712;
mod gas;

pub use {
    allowance::Allowance,
    eip712::DomainSeparator,
    gas::{EffectiveGasPrice, FeePerGas, Gas, GasPrice},
    number::nonzero::U256 as NonZeroU256,
    primitive_types::{H160, H256, U256, U512},
};

// TODO This module is getting a little hectic with all kinds of different
// types, I wonder if there could be meaningful submodules?

/// ERC20 token address for ETH. In reality, ETH is not an ERC20 token because
/// it does not implement the ERC20 interface, but this address is used by
/// convention across the Ethereum ecosystem whenever ETH is treated like an
/// ERC20 token.
pub const ETH_TOKEN: TokenAddress = TokenAddress(ContractAddress(H160([0xee; 20])));

/// An EIP-2930 access list. This type ensures that the addresses and storage
/// keys are not repeated, and that the ordering is deterministic.
///
/// https://eips.ethereum.org/EIPS/eip-2930
#[derive(Debug, Clone, Default)]
pub struct AccessList(HashMap<Address, HashSet<StorageKey>>);

impl AccessList {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl AccessList {
    /// Merge two access lists together.
    pub fn merge(mut self, other: Self) -> Self {
        for (address, storage_keys) in other.0.into_iter() {
            self.0.entry(address).or_default().extend(storage_keys);
        }
        self
    }
}

impl From<web3::types::AccessList> for AccessList {
    fn from(value: web3::types::AccessList) -> Self {
        Self(
            value
                .into_iter()
                .map(|item| {
                    (
                        item.address.into(),
                        item.storage_keys
                            .into_iter()
                            .map(|key| key.into())
                            .collect(),
                    )
                })
                .collect(),
        )
    }
}

impl From<AccessList> for web3::types::AccessList {
    fn from(value: AccessList) -> Self {
        value
            .0
            .into_iter()
            .sorted_by_key(|&(address, _)| address)
            .map(|(address, storage_keys)| web3::types::AccessListItem {
                address: address.into(),
                storage_keys: storage_keys.into_iter().sorted().map(|key| key.0).collect(),
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Into, From)]
struct StorageKey(pub H256);

/// An address. Can be an EOA or a smart contract address.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, From, Into)]
pub struct Address(pub H160);

// TODO This type should probably use Ethereum::is_contract to verify during
// construction that it does indeed point to a contract
/// A smart contract address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Into, From)]
pub struct ContractAddress(pub H160);

impl From<ContractAddress> for Address {
    fn from(value: ContractAddress) -> Self {
        value.0.into()
    }
}

/// An ERC20 token address.
///
/// https://eips.ethereum.org/EIPS/eip-20
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TokenAddress(pub ContractAddress);

impl TokenAddress {
    /// If the token is ETH, return WETH, thereby converting it to erc20.
    pub fn as_erc20(self, weth: WethAddress) -> Self {
        if self == ETH_TOKEN { weth.into() } else { self }
    }
}

/// An ERC20 token amount.
///
/// https://eips.ethereum.org/EIPS/eip-20
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, From, Into)]
pub struct TokenAmount(pub U256);

impl TokenAmount {
    /// Applies a factor to the token amount.
    pub fn apply_factor(&self, factor: f64) -> Option<Self> {
        Some(self.0.checked_mul_f64(factor)?.into())
    }
}

/// A value denominated in an order's surplus token (buy token for
/// sell orders and sell token for buy orders).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, From, Into)]
pub struct SurplusTokenAmount(pub U256);

impl Sub<Self> for TokenAmount {
    type Output = TokenAmount;

    fn sub(self, rhs: Self) -> Self::Output {
        self.0.sub(rhs.0).into()
    }
}

impl num::CheckedSub for TokenAmount {
    fn checked_sub(&self, other: &Self) -> Option<Self> {
        self.0.checked_sub(other.0).map(Into::into)
    }
}

impl Mul<Self> for TokenAmount {
    type Output = TokenAmount;

    fn mul(self, rhs: Self) -> Self::Output {
        self.0.mul(rhs.0).into()
    }
}

impl num::CheckedMul for TokenAmount {
    fn checked_mul(&self, other: &Self) -> Option<Self> {
        self.0.checked_mul(other.0).map(Into::into)
    }
}

impl num::CheckedAdd for TokenAmount {
    fn checked_add(&self, other: &Self) -> Option<Self> {
        self.0.checked_add(other.0).map(Into::into)
    }
}

impl Div<Self> for TokenAmount {
    type Output = TokenAmount;

    fn div(self, rhs: Self) -> Self::Output {
        self.0.div(rhs.0).into()
    }
}

impl num::CheckedDiv for TokenAmount {
    fn checked_div(&self, other: &Self) -> Option<Self> {
        self.0.checked_div(other.0).map(Into::into)
    }
}

impl From<u128> for TokenAmount {
    fn from(value: u128) -> Self {
        Self(value.into())
    }
}

impl std::ops::Add for TokenAmount {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl std::ops::AddAssign for TokenAmount {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl num::Zero for TokenAmount {
    fn zero() -> Self {
        Self(U256::zero())
    }

    fn is_zero(&self) -> bool {
        self.0.is_zero()
    }
}

impl std::fmt::Display for TokenAmount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// The address of the WETH contract.
#[derive(Debug, Clone, Copy, From, Into)]
pub struct WethAddress(pub TokenAddress);

impl From<H160> for WethAddress {
    fn from(value: H160) -> Self {
        WethAddress(value.into())
    }
}

impl From<H160> for TokenAddress {
    fn from(value: H160) -> Self {
        Self(value.into())
    }
}

impl From<TokenAddress> for H160 {
    fn from(value: TokenAddress) -> Self {
        value.0.into()
    }
}

impl From<TokenAddress> for ContractAddress {
    fn from(value: TokenAddress) -> Self {
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

/// An amount of native Ether tokens denominated in wei.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, From, Into)]
pub struct Ether(pub U256);

impl From<Ether> for num::BigInt {
    fn from(value: Ether) -> Self {
        let mut bytes = [0; 32];
        value.0.to_big_endian(&mut bytes);
        num::BigUint::from_bytes_be(&bytes).into()
    }
}

impl From<i32> for Ether {
    fn from(value: i32) -> Self {
        Self(value.into())
    }
}

impl std::ops::Add for Ether {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl num::Zero for Ether {
    fn zero() -> Self {
        Self(U256::zero())
    }

    fn is_zero(&self) -> bool {
        self.0.is_zero()
    }
}

impl std::iter::Sum for Ether {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(num::Zero::zero(), std::ops::Add::add)
    }
}

/// Block number.
#[derive(Debug, Clone, Copy, From, Into)]
pub struct BlockNo(pub u64);

/// An onchain transaction which interacts with a smart contract.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Interaction {
    pub target: Address,
    pub value: Ether,
    pub call_data: Bytes<Vec<u8>>,
}

impl From<Interaction> for model::interaction::InteractionData {
    fn from(interaction: Interaction) -> Self {
        Self {
            target: interaction.target.into(),
            value: interaction.value.into(),
            call_data: interaction.call_data.into(),
        }
    }
}

impl From<model::interaction::InteractionData> for Interaction {
    fn from(interaction: model::interaction::InteractionData) -> Self {
        Self {
            target: interaction.target.into(),
            value: interaction.value.into(),
            call_data: interaction.call_data.into(),
        }
    }
}

/// A transaction ID, AKA transaction hash.
#[derive(Clone, Debug, From, Into)]
pub struct TxId(pub H256);

pub enum TxStatus {
    /// The transaction has been included and executed successfully.
    Executed { block_number: BlockNo },
    /// The transaction has been included but execution failed.
    Reverted { block_number: BlockNo },
    /// The transaction has not been included yet.
    Pending,
}

/// An onchain transaction.
#[derive(Clone)]
pub struct Tx {
    pub from: Address,
    pub to: Address,
    pub value: Ether,
    pub input: Bytes<Vec<u8>>,
    pub access_list: AccessList,
}

impl From<Tx> for CallRequest {
    fn from(value: Tx) -> Self {
        Self {
            from: Some(value.from.into()),
            to: Some(value.to.into()),
            gas: None,
            gas_price: None,
            value: Some(value.value.into()),
            data: Some(value.input.into()),
            transaction_type: None,
            access_list: Some(value.access_list.into()),
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
        }
    }
}

impl std::fmt::Debug for Tx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tx")
            .field("from", &self.from)
            .field("to", &self.to)
            .field("value", &self.value)
            .field("input", &self.input)
            .field("access_list", &self.access_list)
            .finish()
    }
}

impl Tx {
    pub fn set_access_list(self, access_list: AccessList) -> Self {
        Self {
            access_list,
            ..self
        }
    }
}

/// The Keccak-256 hash of a contract's initialization code.
///
/// This value is meaningful in the context of the EVM `CREATE2` opcode in that
/// it influences the deterministic address that the contract ends up on.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CodeDigest(pub H256);

impl From<H256> for CodeDigest {
    fn from(value: H256) -> Self {
        Self(value)
    }
}

impl From<CodeDigest> for H256 {
    fn from(value: CodeDigest) -> Self {
        value.0
    }
}

impl From<[u8; 32]> for CodeDigest {
    fn from(value: [u8; 32]) -> Self {
        Self(H256(value))
    }
}

impl From<CodeDigest> for [u8; 32] {
    fn from(value: CodeDigest) -> Self {
        value.0.0
    }
}

#[derive(Debug, Clone)]
pub struct Flashloan {
    pub lender: ContractAddress,
    pub borrower: Address,
    pub token: TokenAddress,
    pub amount: TokenAmount,
}

impl From<&solvers_dto::solution::Flashloan> for Flashloan {
    fn from(value: &solvers_dto::solution::Flashloan) -> Self {
        Self {
            lender: value.lender.into(),
            borrower: value.borrower.into(),
            token: value.token.into(),
            amount: value.amount.into(),
        }
    }
}

#[allow(clippy::from_over_into)]
impl Into<FlashloanHint> for &Flashloan {
    fn into(self) -> FlashloanHint {
        FlashloanHint {
            lender: self.lender.into(),
            borrower: self.borrower.into(),
            token: self.token.into(),
            amount: self.amount.into(),
        }
    }
}
