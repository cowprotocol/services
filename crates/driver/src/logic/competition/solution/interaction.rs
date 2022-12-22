use crate::logic::{self, eth};

/// Interaction with a smart contract which is needed to execute this solution
/// on the blockchain.
#[derive(Debug)]
pub enum Interaction {
    Custom(Custom),
    Liquidity(Liquidity),
}

/// An arbitrary interaction with any smart contract.
#[derive(Debug)]
pub struct Custom {
    pub target: eth::Address,
    pub value: eth::Ether,
    pub call_data: Vec<u8>,
    pub allowances: Vec<eth::allowance::Required>,
    pub inputs: Vec<eth::Asset>,
    pub outputs: Vec<eth::Asset>,
    /// Can the interaction be executed using the liquidity of our settlement
    /// contract?
    pub internalize: bool,
}

/// An interaction with one of the smart contracts for which we index
/// liquidity.
#[derive(Debug)]
pub struct Liquidity {
    pub liquidity: logic::Liquidity,
    pub input: eth::Asset,
    pub output: eth::Asset,
    /// Can the interaction be executed using the liquidity of our settlement
    /// contract?
    pub internalize: bool,
}
