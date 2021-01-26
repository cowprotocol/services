use contracts::GPv2Settlement;
use model::DomainSeparator;
use orderbook::{
    orderbook::Orderbook, serve_task, storage::InMemoryOrderBook,
    verify_deployed_contract_constants,
};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use structopt::StructOpt;
use tokio::task;

#[derive(Debug, StructOpt)]
struct Arguments {
    #[structopt(flatten)]
    shared: shared_arguments::Arguments,

    #[structopt(long, env = "BIND_ADDRESS", default_value = "0.0.0.0:8080")]
    bind_address: SocketAddr,
}

const MAINTENANCE_INTERVAL: Duration = Duration::from_secs(10);

pub async fn orderbook_maintenance(
    storage: Arc<Orderbook>,
    settlement_contract: GPv2Settlement,
) -> ! {
    loop {
        tracing::debug!("running order book maintenance");
        if let Err(err) = storage.run_maintenance(&settlement_contract).await {
            tracing::error!(?err, "maintenance error");
        }
        tokio::time::delay_for(MAINTENANCE_INTERVAL).await;
    }
}
#[tokio::main]
async fn main() {
    let args = Arguments::from_args();
    tracing_setup::initialize(args.shared.log_filter.as_str());
    tracing::info!("running order book with {:#?}", args);

    let transport = web3::transports::Http::new(args.shared.node_url.as_str())
        .expect("transport creation failed");
    let web3 = web3::Web3::new(transport);
    let settlement_contract = GPv2Settlement::deployed(&web3)
        .await
        .expect("Couldn't load deployed settlement");
    let chain_id = web3
        .eth()
        .chain_id()
        .await
        .expect("Could not get chainId")
        .as_u64();
    verify_deployed_contract_constants(&settlement_contract, chain_id)
        .await
        .expect("Deployed contract constants don't match the ones in this binary");
    let domain_separator =
        DomainSeparator::get_domain_separator(chain_id, settlement_contract.address());
    let storage = InMemoryOrderBook::default();
    let orderbook = Arc::new(Orderbook::new(domain_separator, Box::new(storage)));
    let serve_task = serve_task(orderbook.clone(), args.bind_address);
    let maintenance_task = task::spawn(orderbook_maintenance(orderbook, settlement_contract));
    tokio::select! {
        result = serve_task => tracing::error!(?result, "serve task exited"),
        result = maintenance_task => tracing::error!(?result, "maintenance task exited"),
    };
}
