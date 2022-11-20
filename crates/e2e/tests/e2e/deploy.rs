use anyhow::{Context, Result};
use contracts::{
    BalancerV2Authorizer, BalancerV2Vault, CoWSwapEthFlow, GPv2AllowListAuthentication,
    GPv2Settlement, UniswapV2Factory, UniswapV2Router02, WETH9,
};
use ethcontract::{Address, U256};
use model::DomainSeparator;
use shared::ethrpc::Web3;

pub struct Contracts {
    pub balancer_vault: BalancerV2Vault,
    pub gp_settlement: GPv2Settlement,
    pub uniswap_factory: UniswapV2Factory,
    pub uniswap_router: UniswapV2Router02,
    pub weth: WETH9,
    pub allowance: Address,
    pub domain_separator: DomainSeparator,
    pub ethflow: CoWSwapEthFlow,
}

pub async fn deploy(web3: &Web3) -> Result<Contracts> {
    let network_id = web3.net().version().await.expect("get network ID failed");
    tracing::info!("connected to test network {}", network_id);

    let accounts: Vec<Address> = web3.eth().accounts().await.expect("get accounts failed");
    let admin = accounts[0];

    macro_rules! deploy {
            ($contract:ident) => { deploy!($contract ()) };
            ($contract:ident ( $($param:expr),* $(,)? )) => {
                deploy!($contract ($($param),*) as stringify!($contract))
            };
            ($contract:ident ( $($param:expr),* $(,)? ) as $name:expr) => {{
                let name = $name;
                let instance = $contract::builder(&web3 $(, $param)*)
                    .deploy()
                    .await
                    .with_context(|| format!("failed to deploy {}", name))?;
                instance
            }};
        }

    let weth = deploy!(WETH9());

    let balancer_authorizer = deploy!(BalancerV2Authorizer(admin));
    let balancer_vault = deploy!(BalancerV2Vault(
        balancer_authorizer.address(),
        weth.address(),
        U256::from(0),
        U256::from(0),
    ));

    let uniswap_factory = deploy!(UniswapV2Factory(accounts[0]));
    let uniswap_router = deploy!(UniswapV2Router02(uniswap_factory.address(), weth.address()));

    let gp_authentication = deploy!(GPv2AllowListAuthentication);
    gp_authentication
        .initialize_manager(admin)
        .send()
        .await
        .context("failed to initialize manager")?;
    let gp_settlement = deploy!(GPv2Settlement(
        gp_authentication.address(),
        balancer_vault.address(),
    ));

    gp_authentication
        .add_solver(admin)
        .send()
        .await
        .context("failed to allow list account 0")?;

    contracts::vault::grant_required_roles(
        &balancer_authorizer,
        balancer_vault.address(),
        gp_settlement
            .vault_relayer()
            .call()
            .await
            .context("failed to retrieve Vault relayer contract address")?,
    )
    .await
    .context("failed to authorize Vault relayer")?;

    let allowance = gp_settlement
        .vault_relayer()
        .call()
        .await
        .expect("Couldn't get vault relayer address");
    let domain_separator = DomainSeparator(
        gp_settlement
            .domain_separator()
            .call()
            .await
            .expect("Couldn't query domain separator")
            .0,
    );

    let ethflow = deploy!(CoWSwapEthFlow(gp_settlement.address(), weth.address(),));

    Ok(Contracts {
        balancer_vault,
        gp_settlement,
        uniswap_factory,
        uniswap_router,
        weth,
        allowance,
        domain_separator,
        ethflow,
    })
}
