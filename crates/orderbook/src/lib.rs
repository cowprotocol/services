use {
    crate::{database::Postgres, orderbook::Orderbook},
    anyhow::{anyhow, Context as _, Result},
    contracts::GPv2Settlement,
    futures::{Future, FutureExt},
    model::DomainSeparator,
    shared::{order_quoting::QuoteHandler, price_estimation::native::NativePriceEstimating},
    std::{convert::Infallible, net::SocketAddr, sync::Arc},
    tokio::{task, task::JoinHandle},
    warp::Filter,
    hyper::service::Service,
};

pub mod api;
pub mod app_data;
pub mod arguments;
pub mod database;
mod ipfs;
mod ipfs_app_data;
pub mod orderbook;
pub mod run;
pub mod solver_competition;

#[allow(clippy::too_many_arguments)]
pub fn serve_api(
    database: Postgres,
    orderbook: Arc<Orderbook>,
    quotes: Arc<QuoteHandler>,
    app_data: Arc<app_data::Registry>,
    address: SocketAddr,
    shutdown_receiver: impl Future<Output = ()> + Send + 'static,
    solver_competition_auth: Option<String>,
    native_price_estimator: Arc<dyn NativePriceEstimating>,
) -> JoinHandle<()> {
    let filter = api::handle_all_routes(
        database,
        orderbook,
        quotes,
        app_data,
        solver_competition_auth,
        native_price_estimator,
    )
    .boxed();
    tracing::info!(%address, "serving order book");
    let warp_svc = warp::service(filter);
    let make_svc = hyper::service::make_service_fn(move |_| {
        let warp_svc = warp_svc.clone();
        async move {
            let svc = hyper::service::service_fn(move |req: hyper::Request<hyper::Body>| {
                let mut warp_svc = warp_svc.clone();
                shared::tracing::REQUEST_ID.scope(Default::default(), async move {
                    // Not sure why but we have to have this async block to avoid panics
                    warp_svc.call(req).await
                })
            });
            Ok::<_, Infallible>(svc)
        }
    });
    let server = hyper::Server::bind(&address)
        .serve(make_svc)
        .with_graceful_shutdown(shutdown_receiver)
        .map(|_| ());
    task::spawn(server)
}

/**
 * Check that important constants such as the EIP 712 Domain Separator and
 * Order Type Hash used in this binary match the ones on the deployed
 * contract instance. Signature inconsistencies due to a mismatch of these
 * constants are hard to debug.
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
