use {
    crate::logic::eth,
    thiserror::Error,
    url::Url,
    web3::{Transport, Web3},
};

pub mod contracts;

/// The Ethereum blockchain.
#[derive(Debug, Clone)]
pub struct Ethereum {
    web3: Web3<web3::transports::Http>,
    chain_id: eth::ChainId,
    network_id: eth::NetworkId,
}

impl Ethereum {
    /// Access the Ethereum blockchain through an RPC API hosted at the given
    /// URL.
    pub async fn ethrpc(url: &Url) -> Result<Self, web3::Error> {
        // TODO Probably move shared::ethrpc into its own crate and reuse it here
        let web3 = Web3::new(web3::transports::Http::new(url.as_str())?);
        let chain_id = web3.eth().chain_id().await?.into();
        let network_id = web3.net().version().await?.into();
        Ok(Self {
            web3,
            chain_id,
            network_id,
        })
    }

    pub fn chain_id(&self) -> eth::ChainId {
        self.chain_id
    }

    pub fn network_id(&self) -> &eth::NetworkId {
        &self.network_id
    }

    /// Onchain smart contract bindings.
    pub fn contracts(&self) -> Contracts<'_> {
        Contracts(self)
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
        let amount = contracts::ERC20::at(&self.web3, spender.token.0)
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

    pub async fn create_access_list(&self, tx: eth::Tx) -> Result<eth::AccessList, Error> {
        // Seems like the web3 library still doesn't have a convenience method for this,
        // so the call request has to be built manually.
        let tx = web3::types::TransactionRequest {
            from: tx.from.into(),
            to: Some(tx.to.into()),
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
        if let Some(err) = json.get("error").unwrap().as_str() {
            return Err(Error::Response(err.to_owned()));
        }
        let access_list: web3::types::AccessList =
            serde_json::from_value(json.get("accessList").unwrap().to_owned()).unwrap();
        Ok(access_list.into())
    }

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
}

pub struct Contracts<'a>(&'a Ethereum);

impl Contracts<'_> {
    /// The settlement contract.
    pub fn settlement(&self) -> contracts::GPv2Settlement {
        let address = contracts::GPv2Settlement::raw_contract()
            .networks
            .get(self.0.network_id().as_str())
            .unwrap()
            .address;
        contracts::GPv2Settlement::at(&self.0.web3, address)
    }

    /// The WETH contract.
    pub fn weth(&self) -> contracts::WETH9 {
        let address = contracts::WETH9::raw_contract()
            .networks
            .get(self.0.network_id().as_str())
            .unwrap()
            .address;
        contracts::WETH9::at(&self.0.web3, address)
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("method error: {0:?}")]
    Method(#[from] ethcontract::errors::MethodError),
    #[error("web3 error: {0:?}")]
    Web3(#[from] web3::error::Error),
    #[error("web3 error returned in response: {0:?}")]
    Response(String),
}
