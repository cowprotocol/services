pub use primitive_types::{H160, H256, U256};
use {
    crate::domain,
    derive_more::{Display, From, Into},
};

/// ERC20 token address for ETH. In reality, ETH is not an ERC20 token because
/// it does not implement the ERC20 interface, but this address is used by
/// convention across the Ethereum ecosystem whenever ETH is treated like an
/// ERC20 token.
/// Same address is also used for XDAI on Gnosis Chain.
pub const NATIVE_TOKEN: TokenAddress = TokenAddress(H160([0xee; 20]));

/// An address. Can be an EOA or a smart contract address.
#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, From, Into, Display,
)]
pub struct Address(pub H160);

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
pub struct TxId(pub H256);

/// An ERC20 token address.
///
/// https://eips.ethereum.org/EIPS/eip-20
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, From, Into)]
pub struct TokenAddress(pub H160);

impl TokenAddress {
    /// If the token is ETH/XDAI, return WETH/WXDAI, thereby converting it to
    /// erc20.
    pub fn as_erc20(self, wrapped: WrappedNativeToken) -> Self {
        if self == NATIVE_TOKEN {
            wrapped.into()
        } else {
            self
        }
    }
}

/// ERC20 representation of the chain's native token (e.g. WETH on mainnet,
/// WXDAI on Gnosis Chain).
#[derive(Debug, Clone, Copy, From, Into)]
pub struct WrappedNativeToken(TokenAddress);

impl From<H160> for WrappedNativeToken {
    fn from(value: H160) -> Self {
        WrappedNativeToken(value.into())
    }
}

/// An ERC20 token amount.
///
/// https://eips.ethereum.org/EIPS/eip-20
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, From, Into)]
pub struct TokenAmount(pub U256);

impl TokenAmount {
    /// Applies a factor to the token amount.
    ///
    /// The factor is first multiplied by 10^18 to convert it to integer, to
    /// avoid rounding to 0. Then, the token amount is divided by 10^18 to
    /// convert it back to the original scale.
    ///
    /// The higher the conversion factor (10^18) the precision is higher. E.g.
    /// 0.123456789123456789 will be converted to 123456789123456789.
    pub fn apply_factor(&self, factor: f64) -> Option<Self> {
        Some(
            (self
                .0
                .checked_mul(U256::from_f64_lossy(factor * 1000000000000000000.))?
                / 1000000000000000000u128)
                .into(),
        )
    }
}

/// An ERC20 sell token amount.
///
/// https://eips.ethereum.org/EIPS/eip-20
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, From, Into)]
pub struct SellTokenAmount(pub U256);

impl From<TokenAmount> for SellTokenAmount {
    fn from(value: TokenAmount) -> Self {
        Self(value.0)
    }
}

impl From<SellTokenAmount> for TokenAmount {
    fn from(value: SellTokenAmount) -> Self {
        Self(value.0)
    }
}

impl std::ops::Add for SellTokenAmount {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl num::Zero for SellTokenAmount {
    fn zero() -> Self {
        Self(U256::zero())
    }

    fn is_zero(&self) -> bool {
        self.0.is_zero()
    }
}

impl std::iter::Sum for SellTokenAmount {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(num::Zero::zero(), std::ops::Add::add)
    }
}

impl std::ops::Sub<Self> for SellTokenAmount {
    type Output = SellTokenAmount;

    fn sub(self, rhs: Self) -> Self::Output {
        self.0.sub(rhs.0).into()
    }
}

impl num::CheckedSub for SellTokenAmount {
    fn checked_sub(&self, other: &Self) -> Option<Self> {
        self.0.checked_sub(other.0).map(Into::into)
    }
}

impl num::Saturating for SellTokenAmount {
    fn saturating_add(self, v: Self) -> Self {
        self.0.saturating_add(v.0).into()
    }

    fn saturating_sub(self, v: Self) -> Self {
        self.0.saturating_sub(v.0).into()
    }
}

/// Gas amount in gas units.
///
/// The amount of Ether that is paid in transaction fees is proportional to this
/// amount as well as the transaction's [`EffectiveGasPrice`].
#[derive(Debug, Default, Display, Clone, Copy, Ord, Eq, PartialOrd, PartialEq, From, Into)]
pub struct Gas(pub U256);

/// The `effective_gas_price` as defined by EIP-1559.
///
/// https://eips.ethereum.org/EIPS/eip-1559#specification
#[derive(Debug, Clone, Copy, Display, Default)]
pub struct EffectiveGasPrice(pub Ether);

impl From<U256> for EffectiveGasPrice {
    fn from(value: U256) -> Self {
        Self(value.into())
    }
}

impl From<EffectiveGasPrice> for U256 {
    fn from(value: EffectiveGasPrice) -> Self {
        value.0.into()
    }
}

impl std::ops::Sub<Self> for TokenAmount {
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

impl std::ops::Add for TokenAmount {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl num::CheckedAdd for TokenAmount {
    fn checked_add(&self, other: &Self) -> Option<Self> {
        self.0.checked_add(other.0).map(Into::into)
    }
}

impl std::ops::Mul<Self> for TokenAmount {
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

impl std::ops::Div<Self> for TokenAmount {
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

/// An asset on the Ethereum blockchain. Represents a particular amount of a
/// particular token.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Asset {
    pub amount: TokenAmount,
    pub token: TokenAddress,
}

/// An amount of native Ether tokens denominated in wei.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, From, Into, Display, Default)]
pub struct Ether(pub U256);

impl std::ops::Add for Ether {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
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

/// Domain separator used for signing.
///
/// https://eips.ethereum.org/EIPS/eip-712#definition-of-domainseparator
#[derive(Debug, Clone, Copy)]
pub struct DomainSeparator(pub [u8; 32]);

/// Originated from the blockchain transaction input data.
pub type Calldata = crate::util::Bytes<Vec<u8>>;

/// A settlement event emitted by a settlement smart contract.
#[derive(Debug, Clone, Copy)]
pub struct SettlementEvent {
    pub block: BlockNo,
    pub log_index: u64,
    pub transaction: TxId,
}

/// A trade event emitted by a settlement smart contract.
#[derive(Debug, Clone, Copy)]
pub struct TradeEvent {
    pub block: BlockNo,
    pub log_index: u64,
    pub order_uid: domain::OrderUid,
}

/// Any type of on-chain transaction.
#[derive(Debug, Clone, Default)]
pub struct Transaction {
    /// The hash of the transaction.
    pub hash: TxId,
    /// The address of the sender of the transaction.
    pub from: Address,
    /// The call data of the transaction.
    pub input: Calldata,
    /// The block number of the block that contains the transaction.
    pub block: BlockNo,
    /// The timestamp of the block that contains the transaction.
    pub timestamp: u32,
    /// The gas used by the transaction.
    pub gas: Gas,
    /// The effective gas price of the transaction.
    pub gas_price: EffectiveGasPrice,
}
