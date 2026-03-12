pub use {
    access_list::{AccessList, StorageKey},
    allowance::Allowance,
    alloy_primitives::{Address, B256, U256, U512},
    eip712::DomainSeparator,
    gas::{EffectiveGasPrice, FeePerGas, Gas, GasPrice},
    number::nonzero::NonZeroU256,
    token_amount::{SellTokenAmount, SurplusTokenAmount, TokenAmount},
};
use {
    alloy_primitives::Bytes,
    alloy_rpc_types::TransactionRequest,
    derive_more::{From, Into, derive::Display},
    solvers_dto::auction::FlashloanHint,
};

mod access_list;
pub mod allowance;
mod eip712;
mod gas;
mod token_amount;

// TODO This module is getting a little hectic with all kinds of different
// types, I wonder if there could be meaningful submodules?

/// ERC20 token address for ETH. In reality, ETH is not an ERC20 token because
/// it does not implement the ERC20 interface, but this address is used by
/// convention across the Ethereum ecosystem whenever ETH is treated like an
/// ERC20 token.
pub const ETH_TOKEN: TokenAddress = TokenAddress(Address::repeat_byte(0xee));

/// An ERC20 token address.
///
/// https://eips.ethereum.org/EIPS/eip-20
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TokenAddress(pub Address);

impl TokenAddress {
    /// If the token is ETH, return WETH, thereby converting it to erc20.
    pub fn as_erc20(self, weth: WethAddress) -> Self {
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

/// The address of the WETH contract.
#[derive(Debug, Clone, Copy, From, Into)]
pub struct WethAddress(pub TokenAddress);

impl From<Address> for WethAddress {
    fn from(value: Address) -> Self {
        WethAddress(value.into())
    }
}

impl From<Address> for TokenAddress {
    fn from(value: Address) -> Self {
        Self(value)
    }
}

impl From<TokenAddress> for Address {
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
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    Ord,
    PartialEq,
    PartialOrd,
    From,
    Into,
    Display,
    Default,
    derive_more::Add,
    derive_more::Sub,
)]
pub struct Ether(pub U256);

impl From<Ether> for num::BigInt {
    fn from(value: Ether) -> Self {
        num::BigUint::from_bytes_be(value.0.to_be_bytes::<32>().as_slice()).into()
    }
}

// TODO: check if actually needed
impl From<i32> for Ether {
    fn from(value: i32) -> Self {
        Self(U256::from(value))
    }
}

impl num::Saturating for Ether {
    fn saturating_add(self, v: Self) -> Self {
        Self(self.0.saturating_add(v.0))
    }

    fn saturating_sub(self, v: Self) -> Self {
        Self(self.0.saturating_sub(v.0))
    }
}

impl num::CheckedSub for Ether {
    fn checked_sub(&self, v: &Self) -> Option<Self> {
        self.0.checked_sub(v.0).map(Ether)
    }
}

impl num::Zero for Ether {
    fn zero() -> Self {
        Self(U256::ZERO)
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

/// Originated from the blockchain transaction input data.
pub type Calldata = alloy_primitives::Bytes;

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

/// An onchain transaction which interacts with a smart contract.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Interaction {
    pub target: Address,
    pub value: Ether,
    pub call_data: Bytes,
}

impl From<Interaction> for model::interaction::InteractionData {
    fn from(interaction: Interaction) -> Self {
        Self {
            target: interaction.target,
            value: interaction.value.0,
            call_data: interaction.call_data.to_vec(),
        }
    }
}

impl From<model::interaction::InteractionData> for Interaction {
    fn from(interaction: model::interaction::InteractionData) -> Self {
        Self {
            target: interaction.target,
            value: interaction.value.into(),
            call_data: interaction.call_data.into(),
        }
    }
}

/// A transaction ID, AKA transaction hash.
#[derive(Debug, Copy, Clone, From, Default)]
pub struct TxId(pub B256);

pub enum TxStatus {
    /// The transaction has been included and executed successfully.
    Executed { block_number: BlockNo },
    /// The transaction has been included but execution failed.
    Reverted { block_number: BlockNo },
    /// The transaction has not been included yet.
    Pending,
}

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

/// The Keccak-256 hash of a contract's initialization code.
///
/// This value is meaningful in the context of the EVM `CREATE2` opcode in that
/// it influences the deterministic address that the contract ends up on.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CodeDigest(pub B256);

impl From<B256> for CodeDigest {
    fn from(value: B256) -> Self {
        Self(value)
    }
}

impl From<CodeDigest> for B256 {
    fn from(value: CodeDigest) -> Self {
        value.0
    }
}

impl From<[u8; 32]> for CodeDigest {
    fn from(value: [u8; 32]) -> Self {
        Self(B256::new(value))
    }
}

impl From<CodeDigest> for [u8; 32] {
    fn from(value: CodeDigest) -> Self {
        value.0.0
    }
}

#[derive(Debug, Clone)]
pub struct Flashloan {
    pub liquidity_provider: Address,
    pub protocol_adapter: Address,
    pub receiver: Address,
    pub token: TokenAddress,
    pub amount: TokenAmount,
}

impl From<&solvers_dto::solution::Flashloan> for Flashloan {
    fn from(value: &solvers_dto::solution::Flashloan) -> Self {
        Self {
            liquidity_provider: value.liquidity_provider,
            protocol_adapter: value.protocol_adapter,
            receiver: value.receiver,
            token: value.token.into(),
            amount: value.amount.into(),
        }
    }
}

#[expect(clippy::from_over_into)]
impl Into<FlashloanHint> for &Flashloan {
    fn into(self) -> FlashloanHint {
        FlashloanHint {
            liquidity_provider: self.liquidity_provider,
            protocol_adapter: self.protocol_adapter,
            receiver: self.receiver,
            token: self.token.into(),
            amount: self.amount.into(),
        }
    }
}

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
    pub order_uid: model::order::OrderUid,
}

/// Call frames of a transaction.
#[derive(Clone, Debug, Default)]
pub struct CallFrame {
    /// The address of the call initiator.
    pub from: Address,
    /// The address of the contract that was called.
    pub to: Option<Address>,
    /// Calldata input.
    pub input: Calldata,
    /// Recorded child calls.
    pub calls: Vec<CallFrame>,
}

impl From<alloy_rpc_types::trace::geth::CallFrame> for CallFrame {
    fn from(value: alloy_rpc_types::trace::geth::CallFrame) -> Self {
        Self {
            from: value.from,
            to: value.to,
            input: value.input,
            calls: value.calls.into_iter().map(Into::into).collect(),
        }
    }
}

/// Any type of on-chain transaction.
#[derive(Debug, Clone, Default)]
pub struct Transaction {
    /// The hash of the transaction.
    pub hash: TxId,
    /// The address of the sender of the transaction.
    pub from: Address,
    /// The block number of the block that contains the transaction.
    pub block: BlockNo,
    /// The timestamp of the block that contains the transaction.
    pub timestamp: u32,
    /// The gas used by the transaction.
    pub gas: Gas,
    /// The effective gas price of the transaction.
    pub gas_price: EffectiveGasPrice,
    /// Traces of all Calls contained in the transaction.
    pub trace_calls: CallFrame,
}
