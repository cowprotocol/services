pub mod account_balances;
pub mod api;
pub mod conversions;
pub mod database;
pub mod event_updater;
pub mod fee;
pub mod metrics;
pub mod orderbook;

use crate::database::Database;
use crate::orderbook::Orderbook;
use anyhow::{anyhow, Context as _, Result};
use contracts::GPv2Settlement;
use fee::EthAwareMinFeeCalculator;
use metrics::Metrics;
use model::DomainSeparator;
use prometheus::Registry;
use shared::{
    metrics::{serve_metrics, DEFAULT_METRICS_PORT},
    price_estimate::PriceEstimating,
};
use std::{net::SocketAddr, sync::Arc};
use tokio::{task, task::JoinHandle};

pub fn serve_task(
    database: Database,
    orderbook: Arc<Orderbook>,
    fee_calculator: Arc<EthAwareMinFeeCalculator>,
    price_estimator: Arc<dyn PriceEstimating>,
    address: SocketAddr,
) -> JoinHandle<()> {
    let registry = Registry::default();
    let metrics = Arc::new(Metrics::new(&registry));
    let filter = api::handle_all_routes(
        database,
        orderbook,
        fee_calculator,
        price_estimator,
        metrics,
    );
    let mut metrics_address = address;
    tracing::info!(%address, "serving order book");
    task::spawn(warp::serve(filter).bind(address));

    tracing::info!(%metrics_address, "serving metrics");
    metrics_address.set_port(DEFAULT_METRICS_PORT);
    serve_metrics(registry, metrics_address)
}

/**
 * Check that important constants such as the EIP 712 Domain Separator and Order Type Hash used in this binary match the ones on the deployed contract instance.
 * Signature inconsistencies due to a mismatch of these constants are hard to debug.
 */
pub async fn verify_deployed_contract_constants(
    contract: &GPv2Settlement,
    chain_id: u64,
) -> Result<()> {
    let web3 = contract.raw_instance().web3();
    let bytecode = hex::encode(
        web3.eth()
            .code(contract.address(), None)
            .await
            .context("Could not load deployed bytecode")?
            .0,
    );

    let domain_separator = DomainSeparator::get_domain_separator(chain_id, contract.address());
    if !bytecode.contains(&hex::encode(domain_separator.0)) {
        return Err(anyhow!("Bytecode did not contain domain separator"));
    }

    if !bytecode.contains(&hex::encode(model::order::OrderCreation::ORDER_TYPE_HASH)) {
        return Err(anyhow!("Bytecode did not contain order type hash"));
    }
    Ok(())
}
