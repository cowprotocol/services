use {
    crate::{
        client::ByosClient,
        config,
        liquidity,
        orderbook::OrderbookClient,
        proposal::ProposalBuilder,
        solver::BaselineWrapper,
    },
    alloy_signer_local::PrivateKeySigner,
    byos::domain::eip712,
    clap::Parser,
    contracts::{GPv2Settlement, WETH9},
    shared::arguments::tracing_config,
};

#[derive(Parser, Debug)]
#[command(version)]
struct Args {
    #[arg(long, env, default_value = "warn,subsolver=debug")]
    log: String,

    #[clap(long, env, default_value = "false")]
    use_json_logs: bool,

    #[clap(flatten)]
    tracing: shared::arguments::TracingArguments,

    #[arg(long, env)]
    config: std::path::PathBuf,
}

pub async fn start() {
    observe::panic_hook::install();
    let args = Args::parse();

    let obs_config = observe::Config::new(
        &args.log,
        tracing::Level::ERROR.into(),
        args.use_json_logs,
        tracing_config(&args.tracing, "subsolver".into()),
    );
    observe::tracing::init::initialize_reentrant(&obs_config);

    let commit_hash = option_env!("VERGEN_GIT_SHA").unwrap_or("COMMIT_INFO_NOT_FOUND");
    tracing::info!(%commit_hash, "running subsolver with {args:#?}");

    let config = config::load(&args.config).await;

    let chain_id = config.chain_id;
    let weth = WETH9::deployment_address(&chain_id)
        .unwrap_or_else(|| panic!("no WETH address for chain {chain_id}"));
    let settlement = GPv2Settlement::deployment_address(&chain_id)
        .unwrap_or_else(|| panic!("no settlement address for chain {chain_id}"));

    let orderbook = OrderbookClient::new(config.orderbook_url.clone());
    let byos = ByosClient::new(config.byos_url.clone());
    let solver = BaselineWrapper::new(&config.solver, weth).await;

    let signer: PrivateKeySigner = config.private_key.parse().expect("invalid private key");
    let domain = eip712::byos_domain(chain_id);
    let proposal_builder =
        ProposalBuilder::new(signer.clone(), domain, config.uniswap_v2.router, settlement);

    tracing::info!(
        %chain_id,
        signer = %signer.address(),
        router = %config.uniswap_v2.router,
        "subsolver configured",
    );

    loop {
        match run_iteration(
            &orderbook,
            &byos,
            &solver,
            &proposal_builder,
            &config.solver.base_tokens,
        )
        .await
        {
            Ok(count) => {
                if count > 0 {
                    tracing::info!(proposals = count, "submitted proposals");
                }
            }
            Err(err) => {
                tracing::warn!(?err, "iteration failed");
            }
        }

        tokio::time::sleep(config.poll_interval).await;
    }
}

async fn run_iteration(
    orderbook: &OrderbookClient,
    byos: &ByosClient,
    solver: &BaselineWrapper,
    proposal_builder: &ProposalBuilder,
    base_tokens: &[alloy_primitives::Address],
) -> anyhow::Result<usize> {
    let auction = orderbook.get_auction().await?;
    tracing::debug!(
        auction_id = auction.id,
        orders = auction.orders.len(),
        "polled orderbook",
    );

    if auction.orders.is_empty() {
        return Ok(0);
    }

    // For now, we construct empty pools since the full pool fetching
    // infrastructure requires an RPC connection and pool cache.
    // TODO: integrate shared::sources::uniswap_v2::pool_fetching
    let _token_pairs = liquidity::token_pairs_from_orders(&auction.orders, base_tokens);

    // Use a default gas price (30 gwei)
    let gas_price = alloy_primitives::U256::from(30_000_000_000u64);

    let pools = vec![]; // TODO: fetch real pools
    let solutions = solver
        .solve(&auction.orders, &pools, gas_price, &auction.prices)
        .await;

    tracing::debug!(solutions = solutions.len(), "baseline solver returned");

    let mut submitted = 0;
    for sol in &solutions {
        let trade = match sol.trades.first() {
            Some(solvers::domain::solution::Trade::Fulfillment(f)) => f,
            _ => continue,
        };
        let uid = trade.order().uid.0;

        match proposal_builder.build_and_sign(&uid, sol) {
            Ok(proposal) => match byos.submit_proposal(&proposal).await {
                Ok(id) => {
                    tracing::debug!(
                        %id,
                        order_uid = %const_hex::encode_prefixed(uid),
                        "proposal submitted",
                    );
                    submitted += 1;
                }
                Err(err) => {
                    tracing::warn!(
                        ?err,
                        order_uid = %const_hex::encode_prefixed(uid),
                        "failed to submit proposal",
                    );
                }
            },
            Err(err) => {
                tracing::warn!(
                    ?err,
                    order_uid = %const_hex::encode_prefixed(uid),
                    "failed to build proposal",
                );
            }
        }
    }

    Ok(submitted)
}
