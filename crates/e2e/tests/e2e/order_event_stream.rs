use {
    e2e::setup::*,
    ethrpc::alloy::CallBuilderExt,
    model::{
        order::{OrderCreation, OrderKind, OrderUid},
        signature::EcdsaSigningScheme,
    },
    number::units::EthUnit,
    orderbook::dto::order_event::{Label, OrderEvent},
    reqwest::StatusCode,
    shared::web3::Web3,
    std::time::Duration,
};

#[tokio::test]
#[ignore]
async fn local_node_order_event_stream() {
    run_test(order_event_stream).await;
}

/// The server-sent events stream of one order.
struct OrderEventStream {
    response: reqwest::Response,
    buffer: String,
}

impl OrderEventStream {
    async fn request(uid: &OrderUid) -> reqwest::Response {
        reqwest::get(format!("{API_HOST}/api/v1/orders/{uid}/events"))
            .await
            .unwrap()
    }

    async fn open(uid: &OrderUid) -> Self {
        let response = Self::request(uid).await;
        assert_eq!(response.status(), StatusCode::OK);
        let content_type = response.headers()[reqwest::header::CONTENT_TYPE]
            .to_str()
            .unwrap();
        assert!(content_type.starts_with("text/event-stream"));
        Self {
            response,
            buffer: String::new(),
        }
    }

    /// The next event, or `None` if the server closed the stream.
    async fn next_event(&mut self) -> Option<OrderEvent> {
        loop {
            while let Some(end) = self.buffer.find("\n\n") {
                let frame: String = self.buffer.drain(..end + 2).collect();
                // Frames without data set the reconnect delay or keep the
                // connection alive.
                if let Some(data) = frame.lines().find_map(|line| line.strip_prefix("data:")) {
                    return Some(serde_json::from_str(data).unwrap());
                }
            }
            let chunk = tokio::time::timeout(TIMEOUT, self.response.chunk())
                .await
                .expect("timed out waiting for an order event")
                .unwrap()?;
            self.buffer.push_str(std::str::from_utf8(&chunk).unwrap());
        }
    }
}

async fn order_event_stream(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(10u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    onchain
        .contracts()
        .weth
        .approve(onchain.contracts().allowance, 3u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader.address())
        .value(3u64.eth())
        .send_and_watch()
        .await
        .unwrap();

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let order = OrderCreation {
        sell_token: *onchain.contracts().weth.address(),
        sell_amount: 2u64.eth(),
        buy_token: *token.address(),
        buy_amount: 1u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Buy,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    let uid = services.create_order(&order).await.unwrap();

    let mut events = OrderEventStream::open(&uid).await;
    let another_order = OrderUid::from_parts(Default::default(), trader.address(), order.valid_to);
    let mut events_of_another_order = OrderEventStream::open(&another_order).await;

    // Without a new block no auction runs, so the stream is open before the
    // order makes progress.
    onchain.mint_block().await;

    // The order is fill-or-kill, so the server closes the stream after the
    // trade.
    let mut labels = Vec::new();
    while let Some(event) = events.next_event().await {
        assert_eq!(event.uid, uid);
        labels.push(event.label);
    }
    assert_eq!(labels.first(), Some(&Label::Ready));
    assert!(labels.contains(&Label::Executing));
    assert_eq!(labels.last(), Some(&Label::Traded));

    // A finished order has no stream: neither a traded one nor an expired one.
    let expired_order = OrderUid::from_parts(Default::default(), trader.address(), 1);
    for finished_order in [uid, expired_order] {
        let response = OrderEventStream::request(&finished_order).await;
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    let event_of_another_order = tokio::time::timeout(
        Duration::from_millis(500),
        events_of_another_order.next_event(),
    )
    .await;
    assert!(event_of_another_order.is_err());
}
