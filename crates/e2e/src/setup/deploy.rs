use {
    crate::deploy,
    contracts::{
        GPv2AllowListAuthentication,
        GPv2Settlement,
        WETH9,
        alloy::{
            BalancerV2Authorizer,
            BalancerV2Vault,
            CoWSwapEthFlow,
            FlashLoanRouter,
            HooksTrampoline,
            InstanceExt,
            UniswapV2Factory,
            UniswapV2Router02,
            support::{Balances, Signatures},
        },
    },
    ethcontract::{Address, H256},
    ethrpc::alloy::conversions::{IntoAlloy, IntoLegacy},
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
    pub gp_settlement: GPv2Settlement,
    pub signatures: Signatures::Instance,
    pub gp_authenticator: GPv2AllowListAuthentication,
    pub balances: Balances::Instance,
    pub uniswap_v2_factory: UniswapV2Factory::Instance,
    pub uniswap_v2_router: UniswapV2Router02::Instance,
    pub weth: WETH9,
    pub allowance: Address,
    pub domain_separator: DomainSeparator,
    pub ethflows: Vec<CoWSwapEthFlow::Instance>,
    pub hooks: HooksTrampoline::Instance,
    pub flashloan_router: Option<FlashLoanRouter::Instance>,
}

impl Contracts {
    pub async fn deployed_with(web3: &Web3, deployed: DeployedContracts) -> Self {
        let network_id = web3
            .eth()
            .chain_id()
            .await
            .expect("get network ID failed")
            .to_string();
        tracing::info!("connected to forked test network {}", network_id);

        let gp_settlement = GPv2Settlement::deployed(web3).await.unwrap();
        let balances = match deployed.balances {
            Some(address) => Balances::Instance::new(address.into_alloy(), web3.alloy.clone()),
            None => Balances::Instance::deployed(&web3.alloy)
                .await
                .expect("failed to find balances contract"),
        };
        let signatures = match deployed.signatures {
            Some(address) => Signatures::Instance::new(address.into_alloy(), web3.alloy.clone()),
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
            gp_authenticator: GPv2AllowListAuthentication::deployed(web3).await.unwrap(),
            uniswap_v2_factory: UniswapV2Factory::Instance::deployed(&web3.alloy)
                .await
                .unwrap(),
            uniswap_v2_router: UniswapV2Router02::Instance::deployed(&web3.alloy)
                .await
                .unwrap(),
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
            .eth()
            .chain_id()
            .await
            .expect("get network ID failed")
            .to_string();
        tracing::info!("connected to test network {}", network_id);

        let accounts: Vec<Address> = web3.eth().accounts().await.expect("get accounts failed");
        let admin = accounts[0];

        let weth = deploy!(web3, WETH9());

        let balancer_authorizer =
            BalancerV2Authorizer::Instance::deploy(web3.alloy.clone(), admin.into_alloy())
                .await
                .unwrap();
        let balancer_vault = BalancerV2Vault::Instance::deploy(
            web3.alloy.clone(),
            *balancer_authorizer.address(),
            weth.address().into_alloy(),
            alloy::primitives::U256::ZERO,
            alloy::primitives::U256::ZERO,
        )
        .await
        .unwrap();

        let uniswap_v2_factory =
            UniswapV2Factory::Instance::deploy(web3.alloy.clone(), accounts[0].into_alloy())
                .await
                .unwrap();
        let uniswap_v2_router = UniswapV2Router02::Instance::deploy(
            web3.alloy.clone(),
            *uniswap_v2_factory.address(),
            weth.address().into_alloy(),
        )
        .await
        .unwrap();

        let gp_authenticator = deploy!(web3, GPv2AllowListAuthentication);
        gp_authenticator
            .initialize_manager(admin)
            .send()
            .await
            .expect("failed to initialize manager");
        let gp_settlement = deploy!(
            web3,
            GPv2Settlement(
                gp_authenticator.address(),
                balancer_vault.address().into_legacy(),
            )
        );
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
                .vault_relayer()
                .call()
                .await
                .expect("failed to retrieve Vault relayer contract address")
                .into_alloy(),
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

        let ethflow = CoWSwapEthFlow::Instance::deploy(
            web3.alloy.clone(),
            gp_settlement.address().into_alloy(),
            weth.address().into_alloy(),
        )
        .await
        .unwrap();
        let ethflow_secondary = CoWSwapEthFlow::Instance::deploy(
            web3.alloy.clone(),
            gp_settlement.address().into_alloy(),
            weth.address().into_alloy(),
        )
        .await
        .unwrap();
        let hooks = HooksTrampoline::Instance::deploy(
            web3.alloy.clone(),
            gp_settlement.address().into_alloy(),
        )
        .await
        .unwrap();
        let flashloan_router = FlashLoanRouter::Instance::deploy(
            web3.alloy.clone(),
            gp_settlement.address().into_alloy(),
        )
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

    pub fn default_pool_code(&self) -> H256 {
        match self.chain_id {
            100 => H256(shared::sources::uniswap_v2::HONEYSWAP_INIT),
            _ => H256(shared::sources::uniswap_v2::UNISWAP_INIT),
        }
    }
}
