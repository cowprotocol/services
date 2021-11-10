use crate::{
    BalancerV2Authorizer, BalancerV2Vault, GPv2AllowListAuthentication, GPv2Settlement,
    UniswapV2Factory, UniswapV2Router02, WETH9,
};
use anyhow::{Context, Result};
use ethcontract::{dyns::DynTransport, Address, Web3, U256};
use std::path::{Path, PathBuf};

pub struct Contracts {
    pub balancer_vault: BalancerV2Vault,
    pub gp_settlement: GPv2Settlement,
    pub uniswap_factory: UniswapV2Factory,
    pub uniswap_router: UniswapV2Router02,
    pub weth: WETH9,
}

/// An existing local testnet deployment.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct Deployment {
    pub balancer_vault_address: Address,
    pub gp_settlement_address: Address,
    pub uniswap_factory_address: Address,
    pub uniswap_router_address: Address,
    pub weth_address: Address,
}

impl Deployment {
    pub fn path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("target")
            .join("local_deployment")
    }

    pub fn read() -> Result<Self> {
        let file = std::fs::read_to_string(Self::path()).context("read file")?;
        serde_json::from_str(&file).context("deserialize")
    }

    pub fn write(&self) -> Result<()> {
        let serialized = serde_json::to_string_pretty(&self).unwrap();
        std::fs::write(Self::path(), serialized).context("write file")
    }
}

impl Contracts {
    /// Deploy contracts to a local testnet on which the services can be run.
    pub async fn deploy(web3: &Web3<DynTransport>) -> Result<Self> {
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

        crate::vault::grant_required_roles(
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

        Ok(Contracts {
            balancer_vault,
            gp_settlement,
            uniswap_factory,
            uniswap_router,
            weth,
        })
    }

    pub fn from_deployment(deployment: Deployment, web3: &Web3<DynTransport>) -> Self {
        Self {
            balancer_vault: BalancerV2Vault::with_deployment_info(
                web3,
                deployment.balancer_vault_address,
                None,
            ),
            gp_settlement: GPv2Settlement::with_deployment_info(
                web3,
                deployment.gp_settlement_address,
                None,
            ),
            uniswap_factory: UniswapV2Factory::with_deployment_info(
                web3,
                deployment.uniswap_factory_address,
                None,
            ),
            uniswap_router: UniswapV2Router02::with_deployment_info(
                web3,
                deployment.uniswap_router_address,
                None,
            ),
            weth: WETH9::with_deployment_info(web3, deployment.weth_address, None),
        }
    }

    pub async fn from_web3(web3: &Web3<DynTransport>) -> Result<Self> {
        Ok(Self {
            balancer_vault: BalancerV2Vault::deployed(web3).await?,
            gp_settlement: GPv2Settlement::deployed(web3).await?,
            uniswap_factory: UniswapV2Factory::deployed(web3).await?,
            uniswap_router: UniswapV2Router02::deployed(web3).await?,
            weth: WETH9::deployed(web3).await?,
        })
    }

    pub fn deployment(&self) -> Deployment {
        Deployment {
            balancer_vault_address: self.balancer_vault.address(),
            gp_settlement_address: self.gp_settlement.address(),
            uniswap_factory_address: self.uniswap_factory.address(),
            uniswap_router_address: self.uniswap_router.address(),
            weth_address: self.weth.address(),
        }
    }
}
