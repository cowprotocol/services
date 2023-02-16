use {
    crate::deploy::Contracts,
    anyhow::{anyhow, Context, Result},
    clap::Parser,
    ethcontract::H256,
    model::auction::AuctionWithId,
    reqwest::StatusCode,
    sqlx::Connection,
    std::{future::Future, time::Duration},
    tokio::task::JoinHandle,
};

pub const API_HOST: &str = "http://127.0.0.1:8080";

pub async fn clear_database() {
    tracing::info!("Clearing database.");
    let mut db = sqlx::PgConnection::connect("postgresql://").await.unwrap();
    let mut db = db.begin().await.unwrap();
    database::clear_DANGER_(&mut db).await.unwrap();
    db.commit().await.unwrap();
}

pub fn start_autopilot(contracts: &Contracts, extra_arguments: &[String]) -> JoinHandle<()> {
    let args = [
        "autopilot".to_string(),
        "--network-block-interval=10".to_string(),
        "--auction-update-interval=1".to_string(),
        format!("--ethflow-contract={:?}", contracts.ethflow.address()),
        "--skip-event-sync".to_string(),
        "--enable-limit-orders".to_string(),
    ]
    .into_iter()
    .chain(api_autopilot_solver_arguments(contracts))
    .chain(api_autopilot_arguments())
    .chain(extra_arguments.iter().cloned());
    let args = autopilot::arguments::Arguments::try_parse_from(args).unwrap();
    tokio::task::spawn(autopilot::main(args))
}

pub fn start_api(contracts: &Contracts, extra_arguments: &[String]) -> JoinHandle<()> {
    let args = [
        "orderbook".to_string(),
        "--enable-presign-orders".to_string(),
        "--enable-eip1271-orders".to_string(),
        "--enable-limit-orders".to_string(),
    ]
    .into_iter()
    .chain(api_autopilot_solver_arguments(contracts))
    .chain(api_autopilot_arguments())
    .chain(extra_arguments.iter().cloned());
    let args = orderbook::arguments::Arguments::try_parse_from(args).unwrap();
    tokio::task::spawn(orderbook::run::run(args))
}

pub async fn wait_for_api_to_come_up() {
    let is_up = || async {
        reqwest::get(format!("{API_HOST}/api/v1/version"))
            .await
            .is_ok()
    };
    tracing::info!("Waiting for API to come up.");
    wait_for_condition(Duration::from_secs(10), is_up)
        .await
        .unwrap();
}

pub fn start_old_driver(
    contracts: &Contracts,
    private_key: &[u8; 32],
    extra_args: &[String],
) -> JoinHandle<()> {
    let args = [
        "solver".to_string(),
        format!("--solver-account={}", hex::encode(private_key)),
        "--settle-interval=1".to_string(),
    ]
    .into_iter()
    .chain(api_autopilot_solver_arguments(contracts).chain(extra_args.iter().cloned()));
    let args = solver::arguments::Arguments::try_parse_from(args).unwrap();
    tokio::task::spawn(solver::run::run(args))
}

fn api_autopilot_arguments() -> impl Iterator<Item = String> {
    [
        "--price-estimators=Baseline".to_string(),
        "--native-price-estimators=Baseline".to_string(),
        "--amount-to-estimate-prices-with=1000000000000000000".to_string(),
        "--block-stream-poll-interval-seconds=1".to_string(),
    ]
    .into_iter()
}

fn api_autopilot_solver_arguments(contracts: &Contracts) -> impl Iterator<Item = String> {
    [
        "--baseline-sources=None".to_string(),
        format!(
            "--custom-univ2-baseline-sources={:?}|{:?}",
            contracts.uniswap_router.address(),
            H256(shared::sources::uniswap_v2::UNISWAP_INIT),
        ),
        format!(
            "--settlement-contract-address={:?}",
            contracts.gp_settlement.address()
        ),
        format!("--native-token-address={:?}", contracts.weth.address()),
        format!(
            "--balancer-v2-vault-address={:?}",
            contracts.balancer_vault.address()
        ),
    ]
    .into_iter()
}

pub async fn get_auction() -> Result<AuctionWithId> {
    let response = reqwest::get(format!("{API_HOST}/api/v1/auction")).await?;
    let status = response.status();
    let body = response.text().await?;
    anyhow::ensure!(status == StatusCode::OK, "{body}");
    serde_json::from_str(&body).with_context(|| body.to_string())
}

pub async fn solvable_orders() -> Result<usize> {
    Ok(get_auction().await?.auction.orders.len())
}

/// Repeatedly evaluate condition until it returns true or the timeout is
/// reached. If condition evaluates to true, Ok(()) is returned. If the timeout
/// is reached Err is returned.
pub async fn wait_for_condition<Fut>(
    timeout: Duration,
    mut condition: impl FnMut() -> Fut,
) -> Result<()>
where
    Fut: Future<Output = bool>,
{
    let start = std::time::Instant::now();
    while !condition().await {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if start.elapsed() > timeout {
            return Err(anyhow!("timeout"));
        }
    }
    Ok(())
}
