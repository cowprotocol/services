use {
    contracts::{
        BalancerV2Authorizer,
        BalancerV2Vault,
        CoWSwapEthFlow,
        CowAmmLegacyHelper,
        GPv2AllowListAuthentication,
        GPv2Settlement,
        HooksTrampoline,
        UniswapV2Factory,
        UniswapV2Router02,
        WETH9,
    },
    ethcontract::{errors::DeployError, Address, H256, U256},
    model::DomainSeparator,
    shared::ethrpc::Web3,
};

pub struct Contracts {
    pub chain_id: u64,
    pub balancer_vault: BalancerV2Vault,
    pub gp_settlement: GPv2Settlement,
    pub gp_authenticator: GPv2AllowListAuthentication,
    pub uniswap_v2_factory: UniswapV2Factory,
    pub uniswap_v2_router: UniswapV2Router02,
    pub weth: WETH9,
    pub allowance: Address,
    pub domain_separator: DomainSeparator,
    pub ethflow: CoWSwapEthFlow,
    pub hooks: HooksTrampoline,
    pub cow_amm_helper: Option<CowAmmLegacyHelper>,
}

impl Contracts {
    pub async fn deployed(web3: &Web3) -> Self {
        let network_id = web3
            .eth()
            .chain_id()
            .await
            .expect("get network ID failed")
            .to_string();
        tracing::info!("connected to forked test network {}", network_id);

        let gp_settlement = GPv2Settlement::deployed(web3).await.unwrap();
        let cow_amm_helper = match contracts::CowAmmLegacyHelper::deployed(web3).await {
            Err(DeployError::NotFound(_)) => None,
            Err(err) => panic!("failed to find deployed contract: {:?}", err),
            Ok(contract) => Some(contract)
        };

        Self {
            chain_id: network_id
                .parse()
                .expect("Couldn't parse network ID to u64"),
            balancer_vault: BalancerV2Vault::deployed(web3).await.unwrap(),
            gp_authenticator: GPv2AllowListAuthentication::deployed(web3).await.unwrap(),
            uniswap_v2_factory: UniswapV2Factory::deployed(web3).await.unwrap(),
            uniswap_v2_router: UniswapV2Router02::deployed(web3).await.unwrap(),
            weth: WETH9::deployed(web3).await.unwrap(),
            allowance: gp_settlement
                .vault_relayer()
                .call()
                .await
                .expect("Couldn't get vault relayer address"),
            domain_separator: DomainSeparator(
                gp_settlement
                    .domain_separator()
                    .call()
                    .await
                    .expect("Couldn't query domain separator")
                    .0,
            ),
            ethflow: CoWSwapEthFlow::deployed(web3).await.unwrap(),
            hooks: HooksTrampoline::deployed(web3).await.unwrap(),
            gp_settlement,
            cow_amm_helper,
        }
    }

    pub async fn deploy(web3: &Web3) -> Self {
        let network_id = web3
            .eth()
            .chain_id()
            .await
            .expect("get network ID failed")
            .to_string();
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

        let ethflow = deploy!(CoWSwapEthFlow(gp_settlement.address(), weth.address()));
        let hooks = deploy!(HooksTrampoline(gp_settlement.address()));

        Self {
            chain_id: network_id
                .parse()
                .expect("Couldn't parse network ID to u64"),
            balancer_vault,
            gp_settlement,
            gp_authenticator,
            uniswap_v2_factory,
            uniswap_v2_router,
            weth,
            allowance,
            domain_separator,
            ethflow,
            hooks,
            // Current helper contract only works in forked tests
            cow_amm_helper: None,
        }
    }

    pub fn default_pool_code(&self) -> H256 {
        match self.chain_id {
            100 => H256(shared::sources::uniswap_v2::HONEYSWAP_INIT),
            _ => H256(shared::sources::uniswap_v2::UNISWAP_INIT),
        }
    }
}
