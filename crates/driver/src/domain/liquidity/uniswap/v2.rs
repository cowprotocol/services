use {crate::domain::eth, std::cmp::Ordering};

/// The famous Uniswap V2 constant product pool, modelled by `x · y = k` [^1].
///
/// Note that there are many Uniswap V2 clones with identical behaviour from a
/// liquidity point of view, that are therefore modelled by the same type:
/// - SushiSwap (Mainnet and Gnosis Chain)
/// - Honeyswap (Gnosis Chain)
/// - Baoswap (Gnosis Chain)
///
/// [^1]: <https://uniswap.org/whitepaper.pdf>
#[derive(Clone, Debug)]
pub struct Pool {
    pub address: eth::Address,
    pub router: eth::ContractAddress,
    pub reserves: Reserves,
}

/// A Uniswap V2 pool reserves. These reserves are orders by token address and
/// are guaranteed to be for distict tokens.
#[derive(Clone, Copy, Debug)]
pub struct Reserves(eth::Asset, eth::Asset);

impl Reserves {
    /// Creates new Uniswap V2 token reserves, returns `None` if the specified
    /// token addresses are equal.
    pub fn new(a: eth::Asset, b: eth::Asset) -> Option<Self> {
        match a.token.cmp(&b.token) {
            Ordering::Less => Some(Self(a, b)),
            Ordering::Equal => None,
            Ordering::Greater => Some(Self(b, a)),
        }
    }
}
