use {
    self::contracts::ContractAt,
    crate::{boundary, domain::eth},
    ethcontract::dyns::DynWeb3,
    gas_estimation::{nativegasestimator::NativeGasEstimator, GasPriceEstimating},
    std::{fmt, sync::Arc},
    thiserror::Error,
    web3::Transport,
};

pub mod contracts;
pub mod token;

pub use self::contracts::Contracts;

/// An Ethereum RPC connection.
pub struct Rpc {
    web3: DynWeb3,
    network: Network,
}

/// Network information for an Ethereum blockchain connection.
#[derive(Clone, Debug)]
pub struct Network {
    pub id: eth::NetworkId,
    pub chain: eth::ChainId,
}

impl Rpc {
    /// Instantiate an RPC client to an Ethereum (or Ethereum-compatible) node
    /// at the specifed URL.
    pub async fn new(url: &url::Url) -> Result<Self, Error> {
        let web3 = boundary::buffered_web3_client(url);
        let id = web3.net().version().await?.into();
        let chain = web3.eth().chain_id().await?.into();

        Ok(Self {
            web3,
            network: Network { id, chain },
        })
    }

    /// Returns the network information for the RPC connection.
    pub fn network(&self) -> &Network {
        &self.network
    }
}

/// The Ethereum blockchain.
#[derive(Clone)]
pub struct Ethereum {
    web3: DynWeb3,
    network: Network,
    contracts: Contracts,
    gas: Arc<NativeGasEstimator>,
}

impl Ethereum {
    /// Access the Ethereum blockchain through an RPC API.
    pub async fn new(rpc: Rpc, addresses: contracts::Addresses) -> Result<Self, Error> {
        let Rpc { web3, network } = rpc;
        let contracts = Contracts::new(&web3, &network.id, addresses);
        let gas = Arc::new(
            NativeGasEstimator::new(web3.transport().clone(), None)
                .await
                .map_err(Error::Gas)?,
        );

        Ok(Self {
            web3,
            network,
            contracts,
            gas,
        })
    }

    pub fn network(&self) -> &Network {
        &self.network
    }

    /// Onchain smart contract bindings.
    pub fn contracts(&self) -> &Contracts {
        &self.contracts
    }

    /// Create a contract instance at the specified address.
    pub fn contract_at<T: ContractAt>(&self, address: eth::ContractAddress) -> T {
        T::at(self, address)
    }

    /// Check if a smart contract is deployed to the given address.
    pub async fn is_contract(&self, address: eth::Address) -> Result<bool, Error> {
        let code = self.web3.eth().code(address.into(), None).await?;
        Ok(!code.0.is_empty())
    }

    /// Create access list used by a transaction.
    pub async fn create_access_list(&self, tx: eth::Tx) -> Result<eth::AccessList, Error> {
        let tx = web3::types::TransactionRequest {
            from: tx.from.into(),
            to: Some(tx.to.into()),
            gas_price: Some(eth::U256::zero()),
            value: Some(tx.value.into()),
            data: Some(tx.input.into()),
            access_list: Some(tx.access_list.into()),
            ..Default::default()
        };
        let json = self
            .web3
            .transport()
            .execute(
                "eth_createAccessList",
                vec![serde_json::to_value(&tx).unwrap()],
            )
            .await?;
        if let Some(err) = json.get("error") {
            return Err(Error::Response(err.to_owned()));
        }
        let access_list: web3::types::AccessList =
            serde_json::from_value(json.get("accessList").unwrap().to_owned()).unwrap();
        Ok(access_list.into())
    }

    /// Estimate gas used by a transaction.
    pub async fn estimate_gas(&self, tx: eth::Tx) -> Result<eth::Gas, Error> {
        self.web3
            .eth()
            .estimate_gas(
                web3::types::CallRequest {
                    from: Some(tx.from.into()),
                    to: Some(tx.to.into()),
                    gas_price: Some(eth::U256::zero()),
                    value: Some(tx.value.into()),
                    data: Some(tx.input.into()),
                    access_list: Some(tx.access_list.into()),
                    ..Default::default()
                },
                None,
            )
            .await
            .map(Into::into)
            .map_err(Into::into)
    }

    pub async fn gas_price(&self) -> Result<eth::GasPrice, Error> {
        self.gas
            .estimate()
            .await
            .map(|estimate| eth::GasPrice {
                max: eth::U256::from_f64_lossy(estimate.max_fee_per_gas).into(),
                tip: eth::U256::from_f64_lossy(estimate.max_priority_fee_per_gas).into(),
                base: eth::U256::from_f64_lossy(estimate.base_fee_per_gas).into(),
            })
            .map_err(Error::Gas)
    }

    /// Returns the current [`eth::Ether`] balance of the specified account.
    pub async fn balance(&self, address: eth::Address) -> Result<eth::Ether, Error> {
        self.web3
            .eth()
            .balance(address.into(), None)
            .await
            .map(Into::into)
            .map_err(Into::into)
    }

    /// Returns a [`token::Erc20`] for the specified address.
    pub fn erc20(&self, address: eth::TokenAddress) -> token::Erc20 {
        token::Erc20::new(self.contract_at(address.into()))
    }
}

impl fmt::Debug for Ethereum {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Ethereum")
            .field("web3", &self.web3)
            .field("network", &self.network)
            .field("contracts", &self.contracts)
            .field("gas", &"Arc<NativeGasEstimator>")
            .finish()
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("method error: {0:?}")]
    Method(#[from] ethcontract::errors::MethodError),
    #[error("web3 error: {0:?}")]
    Web3(#[from] web3::error::Error),
    #[error("gas price estimation error: {0}")]
    Gas(boundary::Error),
    #[error("web3 error returned in response: {0:?}")]
    Response(serde_json::Value),
}
