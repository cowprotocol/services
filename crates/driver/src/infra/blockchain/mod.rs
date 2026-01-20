use {
    crate::{
        boundary,
        domain::{eth, eth::U256},
    },
    alloy::{
        eips::eip1559::Eip1559Estimation,
        network::TransactionBuilder,
        providers::Provider,
        rpc::types::{TransactionReceipt, TransactionRequest},
        transports::TransportErrorKind,
    },
    anyhow::anyhow,
    chain::Chain,
    ethcontract::errors::ExecutionError,
    ethrpc::{Web3, block_stream::CurrentBlockWatcher},
    shared::{
        account_balances::{BalanceSimulator, SimulationError},
        gas_price_estimation::Eip1559EstimationExt,
        price_estimation::trade_verifier::balance_overrides::{
            BalanceOverrides,
            BalanceOverriding,
        },
    },
    std::{fmt, sync::Arc},
    thiserror::Error,
    tracing::{Level, instrument},
    url::Url,
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
        current_block_args: &shared::current_block::Arguments,
    ) -> Self {
        let Rpc { web3, chain, args } = rpc;

        let current_block_stream = current_block_args
            .stream(args.url.clone(), web3.alloy.clone())
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
            Some(*contracts.vault().address()),
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
        let code = self.web3.alloy.get_code_at(address).await?;
        Ok(!code.is_empty())
    }

    /// Returns a type that monitors the block chain to inform about the current
    /// block.
    pub fn current_block(&self) -> &CurrentBlockWatcher {
        &self.inner.current_block
    }

    /// Create access list used by a transaction.
    #[instrument(skip_all)]
    pub async fn create_access_list<T>(&self, tx: T) -> Result<eth::AccessList, Error>
    where
        T: Into<TransactionRequest>,
    {
        let tx = tx.into();

        let gas_limit = self.inner.tx_gas_limit.try_into().map_err(|err| {
            Error::GasPrice(anyhow!("failed to convert gas_limit to u64: {err:?}"))
        })?;
        let tx = tx.with_gas_limit(gas_limit);
        let tx = match self.simulation_gas_price().await {
            Some(gas_price) => tx.with_gas_price(gas_price),
            _ => tx,
        };

        let access_list = self.web3.alloy.create_access_list(&tx).pending().await?;

        Ok(access_list
            .ensure_ok()
            .map_err(Error::AccessList)?
            .access_list
            .into())
    }

    /// Estimate gas used by a transaction.
    pub async fn estimate_gas(&self, tx: eth::Tx) -> Result<eth::Gas, Error> {
        let tx = TransactionRequest::default()
            .from(tx.from)
            .to(tx.to)
            .value(tx.value.0)
            .input(tx.input.0.into())
            .access_list(tx.access_list.into());

        let tx = match self.simulation_gas_price().await {
            Some(gas_price) => tx.with_gas_price(gas_price),
            _ => tx,
        };

        let estimated_gas = self
            .web3
            .alloy
            .estimate_gas(tx)
            .pending()
            .await
            .map_err(Error::Rpc)?
            .into();

        Ok(estimated_gas)
    }

    /// The gas price is determined based on the deadline by which the
    /// transaction must be included on-chain. A shorter deadline requires a
    /// higher gas price to increase the likelihood of timely inclusion.
    pub async fn gas_price(&self) -> Result<Eip1559Estimation, Error> {
        self.inner.gas.estimate().await
    }

    pub fn block_gas_limit(&self) -> eth::Gas {
        self.inner.current_block.borrow().gas_limit.into()
    }

    /// Returns the current [`eth::Ether`] balance of the specified account.
    pub async fn balance(&self, address: eth::Address) -> Result<eth::Ether, Error> {
        self.web3
            .alloy
            .get_balance(address)
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
            .alloy
            .get_transaction_receipt(tx_hash.0)
            .await
            .map(|result| {
                let Some(
                    receipt @ TransactionReceipt {
                        block_number: Some(block_number),
                        ..
                    },
                ) = result
                else {
                    return eth::TxStatus::Pending;
                };

                if receipt.status() {
                    eth::TxStatus::Executed {
                        block_number: eth::BlockNo(block_number),
                    }
                } else {
                    eth::TxStatus::Reverted {
                        block_number: eth::BlockNo(block_number),
                    }
                }
            })
            .map_err(Into::into)
    }

    #[instrument(skip(self), ret(level = Level::DEBUG))]
    pub(super) async fn simulation_gas_price(&self) -> Option<u128> {
        let base_fee = self.current_block().borrow().base_fee;
        // Some nodes don't pick a reasonable default value when you don't specify a gas
        // price and default to 0. Additionally some sneaky tokens have special code
        // paths that detect that case to try to behave differently during simulations
        // than they normally would. To not rely on the node picking a reasonable
        // default value we estimate the current gas price upfront. But because it's
        // extremely rare that tokens behave that way we are fine with falling back to
        // the node specific fallback value instead of failing the whole call.
        Some(
            self.inner
                .gas
                .estimate()
                .await
                .ok()?
                .effective(Some(base_fee)),
        )
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
    ContractRpc(#[from] alloy::contract::Error),
    #[error("alloy rpc error: {0:?}")]
    Rpc(#[from] alloy::transports::RpcError<TransportErrorKind>),
    #[error("method error: {0:?}")]
    Method(#[from] ethcontract::errors::MethodError),
    #[error("web3 error: {0:?}")]
    Web3(#[from] web3::error::Error),
    #[error("gas price estimation error: {0}")]
    GasPrice(boundary::Error),
    #[error("access list estimation error: {0:?}")]
    AccessList(String),
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
            Error::ContractRpc(_) => true,
            Error::Rpc(err) => {
                let is_revert = err.is_error_resp();
                tracing::trace!(is_revert, ?err, "classified error");
                is_revert
            }
        }
    }
}

impl From<contracts::Error> for Error {
    fn from(err: contracts::Error) -> Self {
        match err {
            contracts::Error::Method(err) => Self::Method(err),
            contracts::Error::Rpc(err) => Self::ContractRpc(err),
        }
    }
}

impl From<SimulationError> for Error {
    fn from(err: SimulationError) -> Self {
        match err {
            SimulationError::Method(err) => Self::ContractRpc(err),
            SimulationError::Web3(err) => Self::Web3(err),
        }
    }
}
