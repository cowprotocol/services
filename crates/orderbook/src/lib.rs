pub mod account_balances;
pub mod api;
pub mod conversions;
pub mod cow_subsidy;
pub mod database;
pub mod event_updater;
pub mod fee;
pub mod gas_price;
pub mod metrics;
pub mod orderbook;
pub mod signature_validator;
pub mod solvable_orders;
pub mod solver_competition;

use crate::{api::post_quote::OrderQuoter, orderbook::Orderbook};
use anyhow::{anyhow, Context as _, Result};
use contracts::GPv2Settlement;
use database::trades::TradeRetrieving;
use futures::Future;
use model::DomainSeparator;
use solver_competition::SolverCompetition;
use std::{net::SocketAddr, sync::Arc};
use tokio::{task, task::JoinHandle};
use warp::Filter;

pub fn serve_api(
    database: Arc<dyn TradeRetrieving>,
    orderbook: Arc<Orderbook>,
    quoter: Arc<OrderQuoter>,
    address: SocketAddr,
    shutdown_receiver: impl Future<Output = ()> + Send + 'static,
    solver_competition: Arc<SolverCompetition>,
) -> JoinHandle<()> {
    let filter = api::handle_all_routes(database, orderbook, quoter, solver_competition).boxed();
    tracing::info!(%address, "serving order book");
    let (_, server) = warp::serve(filter).bind_with_graceful_shutdown(address, shutdown_receiver);
    task::spawn(server)
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

    let domain_separator = DomainSeparator::new(chain_id, contract.address());
    if !bytecode.contains(&hex::encode(domain_separator.0)) {
        return Err(anyhow!("Bytecode did not contain domain separator"));
    }

    if !bytecode.contains(&hex::encode(model::order::OrderData::TYPE_HASH)) {
        return Err(anyhow!("Bytecode did not contain order type hash"));
    }
    Ok(())
}
