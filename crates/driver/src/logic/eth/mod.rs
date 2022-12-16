use primitive_types::{H160, U256};

pub mod allowance;
mod eip712;

pub use {allowance::Allowance, eip712::DomainSeparator};

// TODO It might make sense to re-export H160 and U256 from here and not
// reference primitive_types directly anywhere, it's probably the best idea

// TODO Constructing this type should probably do some validation, or maybe this
// should be an enum with a Display implementation
/// Name of an Ethereum network, e.g. mainnet or testnet.
#[derive(Debug)]
pub struct NetworkName(pub String);

/// Chain ID as defined by EIP-155.
///
/// https://eips.ethereum.org/EIPS/eip-155
#[derive(Debug, Clone, Copy)]
pub struct ChainId(pub u64);

/// Gas amount.
#[derive(Debug, Clone, Copy)]
pub struct Gas(pub U256);

/// Gas price.
/// TODO This will probably need to be different, autopilot uses GasPrice1559
#[derive(Debug, Clone, Copy)]
pub struct GasPrice(pub U256);

/// An EIP-2930 access list.
///
/// https://eips.ethereum.org/EIPS/eip-2930
#[derive(Debug)]
pub struct AccessList(pub web3::types::AccessList);

impl AccessList {
    pub fn merge(_other: Self) -> Self {
        todo!()
    }
}

/// The results of an Ethereum transaction simulation.
#[derive(Debug)]
pub struct Simulation {
    pub gas: Gas,
    pub access_list: AccessList,
}

/// An address. Can be an EOA or a smart contract address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Address(pub H160);

impl From<H160> for Address {
    fn from(inner: H160) -> Self {
        Self(inner)
    }
}

impl From<Address> for H160 {
    fn from(address: Address) -> Self {
        address.0
    }
}

/// A contract address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Contract(pub H160);

impl From<H160> for Contract {
    fn from(inner: H160) -> Self {
        Self(inner)
    }
}

impl From<Contract> for ethereum_types::H160 {
    fn from(contract: Contract) -> Self {
        contract.0 .0.into()
    }
}

/// The contract is an address on the blockchain.
impl From<Contract> for Address {
    fn from(contract: Contract) -> Self {
        contract.0.into()
    }
}

/// An ERC20 token.
///
/// https://eips.ethereum.org/EIPS/eip-20
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Token(pub H160);

impl From<Token> for H160 {
    fn from(token: Token) -> Self {
        token.0
    }
}

/// An asset on the Ethereum blockchain. Represents a particular amount of a
/// particular token.
#[derive(Debug, Clone, Copy)]
pub struct Asset {
    pub amount: U256,
    pub token: Token,
}

/// An amount of native Ether tokens denominated in wei.
#[derive(Debug, Clone, Copy)]
pub struct Ether(pub U256);

impl From<Ether> for num::BigInt {
    fn from(ether: Ether) -> Self {
        let mut bytes = [0; 32];
        ether.0.to_big_endian(&mut bytes);
        num::BigUint::from_bytes_be(&bytes).into()
    }
}

impl From<Ether> for U256 {
    fn from(ether: Ether) -> Self {
        ether.0
    }
}

/// Block number.
#[derive(Debug, Clone, Copy)]
pub struct BlockNo(pub u64);

/// An onchain transaction which interacts with a smart contract.
#[derive(Debug)]
pub struct Interaction {
    pub target: Address,
    pub value: Ether,
    pub call_data: Vec<u8>,
}
