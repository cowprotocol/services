use {
    alloy::{
        primitives::{Address, B256, U256, keccak256},
        providers::Provider,
        sol_types::SolCall,
    },
    contracts::{
        BalancerV2Authorizer,
        BalancerV2Vault,
        CoWSwapEthFlow,
        FlashLoanRouter,
        GPv2AllowListAuthentication,
        GPv2Settlement,
        HoneyswapRouter,
        HooksTrampoline,
        UniswapV2Factory,
        UniswapV2Router02,
        WETH9,
        support::{Balances, Signatures},
    },
    ethrpc::alloy::CallBuilderExt,
    model::DomainSeparator,
    shared::web3::Web3,
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
            .provider
            .get_chain_id()
            .await
            .expect("get network ID failed")
            .to_string();
        tracing::info!("connected to forked test network {}", network_id);

        let gp_settlement = GPv2Settlement::Instance::deployed(&web3.provider)
            .await
            .unwrap();
        let balances = match deployed.balances {
            Some(address) => Balances::Instance::new(address, web3.provider.clone()),
            None => Balances::Instance::deployed(&web3.provider)
                .await
                .expect("failed to find balances contract"),
        };
        let signatures = match deployed.signatures {
            Some(address) => Signatures::Instance::new(address, web3.provider.clone()),
            None => Signatures::Instance::deployed(&web3.provider)
                .await
                .expect("failed to find signatures contract"),
        };

        let flashloan_router = FlashLoanRouter::Instance::deployed(&web3.provider)
            .await
            .ok();

        Self {
            chain_id: network_id
                .parse()
                .expect("Couldn't parse network ID to u64"),
            balancer_vault: BalancerV2Vault::Instance::deployed(&web3.provider)
                .await
                .unwrap(),
            gp_authenticator: GPv2AllowListAuthentication::Instance::deployed(&web3.provider)
                .await
                .unwrap(),
            uniswap_v2_factory: UniswapV2Factory::Instance::deployed(&web3.provider)
                .await
                .unwrap(),
            uniswap_v2_router: uniswap_v2_router_for_chain(web3).await,
            weth: WETH9::Instance::deployed(&web3.provider).await.unwrap(),
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
                CoWSwapEthFlow::Instance::deployed(&web3.provider)
                    .await
                    .unwrap(),
            ],
            hooks: HooksTrampoline::Instance::deployed(&web3.provider)
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
            .provider
            .get_chain_id()
            .await
            .expect("get network ID failed")
            .to_string();
        tracing::info!("connected to test network {}", network_id);

        let accounts = web3
            .provider
            .get_accounts()
            .await
            .expect("get accounts failed");
        let admin = accounts[0];

        let weth = WETH9::Instance::deploy(web3.provider.clone())
            .await
            .unwrap();

        let balancer_authorizer =
            BalancerV2Authorizer::Instance::deploy(web3.provider.clone(), admin)
                .await
                .unwrap();
        let balancer_vault = BalancerV2Vault::Instance::deploy(
            web3.provider.clone(),
            *balancer_authorizer.address(),
            *weth.address(),
            U256::ZERO,
            U256::ZERO,
        )
        .await
        .unwrap();

        let uniswap_v2_factory = UniswapV2Factory::Instance::deploy(web3.provider.clone(), admin)
            .await
            .unwrap();
        let uniswap_v2_router = UniswapV2Router02::Instance::deploy(
            web3.provider.clone(),
            *uniswap_v2_factory.address(),
            *weth.address(),
        )
        .await
        .unwrap();

        let gp_authenticator = GPv2AllowListAuthentication::Instance::deploy(web3.provider.clone())
            .await
            .unwrap();
        gp_authenticator
            .initializeManager(admin)
            .send_and_watch()
            .await
            .expect("failed to initialize manager");
        let gp_settlement = GPv2Settlement::Instance::deploy(
            web3.provider.clone(),
            *gp_authenticator.address(),
            *balancer_vault.address(),
        )
        .await
        .unwrap();
        let balances = Balances::Instance::deploy(web3.provider.clone())
            .await
            .unwrap();
        let signatures = Signatures::Instance::deploy(web3.provider.clone())
            .await
            .unwrap();

        grant_required_roles(
            &balancer_authorizer,
            *balancer_vault.address(),
            gp_settlement
                .vaultRelayer()
                .call()
                .await
                .expect("failed to retrieve Vault relayer contract address"),
        )
        .await;

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
            web3.provider.clone(),
            *gp_settlement.address(),
            *weth.address(),
        )
        .await
        .unwrap();
        let ethflow_secondary = CoWSwapEthFlow::Instance::deploy(
            web3.provider.clone(),
            *gp_settlement.address(),
            *weth.address(),
        )
        .await
        .unwrap();
        let hooks =
            HooksTrampoline::Instance::deploy(web3.provider.clone(), *gp_settlement.address())
                .await
                .unwrap();
        let flashloan_router =
            FlashLoanRouter::Instance::deploy(web3.provider.clone(), *gp_settlement.address())
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
            100 => B256::new(liquidity_sources::uniswap_v2::HONEYSWAP_INIT),
            _ => B256::new(liquidity_sources::uniswap_v2::UNISWAP_INIT),
        }
    }
}

/// Resolve a router with the canonical UniswapV2 ABI for the current chain.
async fn uniswap_v2_router_for_chain(web3: &Web3) -> UniswapV2Router02::Instance {
    const GNOSIS_CHAIN_ID: u64 = 100;
    let chain_id = web3
        .provider
        .get_chain_id()
        .await
        .expect("get chain id failed");
    let address = match chain_id {
        // Gnosis: no official Uniswap V2 deployment; use Honeyswap's router,
        // which is what xdai's `honeyswap` preset binds in the driver.
        GNOSIS_CHAIN_ID => HoneyswapRouter::deployment_address(&chain_id)
            .expect("HoneyswapRouter deployment address registered for Gnosis"),
        _ => {
            return UniswapV2Router02::Instance::deployed(&web3.provider)
                .await
                .expect("UniswapV2Router02 deployment address registered for this chain");
        }
    };
    UniswapV2Router02::Instance::new(address, web3.provider.clone())
}

fn role_id<Call: SolCall>(vault: Address) -> B256 {
    let mut data = [0u8; 36];
    data[12..32].copy_from_slice(vault.as_slice());
    data[32..36].copy_from_slice(&Call::SELECTOR);
    keccak256(data)
}

async fn grant_required_roles(
    authorizer: &BalancerV2Authorizer::Instance,
    vault: Address,
    vault_relayer: Address,
) {
    use contracts::BalancerV2Vault::BalancerV2Vault::{batchSwapCall, manageUserBalanceCall};

    authorizer
        .grantRoles(
            vec![
                role_id::<manageUserBalanceCall>(vault).0.into(),
                role_id::<batchSwapCall>(vault).0.into(),
            ],
            vault_relayer,
        )
        .send()
        .await
        .unwrap()
        .watch()
        .await
        .unwrap();
}
