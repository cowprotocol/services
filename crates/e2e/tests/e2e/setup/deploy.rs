use {
    contracts::{
        BalancerV2Authorizer,
        BalancerV2Vault,
        CoWSwapEthFlow,
        GPv2AllowListAuthentication,
        GPv2Settlement,
        UniswapV2Factory,
        UniswapV2Router02,
        WETH9,
    },
    ethcontract::{Address, U256},
    model::DomainSeparator,
    shared::ethrpc::Web3,
};

pub struct Contracts {
    pub balancer_vault: BalancerV2Vault,
    pub gp_settlement: GPv2Settlement,
    pub gp_authenticator: GPv2AllowListAuthentication,
    pub uniswap_v2_factory: UniswapV2Factory,
    pub uniswap_v2_router: UniswapV2Router02,
    pub weth: WETH9,
    pub allowance: Address,
    pub domain_separator: DomainSeparator,
    pub ethflow: CoWSwapEthFlow,
}

impl Contracts {
    pub async fn deploy(web3: &Web3) -> Self {
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
                    $contract::builder(&web3 $(, $param)*)
                        .deploy()
                        .await
                        .unwrap_or_else(|e| panic!("failed to deploy {name}: {e:?}"))
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

        let uniswap_v2_factory = deploy!(UniswapV2Factory(accounts[0]));
        let uniswap_v2_router = deploy!(UniswapV2Router02(
            uniswap_v2_factory.address(),
            weth.address()
        ));

        let gp_authenticator = deploy!(GPv2AllowListAuthentication);
        gp_authenticator
            .initialize_manager(admin)
            .send()
            .await
            .expect("failed to initialize manager");
        let gp_settlement = deploy!(GPv2Settlement(
            gp_authenticator.address(),
            balancer_vault.address(),
        ));

        contracts::vault::grant_required_roles(
            &balancer_authorizer,
            balancer_vault.address(),
            gp_settlement
                .vault_relayer()
                .call()
                .await
                .expect("failed to retrieve Vault relayer contract address"),
        )
        .await
        .expect("failed to authorize Vault relayer");

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

        Self {
            balancer_vault,
            gp_settlement,
            gp_authenticator,
            uniswap_v2_factory,
            uniswap_v2_router,
            weth,
            allowance,
            domain_separator,
            ethflow,
        }
    }
}
