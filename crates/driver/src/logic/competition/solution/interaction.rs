use crate::logic::{self, eth};

/// The interactions with smart contracts needed to execute the
/// [`super::Solution`] on the blockchain. The three fields correspond
/// to the [three elements] of the `interactions` parameter passed to our
/// onchain settlement contract.
///
/// [three elements]: https://github.com/cowprotocol/contracts/blob/d043b0bfac7a09463c74dfe1613d0612744ed91c/src/contracts/GPv2Settlement.sol#L125
#[derive(Debug)]
pub struct Interactions {
    /// The interactions used to set up the settlement, before the settlement
    /// contract does any work.
    pub setup: Vec<Interaction>,
    /// The interactions used to settle the trades, after the settlement
    /// contract has executed the necessary transfers into the settlement.
    pub primary: Vec<Interaction>,
    /// The final interactions executed after the settlement contract has moved
    /// funds out of the settlement. These can be used by advanced solvers e.g.
    /// for backrunning.
    pub closing: Vec<Interaction>,
}

impl Interactions {
    pub fn all(&self) -> impl Iterator<Item = &Interaction> {
        self.setup
            .iter()
            .chain(self.primary.iter())
            .chain(self.closing.iter())
    }
}

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
