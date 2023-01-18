/// The famous Uniswap V2 constant product pool, modelled by `x · y = k` [^1].
///
/// Note that there are many Uniswap V2 clones with identical behaviour from a
/// liquidity point of view, that are therefore modelled by the same type:
/// - SushiSwap (Mainnet and Gnosis Chain)
/// - Swapr (Mainnet and Gnosis Chain)
/// - Honeyswap (Gnosis Chain)
/// - Baoswap (Gnosis Chain)
///
/// [^1]: <https://uniswap.org/whitepaper.pdf>
#[derive(Clone, Debug)]
pub struct Pool {}
