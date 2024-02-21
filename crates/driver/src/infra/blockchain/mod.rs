use {
    self::contracts::ContractAt,
    crate::{boundary, domain::eth},
    ethcontract::dyns::DynWeb3,
    ethrpc::current_block::CurrentBlockStream,
    std::{fmt, sync::Arc},
    thiserror::Error,
    web3::Transport,
};

pub mod contracts;
pub mod gas;
pub mod token;

use {ethcontract::errors::ExecutionError, gas_estimation::GasPriceEstimating};

pub use self::{contracts::Contracts, gas::GasPriceEstimator};

/// An Ethereum RPC connection.
pub struct Rpc {
    web3: DynWeb3,
    chain: eth::ChainId,
}

impl Rpc {
    /// Instantiate an RPC client to an Ethereum (or Ethereum-compatible) node
    /// at the specifed URL.
    pub async fn new(url: &url::Url) -> Result<Self, Error> {
        let web3 = boundary::buffered_web3_client(url);
        let chain = web3.eth().chain_id().await?.into();

        Ok(Self { web3, chain })
    }

    /// Returns the chain id for the RPC connection.
    pub fn chain(&self) -> eth::ChainId {
        self.chain
    }

    /// Returns a reference to the underlying web3 client.
    pub fn web3(&self) -> &DynWeb3 {
        &self.web3
    }
}

/// The Ethereum blockchain.
#[derive(Clone)]
pub struct Ethereum {
    web3: DynWeb3,
    inner: Arc<Inner>,
}

struct Inner {
    chain: eth::ChainId,
    contracts: Contracts,
    gas: Arc<GasPriceEstimator>,
    current_block: CurrentBlockStream,
}

impl Ethereum {
    /// Access the Ethereum blockchain through an RPC API.
    ///
    /// # Panics
    ///
    /// Since this type is essential for the program this method will panic on
    /// any initialization error.
    pub async fn new(
        rpc: Rpc,
        addresses: contracts::Addresses,
        gas: Arc<GasPriceEstimator>,
    ) -> Self {
        let Rpc { web3, chain } = rpc;
        let contracts = Contracts::new(&web3, chain, addresses)
            .await
            .expect("could not initialize important smart contracts");

        Self {
            inner: Arc::new(Inner {
                current_block: ethrpc::current_block::current_block_stream(
                    Arc::new(web3.clone()),
                    std::time::Duration::from_millis(500),
                )
                .await
                .expect("couldn't initialize current block stream"),
                chain,
                contracts,
                gas,
            }),
            web3,
        }
    }

    pub fn network(&self) -> eth::ChainId {
        self.inner.chain
    }

    /// Clones self and returns an instance that captures metrics extended with
    /// the provided label.
    pub fn with_metric_label(&self, label: String) -> Self {
        Self {
            web3: ethrpc::instrumented::instrument_with_label(&self.web3, label),
            ..self.clone()
        }
    }

    /// Onchain smart contract bindings.
    pub fn contracts(&self) -> &Contracts {
        &self.inner.contracts
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

    /// Returns a type that monitors the block chain to inform about the current
    /// block.
    pub fn current_block(&self) -> &CurrentBlockStream {
        &self.inner.current_block
    }

    /// Create access list used by a transaction.
    pub async fn create_access_list(&self, tx: eth::Tx) -> Result<eth::AccessList, Error> {
        const MAX_BLOCK_SIZE: u64 = 30_000_000;

        let tx = web3::types::TransactionRequest {
            from: tx.from.into(),
            to: Some(tx.to.into()),
            gas_price: Some(eth::U256::zero()),
            value: Some(tx.value.into()),
            data: Some(tx.input.into()),
            access_list: Some(tx.access_list.into()),
            // Specifically set high gas because some nodes don't pick a sensible value if omitted.
            // And since we are only interested in access lists a very high value is fine.
            gas: Some(MAX_BLOCK_SIZE.into()),
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
            return Err(Error::AccessList(err.to_owned()));
        }
        let access_list: web3::types::AccessList =
            serde_json::from_value(json.get("accessList").unwrap().to_owned()).unwrap();
        Ok(access_list.into())
    }

    pub fn boundary_gas_estimator(&self) -> Arc<dyn GasPriceEstimating> {
        self.inner.gas.gas.clone()
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
        self.inner.gas.estimate().await
    }

    pub fn gas_limit(&self) -> eth::Gas {
        self.inner.current_block.borrow().gas_limit.into()
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
        token::Erc20::new(self, address)
    }

    /// Returns the transaction's on-chain inclusion status.
    pub async fn transaction_status(&self, tx_hash: &eth::TxId) -> Result<eth::TxStatus, Error> {
        self.web3
            .eth()
            .transaction_receipt(tx_hash.0)
            .await
            .map(|result| match result {
                Some(web3::types::TransactionReceipt {
                    status: Some(status),
                    ..
                }) => {
                    if status.is_zero() {
                        eth::TxStatus::Reverted
                    } else {
                        eth::TxStatus::Executed
                    }
                }
                _ => eth::TxStatus::Pending,
            })
            .map_err(Into::into)
    }
}

impl fmt::Debug for Ethereum {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Ethereum")
            .field("web3", &self.web3)
            .field("chain", &self.inner.chain)
            .field("contracts", &self.inner.contracts)
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
    GasPrice(boundary::Error),
    #[error("access list estimation error: {0:?}")]
    AccessList(serde_json::Value),
}

impl Error {
    /// Returns whether the error indicates that the original transaction
    /// reverted.
    pub fn is_revert(&self) -> bool {
        // This behavior is node dependent
        match self {
            Error::Method(error) => matches!(error.inner, ExecutionError::Revert(_)),
            Error::Web3(inner) => {
                let error = ExecutionError::from(inner.clone());
                matches!(error, ExecutionError::Revert(_))
            }
            Error::GasPrice(_) => false,
            Error::AccessList(_) => true,
        }
    }
}

impl From<contracts::Error> for Error {
    fn from(err: contracts::Error) -> Self {
        match err {
            contracts::Error::Method(err) => Self::Method(err),
        }
    }
}
