use {
    crate::logic::competition::Solution,
    primitive_types::{H160, U256},
};

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

/// An Ethereum transaction.
#[derive(Debug)]
pub struct Tx(
    // Currently, this is being "faked". Ideally this should be an agnostic representation of the
    // data needed by a transaction, but this is done as-is now to make the [`crate::boundary`]
    // integration easier.
    pub Solution,
);

/// Gas amount.
#[derive(Debug, Clone, Copy)]
pub struct Gas(pub U256);

/// Gas price.
/// TODO This might need to be different, autopilot uses GasPrice1559 which must
/// have something to do with EIP-1559
#[derive(Debug, Clone, Copy)]
pub struct GasPrice(pub U256);

/// An EIP-2930 access list.
///
/// https://eips.ethereum.org/EIPS/eip-2930
#[derive(Debug)]
pub struct AccessList(pub web3::types::AccessList);

impl AccessList {
    pub fn merge(other: Self) -> Self {
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

/// An ERC20 token.
///
/// https://eips.ethereum.org/EIPS/eip-20
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Token(pub H160);

/// An ERC20 approval.
///
/// https://eips.ethereum.org/EIPS/eip-20#methods, see the approve method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Approval {
    pub spender: Spender,
    pub amount: U256,
}

/// An ERC20 allowance.
///
/// https://eips.ethereum.org/EIPS/eip-20#methods, see the allowance method.
#[derive(Debug, Clone, Copy)]
pub struct Allowance {
    pub spender: Spender,
    pub amount: U256,
}

impl Approval {
    /// Check if the approval has already been approved by the allowance.
    pub fn is_approved(&self, allowance: &Allowance) -> bool {
        self.spender == allowance.spender && self.amount <= allowance.amount
    }
}

/// The spender of an ERC20 token. Every [`Allowance`] and [`Approval`] is tied
/// to a particular spender.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Spender {
    pub address: Address,
    pub token: Token,
}
