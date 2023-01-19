use {
    self::contracts::ContractAt,
    crate::domain::eth,
    thiserror::Error,
    web3::{Transport, Web3},
};

pub mod contracts;

use ethcontract::{dyns::DynWeb3, transport::DynTransport};

pub use self::contracts::Contracts;

/// The Ethereum blockchain.
#[derive(Debug, Clone)]
pub struct Ethereum {
    web3: DynWeb3,
    chain_id: eth::ChainId,
    network_id: eth::NetworkId,
    contracts: Contracts,
}

impl Ethereum {
    /// Access the Ethereum blockchain through an RPC API hosted at the given
    /// URL.
    pub async fn ethrpc(
        url: &url::Url,
        addresses: contracts::Addresses,
    ) -> Result<Self, web3::Error> {
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
        Ok(Self {
            web3,
            chain_id,
            network_id,
            contracts,
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
        T::at(&self.web3, address)
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
        let tx = Self::into_request(tx);
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

    fn into_request(tx: eth::Tx) -> web3::types::TransactionRequest {
        web3::types::TransactionRequest {
            from: tx.from.into(),
            to: Some(tx.to.into()),
            value: Some(tx.value.into()),
            data: Some(tx.input.into()),
            access_list: Some(tx.access_list.into()),
            ..Default::default()
        }
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("method error: {0:?}")]
    Method(#[from] ethcontract::errors::MethodError),
    #[error("web3 error: {0:?}")]
    Web3(#[from] web3::error::Error),
    #[error("web3 error returned in response: {0:?}")]
    Response(serde_json::Value),
}
