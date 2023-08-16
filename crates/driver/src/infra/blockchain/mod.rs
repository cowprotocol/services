use {
    self::contracts::ContractAt,
    crate::{boundary, domain::eth},
    ethcontract::{dyns::DynWeb3, transport::DynTransport},
    gas_estimation::{nativegasestimator::NativeGasEstimator, GasPriceEstimating},
    std::{fmt, sync::Arc},
    thiserror::Error,
    web3::{Transport, Web3},
};

pub mod contracts;

pub use self::contracts::Contracts;

/// The Ethereum blockchain.
#[derive(Clone)]
pub struct Ethereum {
    web3: DynWeb3,
    chain_id: eth::ChainId,
    network_id: eth::NetworkId,
    contracts: Contracts,
    gas: Arc<NativeGasEstimator>,
}

impl Ethereum {
    /// Access the Ethereum blockchain through an RPC API hosted at the given
    /// URL.
    pub async fn ethrpc(url: &url::Url, addresses: contracts::Addresses) -> Result<Self, Error> {
        // TODO Enable batching, reuse ethrpc? Put it in the boundary module?
        // I feel like what we have in shared::ethrpc could be simplified if we use
        // web3::transports::batch or something, but I haven't looked deep into it, just
        // a gut feeling.
        let web3 = Web3::new(DynTransport::new(web3::transports::Http::new(
            url.as_str(),
        )?));
        let chain_id = web3.eth().chain_id().await?.into();
        let network_id = web3.net().version().await?.into();
        let contracts = Contracts::new(&web3, &network_id, addresses);
        let gas = Arc::new(
            NativeGasEstimator::new(web3.transport().clone(), None)
                .await
                .map_err(Error::Gas)?,
        );

        Ok(Self {
            web3,
            chain_id,
            network_id,
            contracts,
            gas,
        })
    }

    pub fn chain_id(&self) -> eth::ChainId {
        self.chain_id
    }

    pub fn network_id(&self) -> &eth::NetworkId {
        &self.network_id
    }

    /// Onchain smart contract bindings.
    pub fn contracts(&self) -> &Contracts {
        &self.contracts
    }

    /// Create a contract instance at the specified address.
    pub fn contract_at<T: ContractAt>(&self, address: eth::ContractAddress) -> T {
        T::at(self, address)
    }

    /// Fetch the ERC20 allowance for the spender. See the allowance method in
    /// EIP-20.
    ///
    /// https://eips.ethereum.org/EIPS/eip-20#methods
    pub async fn allowance(
        &self,
        owner: eth::Address,
        spender: eth::allowance::Spender,
    ) -> Result<eth::allowance::Existing, Error> {
        let amount = contracts::ERC20::at(&self.web3, spender.token.into())
            .allowance(owner.0, spender.address.0)
            .call()
            .await?;
        Ok(eth::Allowance { spender, amount }.into())
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

    pub async fn decimals(&self, address: eth::TokenAddress) -> Result<u8, Error> {
        let erc20 = self.contract_at::<contracts::ERC20>(address.0);
        erc20.methods().decimals().call().await.map_err(Into::into)
    }

    pub async fn symbol(&self, address: eth::TokenAddress) -> Result<String, Error> {
        let erc20 = self.contract_at::<contracts::ERC20>(address.0);
        erc20.methods().symbol().call().await.map_err(Into::into)
    }
}

impl fmt::Debug for Ethereum {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Ethereum")
            .field("web3", &self.web3)
            .field("chain_id", &self.chain_id)
            .field("network_id", &self.network_id)
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
