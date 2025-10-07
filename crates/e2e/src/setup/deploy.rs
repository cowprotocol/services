use {
    crate::deploy,
    contracts::{
        BalancerV2Authorizer,
        BalancerV2Vault,
        CowAmmLegacyHelper,
        FlashLoanRouter,
        GPv2AllowListAuthentication,
        GPv2Settlement,
        WETH9,
        alloy::{
            CoWSwapEthFlow,
            HooksTrampoline,
            InstanceExt,
            UniswapV2Factory,
            UniswapV2Router02,
        },
        support::{Balances, Signatures},
    },
    ethcontract::{Address, H256, U256, errors::DeployError},
    ethrpc::alloy::conversions::IntoAlloy,
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
    pub balancer_vault: BalancerV2Vault,
    pub gp_settlement: GPv2Settlement,
    pub signatures: Signatures,
    pub gp_authenticator: GPv2AllowListAuthentication,
    pub balances: Balances,
    pub uniswap_v2_factory: UniswapV2Factory::Instance,
    pub uniswap_v2_router: UniswapV2Router02::Instance,
    pub weth: WETH9,
    pub allowance: Address,
    pub domain_separator: DomainSeparator,
    pub ethflows: Vec<CoWSwapEthFlow::Instance>,
    pub hooks: HooksTrampoline::Instance,
    pub cow_amm_helper: Option<CowAmmLegacyHelper>,
    pub flashloan_router: Option<FlashLoanRouter>,
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
        let cow_amm_helper = match contracts::CowAmmLegacyHelper::deployed(web3).await {
            Err(DeployError::NotFound(_)) => None,
            Err(err) => panic!("failed to find deployed contract: {err:?}"),
            Ok(contract) => Some(contract),
        };

        let balances = match deployed.balances {
            Some(address) => Balances::at(web3, address),
            None => Balances::deployed(web3)
                .await
                .expect("failed to find balances contract"),
        };
        let signatures = match deployed.signatures {
            Some(address) => Signatures::at(web3, address),
            None => Signatures::deployed(web3)
                .await
                .expect("failed to find signatures contract"),
        };

        let flashloan_router = FlashLoanRouter::deployed(web3).await.ok();

        Self {
            chain_id: network_id
                .parse()
                .expect("Couldn't parse network ID to u64"),
            balancer_vault: BalancerV2Vault::deployed(web3).await.unwrap(),
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
            cow_amm_helper,
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

        let balancer_authorizer = deploy!(web3, BalancerV2Authorizer(admin));
        let balancer_vault = deploy!(
            web3,
            BalancerV2Vault(
                balancer_authorizer.address(),
                weth.address(),
                U256::from(0),
                U256::from(0),
            )
        );

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
            GPv2Settlement(gp_authenticator.address(), balancer_vault.address(),)
        );
        let balances = deploy!(web3, Balances());
        let signatures = deploy!(web3, Signatures());

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
        let flashloan_router = deploy!(web3, FlashLoanRouter(gp_settlement.address()));

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
            cow_amm_helper: None,
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
