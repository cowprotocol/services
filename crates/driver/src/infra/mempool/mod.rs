use {
    crate::{
        boundary::unbuffered_web3_client,
        domain::{competition, eth, mempools},
        infra,
    },
    ethcontract::dyns::DynWeb3,
};

#[derive(Debug, Clone)]
pub struct Config {
    pub min_priority_fee: eth::U256,
    pub gas_price_cap: eth::U256,
    pub target_confirm_time: std::time::Duration,
    pub max_confirm_time: std::time::Duration,
    pub retry_interval: std::time::Duration,
    pub kind: Kind,
}

impl Config {
    pub fn deadline(&self) -> tokio::time::Instant {
        tokio::time::Instant::now() + self.max_confirm_time
    }
}

#[derive(Debug, Clone)]
pub enum Kind {
    /// The public mempool of the [`Ethereum`] node.
    Public(RevertProtection),
    /// The MEVBlocker private mempool.
    MEVBlocker {
        url: reqwest::Url,
        max_additional_tip: eth::U256,
        additional_tip_percentage: f64,
        use_soft_cancellations: bool,
    },
}

impl Kind {
    /// for instrumentization purposes
    pub fn format_variant(&self) -> &'static str {
        match self {
            Kind::Public(_) => "PublicMempool",
            Kind::MEVBlocker { .. } => "MEVBlocker",
        }
    }
}

/// Don't submit transactions with high revert risk (i.e. transactions
/// that interact with on-chain AMMs) to the public mempool.
/// This can be enabled to avoid MEV when private transaction
/// submission strategies are available. If private submission strategies
/// are not available, revert protection is always disabled.
#[derive(Debug, Clone, Copy)]
pub enum RevertProtection {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone)]
pub struct Mempool {
    transport: DynWeb3,
    config: Config,
}

impl std::fmt::Display for Mempool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Mempool({})", self.config.kind.format_variant())
    }
}

impl Mempool {
    pub fn new(config: Config, transport: DynWeb3) -> Self {
        let transport = match &config.kind {
            Kind::Public(_) => transport,
            // Flashbots Protect RPC fallback doesn't support buffered transport
            Kind::MEVBlocker { url, .. } => unbuffered_web3_client(url),
        };
        Self { config, transport }
    }

    /// Submits a transaction to the mempool. Returns optimistically as soon as
    /// the transaction is pending.
    pub async fn submit(
        &self,
        tx: eth::Tx,
        gas: competition::solution::settlement::Gas,
        solver: &infra::Solver,
    ) -> Result<eth::TxId, mempools::Error> {
        ethcontract::transaction::TransactionBuilder::new(self.transport.clone())
            .from(solver.account().clone())
            .to(tx.to.into())
            .gas_price(ethcontract::GasPrice::Eip1559 {
                max_fee_per_gas: gas.price.max().into(),
                max_priority_fee_per_gas: gas.price.tip().into(),
            })
            .data(tx.input.into())
            .value(tx.value.0)
            .gas(gas.limit.0)
            .access_list(web3::types::AccessList::from(tx.access_list))
            .resolve(ethcontract::transaction::ResolveCondition::Pending)
            .send()
            .await
            .map(|result| eth::TxId(result.hash()))
            .map_err(|err| mempools::Error::Other(anyhow::Error::from(err)))
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn may_revert(&self) -> bool {
        match &self.config.kind {
            Kind::Public(_) => true,
            Kind::MEVBlocker { .. } => false,
        }
    }
}
