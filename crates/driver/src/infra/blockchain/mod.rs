use {
    crate::{boundary, domain::eth},
    alloy::providers::Provider,
    chain::Chain,
    ethcontract::{U256, errors::ExecutionError},
    ethrpc::{Web3, alloy::conversions::IntoLegacy, block_stream::CurrentBlockWatcher},
    shared::{
        account_balances::{BalanceSimulator, SimulationError},
        price_estimation::trade_verifier::balance_overrides::{
            BalanceOverrides,
            BalanceOverriding,
        },
    },
    std::{fmt, sync::Arc, time::Duration},
    thiserror::Error,
    url::Url,
    web3::{Transport, types::CallRequest},
};

pub mod contracts;
pub mod gas;
pub mod token;
pub use self::{contracts::Contracts, gas::GasPriceEstimator};

/// An Ethereum RPC connection.
pub struct Rpc {
    web3: Web3,
    chain: Chain,
    args: RpcArgs,
}

pub struct RpcArgs {
    pub url: Url,
    pub max_batch_size: usize,
    pub max_concurrent_requests: usize,
}

impl Rpc {
    /// Instantiate an RPC client to an Ethereum (or Ethereum-compatible) node
    /// at the specifed URL.
    pub async fn try_new(args: RpcArgs) -> Result<Self, RpcError> {
        let web3 = boundary::buffered_web3_client(
            &args.url,
            args.max_batch_size,
            args.max_concurrent_requests,
        );
        let chain = Chain::try_from(web3.alloy.get_chain_id().await?)?;

        Ok(Self { web3, chain, args })
    }

    /// Returns the chain for the RPC connection.
    pub fn chain(&self) -> Chain {
        self.chain
    }

    /// Returns a reference to the underlying web3 client.
    pub fn web3(&self) -> &Web3 {
        &self.web3
    }
}

#[derive(Debug, Error)]
pub enum RpcError {
    #[error("web3 error: {0:?}")]
    Web3(#[from] web3::error::Error),
    #[error("alloy transport error: {0:?}")]
    Alloy(#[from] alloy::transports::TransportError),
    #[error("unsupported chain")]
    UnsupportedChain(#[from] chain::ChainIdNotSupported),
}

/// The Ethereum blockchain.
#[derive(Clone)]
pub struct Ethereum {
    web3: Web3,
    inner: Arc<Inner>,
}

struct Inner {
    chain: Chain,
    contracts: Contracts,
    gas: Arc<GasPriceEstimator>,
    current_block: CurrentBlockWatcher,
    balance_simulator: BalanceSimulator,
    balance_overrider: Arc<dyn BalanceOverriding>,
    tx_gas_limit: U256,
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
        tx_gas_limit: U256,
    ) -> Self {
        let Rpc { web3, chain, args } = rpc;

        let current_block_stream = ethrpc::block_stream::current_block_stream(
            args.url.clone(),
            std::time::Duration::from_millis(500),
        )
        .await
        .expect("couldn't initialize current block stream");

        let contracts = Contracts::new(&web3, chain, addresses)
            .await
            .expect("could not initialize important smart contracts");
        let balance_overrider = Arc::new(BalanceOverrides::new(web3.clone()));
        let balance_simulator = BalanceSimulator::new(
            contracts.settlement().clone(),
            contracts.balance_helper().clone(),
            contracts.vault_relayer().0,
            Some(contracts.vault().address().into_legacy()),
            balance_overrider.clone(),
        );

        Self {
            inner: Arc::new(Inner {
                current_block: current_block_stream,
                chain,
                contracts,
                gas,
                balance_simulator,
                balance_overrider,
                tx_gas_limit,
            }),
            web3,
        }
    }

    pub fn chain(&self) -> Chain {
        self.inner.chain
    }

    pub fn balance_simulator(&self) -> &BalanceSimulator {
        &self.inner.balance_simulator
    }

    pub fn balance_overrider(&self) -> Arc<dyn BalanceOverriding> {
        Arc::clone(&self.inner.balance_overrider)
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

    /// Check if a smart contract is deployed to the given address.
    pub async fn is_contract(&self, address: eth::Address) -> Result<bool, Error> {
        let code = self.web3.eth().code(address.into(), None).await?;
        Ok(!code.0.is_empty())
    }

    /// Returns a type that monitors the block chain to inform about the current
    /// block.
    pub fn current_block(&self) -> &CurrentBlockWatcher {
        &self.inner.current_block
    }

    /// Create access list used by a transaction.
    pub async fn create_access_list<T>(&self, tx: T) -> Result<eth::AccessList, Error>
    where
        CallRequest: From<T>,
    {
        let mut tx: CallRequest = tx.into();
        tx.gas = Some(self.inner.tx_gas_limit);
        tx.gas_price = self.simulation_gas_price().await;

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

    /// Estimate gas used by a transaction.
    pub async fn estimate_gas(&self, tx: &eth::Tx) -> Result<eth::Gas, Error> {
        self.web3
            .eth()
            .estimate_gas(
                web3::types::CallRequest {
                    from: Some(tx.from.into()),
                    to: Some(tx.to.into()),
                    value: Some(tx.value.into()),
                    data: Some(tx.input.clone().into()),
                    access_list: Some(tx.access_list.clone().into()),
                    gas_price: self.simulation_gas_price().await,
                    ..Default::default()
                },
                None,
            )
            .await
            .map(Into::into)
            .map_err(Into::into)
    }

    /// The gas price is determined based on the deadline by which the
    /// transaction must be included on-chain. A shorter deadline requires a
    /// higher gas price to increase the likelihood of timely inclusion.
    pub async fn gas_price(&self, time_limit: Option<Duration>) -> Result<eth::GasPrice, Error> {
        self.inner.gas.estimate(time_limit).await
    }

    pub fn block_gas_limit(&self) -> eth::Gas {
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
                    block_number: Some(block),
                    ..
                }) => {
                    if status.is_zero() {
                        eth::TxStatus::Reverted {
                            block_number: eth::BlockNo(block.as_u64()),
                        }
                    } else {
                        eth::TxStatus::Executed {
                            block_number: eth::BlockNo(block.as_u64()),
                        }
                    }
                }
                _ => eth::TxStatus::Pending,
            })
            .map_err(Into::into)
    }

    pub(super) async fn simulation_gas_price(&self) -> Option<eth::U256> {
        // Some nodes don't pick a reasonable default value when you don't specify a gas
        // price and default to 0. Additionally some sneaky tokens have special code
        // paths that detect that case to try to behave differently during simulations
        // than they normally would. To not rely on the node picking a reasonable
        // default value we estimate the current gas price upfront. But because it's
        // extremely rare that tokens behave that way we are fine with falling back to
        // the node specific fallback value instead of failing the whole call.
        self.inner
            .gas
            .estimate(None)
            .await
            .ok()
            .map(|gas| gas.effective().0.0)
    }

    pub fn web3(&self) -> &Web3 {
        &self.web3
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
    Rpc(#[from] alloy::contract::Error),
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
            Error::Rpc(_) => true,
        }
    }
}

impl From<contracts::Error> for Error {
    fn from(err: contracts::Error) -> Self {
        match err {
            contracts::Error::Method(err) => Self::Method(err),
            contracts::Error::Rpc(err) => Self::Rpc(err),
        }
    }
}

impl From<SimulationError> for Error {
    fn from(err: SimulationError) -> Self {
        match err {
            SimulationError::Method(err) => Self::Rpc(err),
            SimulationError::Web3(err) => Self::Web3(err),
        }
    }
}
