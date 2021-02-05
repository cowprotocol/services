pub mod account_balances;
pub mod api;
pub mod database;
pub mod event_updater;
pub mod integer_conversions;
pub mod orderbook;

use crate::orderbook::Orderbook;
use anyhow::{anyhow, Context as _, Result};
use contracts::GPv2Settlement;
use model::DomainSeparator;
use std::{net::SocketAddr, sync::Arc};
use tokio::{task, task::JoinHandle};
use warp::Filter;

pub fn serve_task(orderbook: Arc<Orderbook>, address: SocketAddr) -> JoinHandle<()> {
    let cors = warp::cors()
        .allow_any_origin()
        .allow_methods(vec!["GET", "POST", "DELETE", "OPTIONS", "PUT", "PATCH"])
        .allow_headers(vec!["Origin", "Content-Type", "X-Auth-Token", "X-AppId"]);
    let filter = api::handle_all_routes(orderbook).with(cors);
    tracing::info!(%address, "serving order book");
    task::spawn(warp::serve(filter).bind(address))
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
