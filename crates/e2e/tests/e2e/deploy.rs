use anyhow::{Context, Result};
use contracts::{
    BalancerV2Authorizer, BalancerV2Vault, GPv2AllowListAuthentication, GPv2Settlement,
    UniswapV2Factory, UniswapV2Router02, WETH9,
};
use ethcontract::Address;
use model::DomainSeparator;
use shared::Web3;

pub struct Contracts {
    pub balancer_vault: BalancerV2Vault,
    pub gp_settlement: GPv2Settlement,
    pub uniswap_factory: UniswapV2Factory,
    pub uniswap_router: UniswapV2Router02,
    pub weth: WETH9,
    pub allowance: Address,
    pub domain_separator: DomainSeparator,
}

pub async fn deploy(web3: &Web3) -> Result<Contracts> {
    let contracts = contracts::deploy::Contracts::deploy(web3).await?;
    let allowance = contracts
        .gp_settlement
        .vault_relayer()
        .call()
        .await
        .context("Couldn't get vault relayer address")?;
    let domain_separator = contracts
        .gp_settlement
        .domain_separator()
        .call()
        .await
        .context("Couldn't query domain separator")?
        .0;
    Ok(Contracts {
        balancer_vault: contracts.balancer_vault,
        gp_settlement: contracts.gp_settlement,
        uniswap_factory: contracts.uniswap_factory,
        uniswap_router: contracts.uniswap_router,
        weth: contracts.weth,
        allowance,
        domain_separator: DomainSeparator(domain_separator),
    })
}
