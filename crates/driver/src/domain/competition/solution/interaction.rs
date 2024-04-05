use crate::{
    domain::{self, eth, liquidity},
    util::Bytes,
};

/// Interaction with a smart contract which is needed to execute this solution
/// on the blockchain.
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum Interaction {
    Custom(Custom),
    Liquidity(Liquidity),
}

impl Interaction {
    /// Should the interaction be internalized?
    pub fn internalize(&self) -> bool {
        match self {
            Interaction::Custom(custom) => custom.internalize,
            Interaction::Liquidity(liquidity) => liquidity.internalize,
        }
    }

    /// The assets consumed by this interaction. These assets are taken from the
    /// settlement contract when the interaction executes.
    pub fn inputs(&self) -> Vec<eth::Asset> {
        match self {
            Interaction::Custom(custom) => custom.inputs.clone(),
            Interaction::Liquidity(liquidity) => vec![liquidity.input],
        }
    }

    /// The assets output by this interaction. These assets are sent into the
    /// settlement contract when the interaction executes.
    pub fn outputs(&self) -> Vec<eth::Asset> {
        match self {
            Interaction::Custom(custom) => custom.outputs.clone(),
            Interaction::Liquidity(liquidity) => vec![liquidity.output],
        }
    }

    /// Returns the ERC20 approvals required for executing this interaction
    /// onchain.
    pub fn allowances(&self) -> Vec<eth::allowance::Required> {
        match self {
            Interaction::Custom(interaction) => interaction.allowances.clone(),
            Interaction::Liquidity(interaction) => {
                let address = match &interaction.liquidity.kind {
                    liquidity::Kind::UniswapV2(pool) => pool.router.into(),
                    liquidity::Kind::UniswapV3(pool) => pool.router.into(),
                    liquidity::Kind::BalancerV2Stable(pool) => pool.vault.into(),
                    liquidity::Kind::BalancerV2Weighted(pool) => pool.vault.into(),
                    liquidity::Kind::Swapr(pool) => pool.base.router.into(),
                    liquidity::Kind::ZeroEx(pool) => pool.zeroex.address().into(),
                };
                vec![eth::Allowance {
                    token: interaction.input.token,
                    spender: address,
                    amount: interaction.input.amount.into(),
                }
                .into()]
            }
        }
    }
}

/// An arbitrary interaction with any smart contract.
#[derive(Debug, Clone)]
pub struct Custom {
    pub target: eth::ContractAddress,
    pub value: eth::Ether,
    pub call_data: Bytes<Vec<u8>>,
    pub allowances: Vec<eth::allowance::Required>,
    /// See the [`Interaction::inputs`] method.
    pub inputs: Vec<eth::Asset>,
    /// See the [`Interaction::outputs`] method.
    pub outputs: Vec<eth::Asset>,
    /// Can the interaction be executed using the liquidity of our settlement
    /// contract?
    pub internalize: bool,
}

/// An interaction with one of the smart contracts for which we index
/// liquidity.
#[derive(Debug, Clone)]
pub struct Liquidity {
    pub liquidity: domain::Liquidity,
    /// See the [`Interaction::inputs`] method.
    pub input: eth::Asset,
    /// See the [`Interaction::outputs`] method.
    pub output: eth::Asset,
    /// Can the interaction be executed using the funds which belong to our
    /// settlement contract?
    pub internalize: bool,
}
