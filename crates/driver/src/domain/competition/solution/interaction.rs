use crate::domain::{self, eth, liquidity};

/// Interaction with a smart contract which is needed to execute this solution
/// on the blockchain.
#[derive(Debug)]
pub enum Interaction {
    Custom(Custom),
    Liquidity(Liquidity),
}

impl Interaction {
    /// Returns whether or not the interaction should be internalized.
    pub fn internalize(&self) -> bool {
        match self {
            Interaction::Custom(custom) => custom.internalize,
            Interaction::Liquidity(liquidity) => liquidity.internalize,
        }
    }

    /// Returns the ERC20 allowances required for executing this interaction
    /// onchain.
    pub fn allowances(&self) -> Vec<eth::allowance::Required> {
        match self {
            Interaction::Custom(interaction) => interaction.allowances.clone(),
            Interaction::Liquidity(interaction) => {
                let address = match &interaction.liquidity.kind {
                    liquidity::Kind::UniswapV2(pool) => pool.router.into(),
                    liquidity::Kind::UniswapV3(_) => todo!(),
                    liquidity::Kind::BalancerV2Stable(_) => todo!(),
                    liquidity::Kind::BalancerV2Weighted(_) => todo!(),
                    liquidity::Kind::Swapr(_) => todo!(),
                    liquidity::Kind::ZeroEx(_) => todo!(),
                };
                vec![eth::Allowance {
                    spender: eth::allowance::Spender {
                        address,
                        token: interaction.input.token,
                    },
                    amount: interaction.input.amount,
                }
                .into()]
            }
        }
    }
}

/// An arbitrary interaction with any smart contract.
#[derive(Debug)]
pub struct Custom {
    pub target: eth::ContractAddress,
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
    pub liquidity: domain::Liquidity,
    pub input: eth::Asset,
    pub output: eth::Asset,
    /// Can the interaction be executed using the liquidity of our settlement
    /// contract?
    pub internalize: bool,
}
