use {chain::Chain, ethereum_types::H160};

#[derive(Debug, Clone)]
pub struct Config {
    pub rpc_url: String,
    #[allow(dead_code)]
    pub weth: H160,
    pub uniswap_v2_router: H160,
    #[allow(dead_code)]
    pub chain_id: Option<Chain>,
}

/// Get the settlement contract address for a given chain
pub fn get_settlement_address(chain: Chain) -> anyhow::Result<H160> {
    // GPv2Settlement is deployed at the same address on all supported chains
    // See: crates/contracts/src/alloy.rs GPv2Settlement deployments
    let address = match chain {
        Chain::Mainnet
        | Chain::Gnosis
        | Chain::Sepolia
        | Chain::ArbitrumOne
        | Chain::Base
        | Chain::Avalanche
        | Chain::Bnb
        | Chain::Optimism
        | Chain::Polygon => "0x9008D19f58AAbD9eD0D60971565AA8510560ab41"
            .parse()
            .expect("valid settlement address"),
        _ => anyhow::bail!("unsupported chain for settlement address: {:?}", chain),
    };
    Ok(address)
}

pub fn get_uniswap_v2_router_address(chain: Chain) -> anyhow::Result<H160> {
    // Uniswap v2 router is deployed at the same address on all supported chains
    // See: crates/contracts/src/alloy.rs GPv2Settlement deployments
    let address = match chain {
        Chain::Mainnet
        | Chain::Gnosis
        | Chain::Sepolia
        | Chain::ArbitrumOne
        | Chain::Base
        | Chain::Avalanche
        | Chain::Bnb
        | Chain::Optimism
        | Chain::Polygon => "0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D"
            .parse()
            .expect("valid uniswap address"),
        _ => anyhow::bail!("unsupported chain for uniswap address: {:?}", chain),
    };
    Ok(address)
}

pub fn get_weth_address(chain: Chain) -> anyhow::Result<H160> {
    let address = match chain {
        Chain::Mainnet => "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
        Chain::Gnosis => "0xe91D153E0b41518A2Ce8Dd3D7944Fa863463a97d",
        Chain::Sepolia => "0xfFf9976782d46CC05630D1f6eBAb18b2324d6B14",
        Chain::ArbitrumOne => "0x82aF49447D8a07e3bd95BD0d56f35241523fBab1",
        _ => anyhow::bail!("unsupported chain for WETH address: {:?}", chain),
    };
    Ok(address.parse()?)
}
