use crate::logic::eth;

/// A source of liquidity which can be used by the solver.
#[derive(Debug, Clone, Copy)]
pub struct Liquidity {
    pub id: Id,
    /// Depending on the liquidity provider, this can mean different things.
    /// Usually it's the address of the liquidity pool.
    pub address: eth::Address,
    /// Estimation of gas needed to use this liquidity on-chain.
    pub gas: eth::Gas,
    // TODO There will be plenty more data here in the future.
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Id(pub usize);
