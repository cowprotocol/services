use {
    alloy::{
        primitives::{Address, B256, U256},
        providers::Provider,
    },
    contracts::alloy::{
        BalancerV2Authorizer,
        BalancerV2Vault,
        CoWSwapEthFlow,
        FlashLoanRouter,
        GPv2AllowListAuthentication,
        GPv2Settlement,
        HooksTrampoline,
        UniswapV2Factory,
        UniswapV2Router02,
        WETH9,
        support::{Balances, Signatures},
    },
    ethrpc::alloy::CallBuilderExt,
    model::DomainSeparator,
    shared::ethrpc::Web3,
};

#[derive(Default)]
pub struct DeployedContracts {
    pub balances: Option<Address>,
    pub signatures: Option<Address>,
}

pub struct Contracts {
    pub chain_id: u64,
    pub balancer_vault: BalancerV2Vault::Instance,
    pub gp_settlement: GPv2Settlement::Instance,
    pub signatures: Signatures::Instance,
    pub gp_authenticator: GPv2AllowListAuthentication::Instance,
    pub balances: Balances::Instance,
    pub uniswap_v2_factory: UniswapV2Factory::Instance,
    pub uniswap_v2_router: UniswapV2Router02::Instance,
    pub weth: WETH9::Instance,
    pub allowance: Address,
    pub domain_separator: DomainSeparator,
    pub ethflows: Vec<CoWSwapEthFlow::Instance>,
    pub hooks: HooksTrampoline::Instance,
    pub flashloan_router: Option<FlashLoanRouter::Instance>,
}

impl Contracts {
    pub async fn deployed_with(web3: &Web3, deployed: DeployedContracts) -> Self {
        let network_id = web3
            .alloy
            .get_chain_id()
            .await
            .expect("get network ID failed")
            .to_string();
        tracing::info!("connected to forked test network {}", network_id);

        let gp_settlement = GPv2Settlement::Instance::deployed(&web3.alloy)
            .await
            .unwrap();
        let balances = match deployed.balances {
            Some(address) => Balances::Instance::new(address, web3.alloy.clone()),
            None => Balances::Instance::deployed(&web3.alloy)
                .await
                .expect("failed to find balances contract"),
        };
        let signatures = match deployed.signatures {
            Some(address) => Signatures::Instance::new(address, web3.alloy.clone()),
            None => Signatures::Instance::deployed(&web3.alloy)
                .await
                .expect("failed to find signatures contract"),
        };

        let flashloan_router = FlashLoanRouter::Instance::deployed(&web3.alloy).await.ok();

        Self {
            chain_id: network_id
                .parse()
                .expect("Couldn't parse network ID to u64"),
            balancer_vault: BalancerV2Vault::Instance::deployed(&web3.alloy)
                .await
                .unwrap(),
            gp_authenticator: GPv2AllowListAuthentication::Instance::deployed(&web3.alloy)
                .await
                .unwrap(),
            uniswap_v2_factory: UniswapV2Factory::Instance::deployed(&web3.alloy)
                .await
                .unwrap(),
            uniswap_v2_router: UniswapV2Router02::Instance::deployed(&web3.alloy)
                .await
                .unwrap(),
            weth: WETH9::Instance::deployed(&web3.alloy).await.unwrap(),
            allowance: gp_settlement
                .vaultRelayer()
                .call()
                .await
                .expect("Couldn't get vault relayer address"),
            domain_separator: DomainSeparator(
                gp_settlement
                    .domainSeparator()
                    .call()
                    .await
                    .expect("Couldn't query domain separator")
                    .0,
            ),
            ethflows: vec![
                CoWSwapEthFlow::Instance::deployed(&web3.alloy)
                    .await
                    .unwrap(),
            ],
            hooks: HooksTrampoline::Instance::deployed(&web3.alloy)
                .await
                .unwrap(),
            gp_settlement,
            balances,
            signatures,
            flashloan_router,
        }
    }

    pub async fn deploy(web3: &Web3) -> Self {
        let network_id = web3
            .alloy
            .get_chain_id()
            .await
            .expect("get network ID failed")
            .to_string();
        tracing::info!("connected to test network {}", network_id);

        let accounts = web3
            .alloy
            .get_accounts()
            .await
            .expect("get accounts failed");
        let admin = accounts[0];

        let weth = WETH9::Instance::deploy(web3.alloy.clone()).await.unwrap();

        let balancer_authorizer = BalancerV2Authorizer::Instance::deploy(web3.alloy.clone(), admin)
            .await
            .unwrap();
        let balancer_vault = BalancerV2Vault::Instance::deploy(
            web3.alloy.clone(),
            *balancer_authorizer.address(),
            *weth.address(),
            U256::ZERO,
            U256::ZERO,
        )
        .await
        .unwrap();

        let uniswap_v2_factory = UniswapV2Factory::Instance::deploy(web3.alloy.clone(), admin)
            .await
            .unwrap();
        let uniswap_v2_router = UniswapV2Router02::Instance::deploy(
            web3.alloy.clone(),
            *uniswap_v2_factory.address(),
            *weth.address(),
        )
        .await
        .unwrap();

        let gp_authenticator = GPv2AllowListAuthentication::Instance::deploy(web3.alloy.clone())
            .await
            .unwrap();
        gp_authenticator
            .initializeManager(admin)
            .send_and_watch()
            .await
            .expect("failed to initialize manager");
        let gp_settlement = GPv2Settlement::Instance::deploy(
            web3.alloy.clone(),
            *gp_authenticator.address(),
            *balancer_vault.address(),
        )
        .await
        .unwrap();
        let balances = Balances::Instance::deploy(web3.alloy.clone())
            .await
            .unwrap();
        let signatures = Signatures::Instance::deploy(web3.alloy.clone())
            .await
            .unwrap();

        contracts::vault::grant_required_roles(
            &balancer_authorizer,
            *balancer_vault.address(),
            gp_settlement
                .vaultRelayer()
                .call()
                .await
                .expect("failed to retrieve Vault relayer contract address"),
        )
        .await
        .expect("failed to authorize Vault relayer");

        let allowance = gp_settlement
            .vaultRelayer()
            .call()
            .await
            .expect("Couldn't get vault relayer address");
        let domain_separator = DomainSeparator(
            gp_settlement
                .domainSeparator()
                .call()
                .await
                .expect("Couldn't query domain separator")
                .0,
        );

        let ethflow = CoWSwapEthFlow::Instance::deploy(
            web3.alloy.clone(),
            *gp_settlement.address(),
            *weth.address(),
        )
        .await
        .unwrap();
        let ethflow_secondary = CoWSwapEthFlow::Instance::deploy(
            web3.alloy.clone(),
            *gp_settlement.address(),
            *weth.address(),
        )
        .await
        .unwrap();
        let hooks = HooksTrampoline::Instance::deploy(web3.alloy.clone(), *gp_settlement.address())
            .await
            .unwrap();
        let flashloan_router =
            FlashLoanRouter::Instance::deploy(web3.alloy.clone(), *gp_settlement.address())
                .await
                .unwrap();

        Self {
            chain_id: network_id
                .parse()
                .expect("Couldn't parse network ID to u64"),
            balancer_vault,
            gp_settlement,
            gp_authenticator,
            balances,
            signatures,
            uniswap_v2_factory,
            uniswap_v2_router,
            weth,
            allowance,
            domain_separator,
            ethflows: vec![ethflow, ethflow_secondary],
            hooks,
            // Current helper contract only works in forked tests
            flashloan_router: Some(flashloan_router),
        }
    }

    pub fn default_pool_code(&self) -> B256 {
        match self.chain_id {
            100 => B256::new(shared::sources::uniswap_v2::HONEYSWAP_INIT),
            _ => B256::new(shared::sources::uniswap_v2::UNISWAP_INIT),
        }
    }
}
