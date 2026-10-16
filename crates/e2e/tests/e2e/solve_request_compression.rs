use {
    configs::{
        autopilot::{
            Configuration,
            run_loop::RunLoopConfig,
            solver::{Account, Solver},
        },
        test_util::TestDefault,
    },
    e2e::setup::{
        proxy::{OnRequest, ReverseProxy},
        *,
    },
    ethrpc::alloy::CallBuilderExt,
    model::{
        order::{OrderCreation, OrderKind},
        signature::EcdsaSigningScheme,
    },
    number::units::EthUnit,
    shared::web3::Web3,
    std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    url::Url,
};

#[tokio::test]
#[ignore]
async fn local_node_opted_in_driver_receives_compressed_solve_requests() {
    run_test(opted_in_driver_receives_compressed_solve_requests).await;
}

#[tokio::test]
#[ignore]
async fn local_node_driver_without_opt_in_receives_plain_solve_requests() {
    run_test(driver_without_opt_in_receives_plain_solve_requests).await;
}

#[tokio::test]
#[ignore]
async fn local_node_disabled_compression_overrides_driver_opt_in() {
    run_test(disabled_compression_overrides_driver_opt_in).await;
}

async fn opted_in_driver_receives_compressed_solve_requests(web3: Web3) {
    let received = settle_order_through_proxy(web3, true, true).await;

    assert!(received.brotli() > 0, "{received:?}");
    assert_eq!(received.plain(), 0, "{received:?}");
}

async fn driver_without_opt_in_receives_plain_solve_requests(web3: Web3) {
    let received = settle_order_through_proxy(web3, true, false).await;

    assert!(received.plain() > 0, "{received:?}");
    assert_eq!(received.brotli(), 0, "{received:?}");
}

async fn disabled_compression_overrides_driver_opt_in(web3: Web3) {
    let received = settle_order_through_proxy(web3, false, true).await;

    assert!(received.plain() > 0, "{received:?}");
    assert_eq!(received.brotli(), 0, "{received:?}");
}

/// Counts the `/solve` requests the driver received, by encoding. Only bodies
/// that decode to a full auction are counted, so a body the driver couldn't
/// parse shows up as missing.
#[derive(Debug, Default)]
struct Received {
    plain: AtomicUsize,
    brotli: AtomicUsize,
    undecodable: AtomicUsize,
}

impl Received {
    fn plain(&self) -> usize {
        self.assert_all_decodable();
        self.plain.load(Ordering::Acquire)
    }

    fn brotli(&self) -> usize {
        self.assert_all_decodable();
        self.brotli.load(Ordering::Acquire)
    }

    fn assert_all_decodable(&self) {
        assert_eq!(self.undecodable.load(Ordering::Acquire), 0, "{self:?}");
    }

    fn record(&self, content_encoding: Option<&str>, body: &[u8]) {
        let (counter, json) = match content_encoding {
            None => (&self.plain, Some(body.to_vec())),
            Some("br") => {
                let mut decompressed = Vec::new();
                let json = brotli::BrotliDecompress(&mut &*body, &mut decompressed)
                    .ok()
                    .map(|_| decompressed);
                (&self.brotli, json)
            }
            Some(_) => (&self.undecodable, None),
        };
        let is_full_auction = json
            .and_then(|json| serde_json::from_slice::<serde_json::Value>(&json).ok())
            .is_some_and(|json| json["kind"] == "full");
        let counter = if is_full_auction {
            counter
        } else {
            &self.undecodable
        };
        counter.fetch_add(1, Ordering::AcqRel);
    }
}

/// Settles a single order with the autopilot talking to the driver through a
/// proxy that records every `/solve` request, and returns what it recorded.
async fn settle_order_through_proxy(
    web3: Web3,
    compress_solve_request: bool,
    supports_brotli: bool,
) -> Arc<Received> {
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(1u64.eth()).await;
    let [trader] = onchain.make_accounts(1u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    token.mint(trader.address(), 10u64.eth()).await;
    token
        .approve(onchain.contracts().allowance, 10u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    let received = Arc::new(Received::default());
    let on_request: OnRequest = Arc::new({
        let received = Arc::clone(&received);
        move |parts, body| {
            if !parts.uri.path().ends_with("/solve") {
                return;
            }
            let content_encoding = parts
                .headers
                .get("content-encoding")
                .map(|value| value.to_str().unwrap());
            received.record(content_encoding, body);
        }
    });
    let backend: Url = "http://0.0.0.0:11088".parse().unwrap();
    let _proxy =
        ReverseProxy::start_with_callback("0.0.0.0:11089".parse().unwrap(), &[backend], on_request);

    let services = Services::new(&onchain).await;
    let config = Configuration::test_no_drivers();
    let config = Configuration {
        drivers: vec![Solver {
            supports_brotli,
            ..Solver::new(
                "test_solver".to_string(),
                "http://localhost:11089/test_solver".parse().unwrap(),
                Account::Address(solver.address()),
            )
        }],
        run_loop: RunLoopConfig {
            compress_solve_request,
            ..config.run_loop
        },
        ..config
    };
    services
        .start_protocol_with_args(
            config,
            configs::orderbook::Configuration::test_default(),
            solver,
        )
        .await;

    let weth = &onchain.contracts().weth;
    let order = OrderCreation {
        sell_token: *token.address(),
        sell_amount: 10u64.eth(),
        buy_token: *weth.address(),
        buy_amount: 5u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    let balance_before = weth.balanceOf(trader.address()).call().await.unwrap();
    services.create_order(&order).await.unwrap();

    tracing::info!("Waiting for trade.");
    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        let balance_after = weth.balanceOf(trader.address()).call().await.unwrap();
        balance_after.saturating_sub(balance_before) >= 5u64.eth()
    })
    .await
    .unwrap();

    received
}
