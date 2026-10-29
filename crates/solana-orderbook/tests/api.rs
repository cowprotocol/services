//! Integration tests for the HTTP API server.

use {
    base64::Engine,
    cow_solana_rpc::{Mocks, MocksMap, RpcRequest, SolanaRPC},
    database::{
        byte_array::ByteArray,
        solana::{OrderEventLabel, OrderKind},
    },
    solana_orderbook::infra::{api::Api, db, prices::NativePrices, quoter::Quoter},
    solana_sdk::signer::Signer,
    spl_token_2022_interface::extension::{
        BaseStateWithExtensionsMut,
        ExtensionType,
        transfer_fee::TransferFeeConfig,
    },
    sqlx::PgPool,
    std::{net::SocketAddr, time::Duration},
    tokio_util::sync::CancellationToken,
};

async fn mock_api() -> Api {
    Api {
        addr: "0.0.0.0:0".parse().unwrap(),
        // A lazy pool at a dead endpoint keeps these tests database-free: a
        // quote insert degrades to an id-less answer, nothing else queries.
        pool: PgPool::connect_lazy("postgresql://127.0.0.1:1/").unwrap(),
        quoter: dead_quoter(),
        prices: spawn_mock_autopilot(Some(1_000_000_000)).await,
        validation: Default::default(),
        quote_expiry: Duration::from_secs(60),
        sponsoring: None,
        app_data_size_limit: 8192,
    }
}

/// A quoter pointing at a dead endpoint: every quote attempt fails.
fn dead_quoter() -> Quoter {
    Quoter::new(
        vec!["http://127.0.0.1:1/".parse().unwrap()],
        Duration::from_secs(1),
    )
}

/// Spawn the API server on an ephemeral port and return its bound address.
async fn spawn_server() -> SocketAddr {
    spawn_server_with(dead_quoter()).await
}

/// Spawn the API server with the given quoter.
async fn spawn_server_with(quoter: Quoter) -> SocketAddr {
    let api = Api {
        quoter,
        ..mock_api().await
    };
    let (listener, addr) = api.bind().await.unwrap();
    // A token that is never cancelled keeps the server alive for the test.
    let shutdown = CancellationToken::new();
    tokio::spawn(async move { api.serve(listener, shutdown).await.unwrap() });
    addr
}

/// A tiny axum server that answers `/native_price/{mint}` with `price` for
/// every mint, or 404 without one. It stands in for the autopilot.
async fn spawn_mock_autopilot(price: Option<u64>) -> NativePrices {
    let app = axum::Router::new().route(
        "/native_price/{mint}",
        axum::routing::get(move || async move {
            match price {
                Some(price) => (
                    axum::http::StatusCode::OK,
                    serde_json::json!({ "price": price.to_string() }).to_string(),
                ),
                None => (axum::http::StatusCode::NOT_FOUND, "No liquidity".to_owned()),
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    NativePrices::new(
        format!("http://{addr}").parse().unwrap(),
        Duration::from_secs(1),
    )
}

/// A tiny axum server that answers `/quote` with a fixed response. It stands
/// in for the driver.
async fn spawn_mock_driver(response: serde_json::Value) -> SocketAddr {
    let app = axum::Router::new().route(
        "/quote",
        axum::routing::post(move || {
            let response = response.clone();
            async move { axum::Json(response) }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    addr
}

#[tokio::test]
async fn healthz_returns_200() {
    let addr = spawn_server().await;
    let response = reqwest::Client::new()
        .get(format!("http://{addr}/healthz"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
}

/// Spawn the API server on `pool` and return the status and the body of
/// `GET /api/v1/ready`.
async fn get_ready(pool: PgPool) -> (reqwest::StatusCode, String) {
    let api = Api {
        pool,
        ..mock_api().await
    };
    let (listener, addr) = api.bind().await.unwrap();
    let shutdown = CancellationToken::new();
    tokio::spawn(async move { api.serve(listener, shutdown).await.unwrap() });
    let response = reqwest::Client::new()
        .get(format!("http://{addr}/api/v1/ready"))
        .send()
        .await
        .unwrap();
    (response.status(), response.text().await.unwrap())
}

/// The readiness probe fails when the database does not answer.
#[tokio::test]
async fn ready_returns_500_without_a_database() {
    // The pool tries a refused connection again until the acquire timeout.
    // A short timeout keeps the test fast.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .acquire_timeout(Duration::from_millis(100))
        .connect_lazy("postgresql://127.0.0.1:1/")
        .unwrap();
    let (status, body) = get_ready(pool).await;
    assert_eq!(status, reqwest::StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&body).unwrap(),
        serde_json::json!({"errorType": "InternalServerError", "description": ""})
    );
}

/// The readiness probe passes when the database answers.
#[tokio::test]
#[ignore = "needs a local database"]
async fn solana_db_ready_returns_200() {
    let pool = PgPool::connect("postgresql://").await.unwrap();
    let (status, _) = get_ready(pool).await;
    assert_eq!(status, reqwest::StatusCode::OK);
}

/// The version route answers like the EVM orderbook's: the build's
/// `<ref>@<sha>` as plain text.
#[tokio::test]
async fn version_reports_the_build() {
    let addr = spawn_server().await;
    let response = reqwest::Client::new()
        .get(format!("http://{addr}/api/v1/version"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert_eq!(
        response.text().await.unwrap(),
        observe::version::git_version()
    );
}

/// The path parameters are checked before any database read, in the API's
/// error shape.
#[tokio::test]
async fn solver_competition_rejects_malformed_path_parameters() {
    let addr = spawn_server().await;
    for (path, error_type) in [
        ("/api/v2/solver_competition/abc", "InvalidAuctionId"),
        ("/api/v2/solver_competition/-1", "InvalidAuctionId"),
        (
            "/restricted/api/v2/solver_competition/abc",
            "InvalidAuctionId",
        ),
        (
            "/api/v2/solver_competition/by_tx_hash/not-base58",
            "InvalidTransactionSignature",
        ),
    ] {
        let response = reqwest::get(format!("http://{addr}{path}")).await.unwrap();
        assert_eq!(
            response.status(),
            reqwest::StatusCode::BAD_REQUEST,
            "{path}"
        );
        let body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(body["errorType"], error_type, "{path}");
    }
}

#[tokio::test]
async fn shuts_down_cleanly_on_signal() {
    let api = mock_api().await;
    let (listener, addr) = api.bind().await.unwrap();
    let shutdown_token = CancellationToken::new();
    let serve = api.serve(listener, shutdown_token.clone());
    let handle = tokio::spawn(async move { serve.await.unwrap() });

    // The server is up and serving requests.
    let response = reqwest::Client::new()
        .get(format!("http://{addr}/healthz"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    // Trigger graceful shutdown and assert the serve future completes cleanly.
    shutdown_token.cancel();
    handle.await.unwrap();
}

/// A malformed quote body is rejected in the API's error shape, not axum's
/// plain-text default.
#[tokio::test]
async fn malformed_quote_body_keeps_the_error_shape() {
    let addr = spawn_server().await;
    let response = reqwest::Client::new()
        .post(format!("http://{addr}/api/v1/quote"))
        .json(&serde_json::json!({"sellToken": "not-a-pubkey"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap(),
        serde_json::json!({
            "errorType": "InvalidRequestBody",
            "description": "The request body could not be parsed."
        })
    );
}

/// A quote body with the given validity fields, otherwise well formed.
fn quote_body(validity: serde_json::Value) -> serde_json::Value {
    let mut body = serde_json::json!({
        "from": "9VXC6LH9eXMBpXLQnxMYAGkjs59Zon2ACciJwQ6iMzNB",
        "sellToken": "So11111111111111111111111111111111111111112",
        "buyToken": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
        "kind": "sell",
        "sellAmountBeforeFee": "10000000"
    });
    body.as_object_mut()
        .unwrap()
        .extend(validity.as_object().unwrap().clone());
    body
}

async fn post_quote(addr: SocketAddr, body: serde_json::Value) -> (reqwest::StatusCode, String) {
    let response = reqwest::Client::new()
        .post(format!("http://{addr}/api/v1/quote"))
        .json(&body)
        .send()
        .await
        .unwrap();
    let status = response.status();
    let json: serde_json::Value = response.json().await.unwrap();
    (
        status,
        json["errorType"].as_str().unwrap_or_default().to_owned(),
    )
}

/// Validation runs before any driver is asked, so every rejection here
/// answers 400 without reaching the dead quoter.
#[tokio::test]
async fn quote_validation_rejects_bad_orders() {
    let addr = spawn_server().await;
    let mut same_tokens = quote_body(serde_json::json!({"validFor": 1800}));
    same_tokens["buyToken"] = same_tokens["sellToken"].clone();
    // The body sells wSOL, which counts as the same token as native SOL.
    let mut unwrap = quote_body(serde_json::json!({"validFor": 1800}));
    unwrap["buyToken"] = serde_json::json!("11111111111111111111111111111111");
    let mut zero_amount = quote_body(serde_json::json!({"validFor": 1800}));
    zero_amount["sellAmountBeforeFee"] = serde_json::json!("0");

    for (body, expected) in [
        (
            quote_body(serde_json::json!({"validFor": 10})),
            "InsufficientValidTo",
        ),
        (
            quote_body(serde_json::json!({"validFor": 2 * 365 * 24 * 60 * 60})),
            "ExcessiveValidTo",
        ),
        (same_tokens, "SameBuyAndSellToken"),
        (unwrap, "SameBuyAndSellToken"),
        (zero_amount, "ZeroAmount"),
    ] {
        let (status, kind) = post_quote(addr, body).await;
        assert_eq!(
            (status, kind.as_str()),
            (reqwest::StatusCode::BAD_REQUEST, expected)
        );
    }
}

/// Without sponsoring the quote reads no wallet, so a native SOL buy quoted
/// under the rent-exempt minimum of an empty account answers
/// `InvalidNativeBuy`.
#[tokio::test]
async fn quote_rejects_a_native_buy_under_the_payout_floor() {
    for (buy_amount, expected) in [
        (
            "890879",
            (reqwest::StatusCode::BAD_REQUEST, "InvalidNativeBuy"),
        ),
        ("890880", (reqwest::StatusCode::OK, "")),
    ] {
        let driver = spawn_mock_driver(serde_json::json!({
            "sellAmount": "10000000",
            "buyAmount": buy_amount,
            "solver": "9VXC6LH9eXMBpXLQnxMYAGkjs59Zon2ACciJwQ6iMzNB",
        }))
        .await;
        let addr = spawn_server_with(Quoter::new(
            vec![format!("http://{driver}/").parse().unwrap()],
            Duration::from_secs(1),
        ))
        .await;
        let mut body = quote_body(serde_json::json!({"validFor": 1800}));
        body["sellToken"] = serde_json::json!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
        body["buyToken"] = serde_json::json!("11111111111111111111111111111111");
        let (status, kind) = post_quote(addr, body).await;
        assert_eq!((status, kind.as_str()), expected);
    }
}

/// With sponsoring on, a native SOL buy quoted under the rent-exempt minimum
/// of an empty account reads its wallet: a funded wallet takes the payout, a
/// wallet the payout leaves under the minimum does not. The lookups answer
/// the sell mint, then the wallet.
#[tokio::test]
async fn quote_reads_the_wallet_of_a_small_native_buy() {
    let floor = solana_sdk::rent::Rent::default().minimum_balance(0);
    for (lamports, buy_amount, expected) in [
        (floor, 1, (reqwest::StatusCode::OK, "")),
        (1_000, floor - 1_000, (reqwest::StatusCode::OK, "")),
        (
            1_000,
            floor - 1_001,
            (reqwest::StatusCode::BAD_REQUEST, "InvalidNativeBuy"),
        ),
    ] {
        let driver = spawn_mock_driver(serde_json::json!({
            "sellAmount": "10000000",
            "buyAmount": buy_amount.to_string(),
            "solver": "9VXC6LH9eXMBpXLQnxMYAGkjs59Zon2ACciJwQ6iMzNB",
        }))
        .await;
        let wallet = solana_sdk::account::Account {
            lamports,
            owner: solana_system_interface::program::ID,
            ..Default::default()
        };
        let mocks = MocksMap::from_iter([
            (
                RpcRequest::GetMultipleAccounts,
                accounts_response(&[Some(mint_account(spl_token_interface::ID))]),
            ),
            (
                RpcRequest::GetMultipleAccounts,
                accounts_response(&[Some(wallet)]),
            ),
        ]);
        let api = Api {
            quoter: Quoter::new(
                vec![format!("http://{driver}/").parse().unwrap()],
                Duration::from_secs(1),
            ),
            sponsoring: Some(solana_orderbook::infra::api::Sponsoring {
                funder: solana_sdk::pubkey::Pubkey::new_unique(),
                settlement_program: program_id(),
                rpc: SolanaRPC::new_mock_with_mocks_map(mocks),
                max_priority_fee_lamports: 100_000,
                mints: Default::default(),
            }),
            ..mock_api().await
        };
        let (listener, addr) = api.bind().await.unwrap();
        let shutdown = CancellationToken::new();
        tokio::spawn(async move { api.serve(listener, shutdown).await.unwrap() });
        let mut body = quote_body(serde_json::json!({"validFor": 1800}));
        body["sellToken"] = serde_json::json!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
        body["buyToken"] = serde_json::json!("11111111111111111111111111111111");
        let (status, kind) = post_quote(addr, body).await;
        assert_eq!((status, kind.as_str()), expected);
    }
}

/// The full happy path: the driver's amounts come back in the EVM response
/// shape with the request's own fields echoed, net of the buy token account
/// rent and the driver's network fee at the mock price of a lamport an atom.
#[tokio::test]
async fn quote_answers_in_the_evm_shape() {
    let driver = spawn_mock_driver(serde_json::json!({
        "sellAmount": "10000000",
        "buyAmount": "3461207",
        "solver": "9VXC6LH9eXMBpXLQnxMYAGkjs59Zon2ACciJwQ6iMzNB",
        "networkFee": "6400",
    }))
    .await;
    let addr = spawn_server_with(Quoter::new(
        vec![format!("http://{driver}/").parse().unwrap()],
        Duration::from_secs(1),
    ))
    .await;

    let valid_to = chrono::Utc::now().timestamp() + 600;
    let mut body = quote_body(serde_json::json!({"validTo": valid_to}));
    body["receiver"] = serde_json::json!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
    body["appData"] = serde_json::json!("{}");
    body["appDataHash"] = serde_json::json!(EMPTY_APP_DATA_HASH);

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/api/v1/quote"))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let json: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "quote": {
                "sellToken": body["sellToken"],
                "buyToken": body["buyToken"],
                "receiver": body["receiver"],
                "sellAmount": "10000000",
                "buyAmount": "1234567",
                "validTo": valid_to,
                "appData": body["appData"],
                "appDataHash": body["appDataHash"],
                "feeAmount": "0",
                // In the buy token, at the mocked lamport an atom. Without
                // sponsoring the buy token account is not read, so it counts
                // as the costliest a settleable mint can need at the SDK's
                // default rent.
                "costs": {
                    "token": body["buyToken"],
                    "networkFees": "6400",
                    "buyTokenAtaCosts": "2220240",
                    "protocolFee": "0",
                    "total": "2226640",
                },
                "kind": "sell",
                "partiallyFillable": false,
            },
            "from": body["from"],
            "expiration": json["expiration"],
            "id": null,
            "verified": false,
            "funder": null,
        })
    );
    // The amounts are honored for about a minute from now.
    let expiration: chrono::DateTime<chrono::Utc> =
        json["expiration"].as_str().unwrap().parse().unwrap();
    let honored_for = (expiration - chrono::Utc::now()).num_seconds();
    assert!(
        (50..=60).contains(&honored_for),
        "expiration {honored_for}s away"
    );

    // A document alone answers with its hash alongside, no app data as the
    // zero hash, like the EVM orderbook.
    let zero = format!("0x{}", "00".repeat(32));
    for (app_data, expected_app_data, expected_hash) in [
        (
            Some("{}"),
            serde_json::json!("{}"),
            Some(EMPTY_APP_DATA_HASH),
        ),
        (None, serde_json::json!(zero), None),
    ] {
        let mut body = quote_body(serde_json::json!({"validTo": valid_to}));
        if let Some(app_data) = app_data {
            body["appData"] = serde_json::json!(app_data);
        }
        let response = reqwest::Client::new()
            .post(format!("http://{addr}/api/v1/quote"))
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        let json: serde_json::Value = response.json().await.unwrap();
        assert_eq!(json["quote"]["appData"], expected_app_data, "{app_data:?}");
        assert_eq!(
            json["quote"]
                .get("appDataHash")
                .and_then(|hash| hash.as_str()),
            expected_hash,
            "{app_data:?}"
        );
    }
}

/// A sponsoring deployment names its funder, so a client can pin the fee payer
/// of the creation transaction it signs next without carrying the address.
#[tokio::test]
async fn quote_names_the_funder_when_sponsoring_is_on() {
    let driver = spawn_mock_driver(serde_json::json!({
        "sellAmount": "10000000",
        "buyAmount": "10000000",
        "solver": "9VXC6LH9eXMBpXLQnxMYAGkjs59Zon2ACciJwQ6iMzNB",
    }))
    .await;
    let funder = solana_sdk::pubkey::Pubkey::new_from_array([0x77; 32]);
    let api = Api {
        quoter: Quoter::new(
            vec![format!("http://{driver}/").parse().unwrap()],
            Duration::from_secs(1),
        ),
        sponsoring: Some(solana_orderbook::infra::api::Sponsoring {
            funder,
            settlement_program: program_id(),
            rpc: SolanaRPC::new_mock_with_mocks(Mocks::from([(
                RpcRequest::GetMultipleAccounts,
                classic_mints(),
            )])),
            max_priority_fee_lamports: 100_000,
            mints: Default::default(),
        }),
        ..mock_api().await
    };
    let (listener, addr) = api.bind().await.unwrap();
    let shutdown = CancellationToken::new();
    tokio::spawn(async move { api.serve(listener, shutdown).await.unwrap() });

    let valid_to = chrono::Utc::now().timestamp() + 600;
    let response = reqwest::Client::new()
        .post(format!("http://{addr}/api/v1/quote"))
        .json(&quote_body(serde_json::json!({"validTo": valid_to})))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let json: serde_json::Value = response.json().await.unwrap();
    assert_eq!(json["funder"], serde_json::json!(funder.to_string()));
}

/// The lookups answer the mints, then the owner, its associated token
/// account, the buy mint and the rent sysvar. The rent comes off the buy
/// amount at the mock price of a lamport an atom.
#[tokio::test]
async fn quote_prices_the_missing_buy_token_account_by_its_mint() {
    let driver = spawn_mock_driver(serde_json::json!({
        "sellAmount": "10000000",
        "buyAmount": "10000000",
        "solver": "9VXC6LH9eXMBpXLQnxMYAGkjs59Zon2ACciJwQ6iMzNB",
    }))
    .await;
    let free_fee_mint =
        solana_testlib::token_2022_mint(&[ExtensionType::TransferFeeConfig], |mint| {
            mint.init_extension::<TransferFeeConfig>(true).unwrap();
        });
    // The mainnet rent since SIMD-0437, under the SDK's default.
    let rent = solana_sdk::account::create_account_for_test(
        &solana_sdk::rent::Rent::with_lamports_per_byte(5080),
    );
    for (buy_mint, cost) in [
        (mint_account(spl_token_interface::ID), 1_488_440_u64),
        (mint_account(spl_token_2022_interface::ID), 1_513_840),
        (free_fee_mint, 1_574_800),
    ] {
        let mints = accounts_response(&[
            Some(mint_account(spl_token_interface::ID)),
            Some(buy_mint.clone()),
        ]);
        let mocks = MocksMap::from_iter([
            (RpcRequest::GetMultipleAccounts, mints),
            (
                RpcRequest::GetMultipleAccounts,
                accounts_response(&[None, None, Some(buy_mint), Some(rent.clone())]),
            ),
        ]);
        let api = Api {
            quoter: Quoter::new(
                vec![format!("http://{driver}/").parse().unwrap()],
                Duration::from_secs(1),
            ),
            sponsoring: Some(solana_orderbook::infra::api::Sponsoring {
                funder: solana_sdk::pubkey::Pubkey::new_unique(),
                settlement_program: cow_settlement_interface::id(),
                rpc: SolanaRPC::new_mock_with_mocks_map(mocks),
                max_priority_fee_lamports: 100_000,
                mints: Default::default(),
            }),
            ..mock_api().await
        };
        let (listener, addr) = api.bind().await.unwrap();
        let shutdown = CancellationToken::new();
        tokio::spawn(async move { api.serve(listener, shutdown).await.unwrap() });

        let response = reqwest::Client::new()
            .post(format!("http://{addr}/api/v1/quote"))
            .json(&quote_body(serde_json::json!({"validFor": 1800})))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        let json: serde_json::Value = response.json().await.unwrap();
        assert_eq!(json["quote"]["costs"]["buyTokenAtaCosts"], cost.to_string());
        assert_eq!(json["quote"]["buyAmount"], (10_000_000 - cost).to_string());
    }
}

/// With several drivers configured, the best answer wins: the largest buy
/// amount for a sell order once each driver's own network fee and the buy
/// token account rent (2,220,240 lamports, a lamport an atom) come off it.
/// The larger raw answer loses to the fee it reports, and a driver that
/// reports no fee is charged none.
#[tokio::test]
async fn quote_picks_the_best_driver_answer() {
    let worse = spawn_mock_driver(serde_json::json!({
        "sellAmount": "10000000",
        "buyAmount": "5001000",
        "solver": "9VXC6LH9eXMBpXLQnxMYAGkjs59Zon2ACciJwQ6iMzNB",
        "networkFee": "2000000",
    }))
    .await;
    let better = spawn_mock_driver(serde_json::json!({
        "sellAmount": "10000000",
        "buyAmount": "5000000",
        "solver": "9VXC6LH9eXMBpXLQnxMYAGkjs59Zon2ACciJwQ6iMzNB",
    }))
    .await;
    let addr = spawn_server_with(Quoter::new(
        vec![
            format!("http://{worse}/").parse().unwrap(),
            format!("http://{better}/").parse().unwrap(),
            "http://127.0.0.1:1/".parse().unwrap(),
        ],
        Duration::from_secs(1),
    ))
    .await;

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/api/v1/quote"))
        .json(&quote_body(serde_json::json!({"validFor": 1800})))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let json: serde_json::Value = response.json().await.unwrap();
    assert_eq!(json["quote"]["buyAmount"], "2779760");
    assert_eq!(json["quote"]["costs"]["networkFees"], "0");
}

/// A buy order keeps its buy amount and pulls the execution cost, priced in
/// the sell token, through the sell amount instead.
#[tokio::test]
async fn quote_charges_a_buy_order_on_the_sell_side() {
    let driver = spawn_mock_driver(serde_json::json!({
        "sellAmount": "10000000",
        "buyAmount": "1234567",
        "solver": "9VXC6LH9eXMBpXLQnxMYAGkjs59Zon2ACciJwQ6iMzNB",
        "networkFee": "6400",
    }))
    .await;
    let addr = spawn_server_with(Quoter::new(
        vec![format!("http://{driver}/").parse().unwrap()],
        Duration::from_secs(1),
    ))
    .await;

    let mut body = quote_body(serde_json::json!({"validFor": 1800}));
    body.as_object_mut().unwrap().remove("sellAmountBeforeFee");
    body["kind"] = serde_json::json!("buy");
    body["buyAmountAfterFee"] = serde_json::json!("1234567");
    let response = reqwest::Client::new()
        .post(format!("http://{addr}/api/v1/quote"))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let json: serde_json::Value = response.json().await.unwrap();
    assert_eq!(json["quote"]["kind"], "buy");
    assert_eq!(json["quote"]["sellAmount"], "12226640");
    assert_eq!(json["quote"]["buyAmount"], "1234567");
    assert_eq!(
        json["quote"]["costs"],
        serde_json::json!({
            "token": body["sellToken"],
            "networkFees": "6400",
            "buyTokenAtaCosts": "2220240",
            "protocolFee": "0",
            "total": "2226640",
        })
    );
}

/// A fill the execution cost swallows is no quote.
#[tokio::test]
async fn quote_rejects_a_fill_the_cost_swallows() {
    let driver = spawn_mock_driver(serde_json::json!({
        "sellAmount": "10000000",
        "buyAmount": "1000",
        "solver": "9VXC6LH9eXMBpXLQnxMYAGkjs59Zon2ACciJwQ6iMzNB",
    }))
    .await;
    let addr = spawn_server_with(Quoter::new(
        vec![format!("http://{driver}/").parse().unwrap()],
        Duration::from_secs(1),
    ))
    .await;
    let (status, kind) = post_quote(addr, quote_body(serde_json::json!({"validFor": 1800}))).await;
    assert_eq!(
        (status, kind.as_str()),
        (
            reqwest::StatusCode::BAD_REQUEST,
            "SellAmountDoesNotCoverFee"
        )
    );
}

/// A sponsoring deployment reads the quoted mints and refuses one the
/// settlement program cannot move before asking any driver.
#[tokio::test]
async fn quote_rejects_mints_the_settlement_program_cannot_move() {
    let addr = spawn_sponsored_server_with(
        PgPool::connect_lazy("postgresql://").unwrap(),
        solana_sdk::pubkey::Pubkey::new_unique(),
        sponsored_mocks(
            true,
            accounts_response(&[None, Some(mint_account(spl_token_interface::ID))]),
        ),
    )
    .await;
    let (status, kind) = post_quote(addr, quote_body(serde_json::json!({"validFor": 1800}))).await;
    assert_eq!(
        (status, kind.as_str()),
        (reqwest::StatusCode::BAD_REQUEST, "UnsupportedToken")
    );
}

/// Native SOL has no mint to read, and a failed mint lookup quotes unchecked:
/// both reach the drivers, a dead one here.
#[tokio::test]
async fn quote_checks_only_the_mints_it_can_read() {
    let mut native_buy = quote_body(serde_json::json!({"validFor": 1800}));
    // The default body sells wSOL, which for native SOL is the same token.
    native_buy["sellToken"] = serde_json::json!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
    native_buy["buyToken"] = serde_json::json!(
        cow_settlement_interface::data::intent::ENCODED_NATIVE_SOL_TRANSFER.to_string()
    );
    for (body, mints) in [
        (
            native_buy,
            accounts_response(&[Some(mint_account(spl_token_interface::ID))]),
        ),
        (
            quote_body(serde_json::json!({"validFor": 1800})),
            serde_json::json!("not an account list"),
        ),
    ] {
        let addr = spawn_sponsored_server_with(
            PgPool::connect_lazy("postgresql://").unwrap(),
            solana_sdk::pubkey::Pubkey::new_unique(),
            sponsored_mocks(true, mints),
        )
        .await;
        let (status, kind) = post_quote(addr, body).await;
        assert_eq!(
            (status, kind.as_str()),
            (reqwest::StatusCode::NOT_FOUND, "NoLiquidity")
        );
    }
}

/// A token the autopilot cannot price answers no liquidity even when a
/// driver quotes the pair: an order on it would never enter an auction.
#[tokio::test]
async fn quote_without_a_native_price_is_no_liquidity() {
    let driver = spawn_mock_driver(serde_json::json!({
        "sellAmount": "10000000",
        "buyAmount": "1234567",
        "solver": "9VXC6LH9eXMBpXLQnxMYAGkjs59Zon2ACciJwQ6iMzNB",
    }))
    .await;
    let api = Api {
        quoter: Quoter::new(
            vec![format!("http://{driver}/").parse().unwrap()],
            Duration::from_secs(1),
        ),
        prices: spawn_mock_autopilot(None).await,
        ..mock_api().await
    };
    let (listener, addr) = api.bind().await.unwrap();
    let shutdown = CancellationToken::new();
    tokio::spawn(async move { api.serve(listener, shutdown).await.unwrap() });
    let (status, kind) = post_quote(addr, quote_body(serde_json::json!({"validFor": 1800}))).await;
    assert_eq!(
        (status, kind.as_str()),
        (reqwest::StatusCode::NOT_FOUND, "NoLiquidity")
    );
}

/// Every driver failure reads as no liquidity, mirroring the EVM mapping of
/// estimator errors.
#[tokio::test]
async fn quote_without_a_route_is_no_liquidity() {
    let addr = spawn_server().await;
    let (status, kind) = post_quote(addr, quote_body(serde_json::json!({"validFor": 1800}))).await;
    assert_eq!(
        (status, kind.as_str()),
        (reqwest::StatusCode::NOT_FOUND, "NoLiquidity")
    );
}

/// The keccak256 of `{}`, the hash the EVM orderbook names the empty document
/// by.
const EMPTY_APP_DATA_HASH: &str =
    "0xb48d38f93eaa084033fc5970bf96e559c33c4cdc07d889ab00b4d63f9590739d";

/// The quote parses app data like order creation: a document passes on to
/// the (dead) driver, a hash that disagrees with it is refused, and so is
/// `appDataHash` alone.
#[tokio::test]
async fn quote_checks_the_app_data_forms() {
    let addr = spawn_server().await;
    let other = format!("0x{}", "11".repeat(32));
    for (app_data, app_data_hash, expected) in [
        (
            Some("{}"),
            None,
            (reqwest::StatusCode::NOT_FOUND, "NoLiquidity"),
        ),
        (
            Some("{}"),
            Some(other.as_str()),
            (reqwest::StatusCode::BAD_REQUEST, "AppDataHashMismatch"),
        ),
        (
            None,
            Some(EMPTY_APP_DATA_HASH),
            (reqwest::StatusCode::BAD_REQUEST, "InvalidRequestBody"),
        ),
    ] {
        let mut body = quote_body(serde_json::json!({"validFor": 1800}));
        if let Some(app_data) = app_data {
            body["appData"] = serde_json::json!(app_data);
        }
        if let Some(app_data_hash) = app_data_hash {
            body["appDataHash"] = serde_json::json!(app_data_hash);
        }
        let (status, kind) = post_quote(addr, body).await;
        assert_eq!(
            (status, kind.as_str()),
            expected,
            "{app_data:?} {app_data_hash:?}"
        );
    }
}

/// The answered quote, net of the execution cost, lands in `solana.quotes`
/// and its id comes back.
#[tokio::test]
#[ignore = "needs the solana.* schema applied to the local database"]
async fn solana_db_quote_is_persisted() {
    let pool = PgPool::connect("postgresql://").await.unwrap();
    sqlx::query("TRUNCATE solana.quotes")
        .execute(&pool)
        .await
        .unwrap();
    let solver = solana_sdk::pubkey::Pubkey::new_unique();
    let driver = spawn_mock_driver(serde_json::json!({
        "sellAmount": "10000000",
        "buyAmount": "10000000",
        "solver": solver.to_string(),
    }))
    .await;
    let api = Api {
        pool: pool.clone(),
        quoter: Quoter::new(
            vec![format!("http://{driver}/").parse().unwrap()],
            Duration::from_secs(1),
        ),
        ..mock_api().await
    };
    let (listener, addr) = api.bind().await.unwrap();
    let shutdown = CancellationToken::new();
    tokio::spawn(async move { api.serve(listener, shutdown).await.unwrap() });

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/api/v1/quote"))
        .json(&quote_body(serde_json::json!({"validFor": 1800})))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let json: serde_json::Value = response.json().await.unwrap();
    let id = json["id"].as_i64().unwrap();

    type Row = (
        i64,
        Vec<u8>,
        Vec<u8>,
        String,
        String,
        String,
        Vec<u8>,
        chrono::DateTime<chrono::Utc>,
    );
    let row: Row = sqlx::query_as(
        "SELECT id, sell_token, buy_token, sell_amount::text, buy_amount::text, kind::text, \
         solver, expiration_timestamp FROM solana.quotes",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let sell: solana_sdk::pubkey::Pubkey = "So11111111111111111111111111111111111111112"
        .parse()
        .unwrap();
    let buy: solana_sdk::pubkey::Pubkey = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
        .parse()
        .unwrap();
    assert_eq!(row.0, id);
    assert_eq!(row.1, sell.to_bytes());
    assert_eq!(row.2, buy.to_bytes());
    assert_eq!(row.3, "10000000");
    assert_eq!(row.4, "7779760");
    assert_eq!(row.5, "sell");
    assert_eq!(row.6, solver.to_bytes());
    // The stored expiry is the answered one, up to timestamptz rounding.
    let answered: chrono::DateTime<chrono::Utc> =
        json["expiration"].as_str().unwrap().parse().unwrap();
    assert!((row.7 - answered).num_milliseconds().abs() <= 1);
}

/// Parameter validation of the trades endpoint short-circuits before any
/// database access.
#[tokio::test]
async fn trades_rejects_an_invalid_limit() {
    let addr = spawn_server().await;
    let uid = "11".repeat(32);
    for limit in ["0", "1001"] {
        let response = reqwest::Client::new()
            .get(format!(
                "http://{addr}/api/v2/trades?orderUid={uid}&limit={limit}"
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
        let json: serde_json::Value = response.json().await.unwrap();
        assert_eq!(json["errorType"], "InvalidLimit");
    }
}

/// Parameter validation of the account orders endpoint short-circuits before
/// any database access.
#[tokio::test]
async fn account_orders_rejects_bad_parameters() {
    let addr = spawn_server().await;
    let client = reqwest::Client::new();

    let response = client
        .get(format!("http://{addr}/api/v1/account/not-a-pubkey/orders"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
    let json: serde_json::Value = response.json().await.unwrap();
    assert_eq!(json["errorType"], "InvalidOwner");

    let response = client
        .get(format!(
            "http://{addr}/api/v1/account/9VXC6LH9eXMBpXLQnxMYAGkjs59Zon2ACciJwQ6iMzNB/orders?limit=0"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
    let json: serde_json::Value = response.json().await.unwrap();
    assert_eq!(json["errorType"], "LIMIT_OUT_OF_BOUNDS");
}

/// A sponsored-placement server: a fixed funder pubkey, the interface's
/// default settlement program, and a mock RPC answering the blockhash and
/// height probes and the account lookup with two classic SPL Token mints.
async fn spawn_sponsored_server(
    pool: PgPool,
    funder: solana_sdk::pubkey::Pubkey,
    blockhash_valid: bool,
) -> SocketAddr {
    spawn_sponsored_server_with(
        pool,
        funder,
        sponsored_mocks(blockhash_valid, classic_mints()),
    )
    .await
}

/// The mock blockhash probe answers "expired", and that probe is the first
/// gate after the account checks, so an order rejected with this kind passed
/// every check before it without the request reaching the database.
const PASSED_ACCOUNT_CHECKS: &str = "BlockhashExpired";

/// The mock RPC answers to the blockhash and height probes, and `accounts`
/// to the order account lookup, a `getMultipleAccounts` response.
fn sponsored_mocks(blockhash_valid: bool, accounts: serde_json::Value) -> Mocks {
    Mocks::from([
        (
            RpcRequest::IsBlockhashValid,
            serde_json::json!({
                "context": { "slot": 1u64, "apiVersion": "2.0.0" },
                "value": blockhash_valid,
            }),
        ),
        (RpcRequest::GetBlockHeight, serde_json::json!(100u64)),
        (RpcRequest::GetMultipleAccounts, accounts),
    ])
}

/// A sponsored-placement server whose mock RPC answers from `mocks`.
async fn spawn_sponsored_server_with(
    pool: PgPool,
    funder: solana_sdk::pubkey::Pubkey,
    mocks: Mocks,
) -> SocketAddr {
    let api = Api {
        pool,
        sponsoring: Some(solana_orderbook::infra::api::Sponsoring {
            funder,
            settlement_program: program_id(),
            rpc: SolanaRPC::new_mock_with_mocks(mocks),
            max_priority_fee_lamports: 100_000,
            mints: Default::default(),
        }),
        ..mock_api().await
    };
    let (listener, addr) = api.bind().await.unwrap();
    let shutdown = CancellationToken::new();
    tokio::spawn(async move { api.serve(listener, shutdown).await.unwrap() });
    addr
}

/// A plain mint in the base layout both token programs share, owned by
/// `program`.
fn mint_account(program: solana_sdk::pubkey::Pubkey) -> solana_sdk::account::Account {
    solana_sdk::account::Account {
        owner: program,
        ..solana_testlib::classic_mint(6)
    }
}

/// A `getMultipleAccounts` response listing `accounts` in request order,
/// `None` for a missing account.
fn accounts_response(accounts: &[Option<solana_sdk::account::Account>]) -> serde_json::Value {
    solana_testlib::multiple_accounts_json(accounts.iter().map(|account| {
        account
            .as_ref()
            .map_or(serde_json::Value::Null, solana_testlib::account_json)
    }))
}

/// The account lookup of an order selling and buying classic SPL Token mints.
fn classic_mints() -> serde_json::Value {
    let mint = Some(mint_account(spl_token_interface::ID));
    accounts_response(&[mint.clone(), mint])
}

/// The order intent a sponsored transaction carries. `native` sells SOL
/// through the wSOL mint with real associated token accounts, so preparation
/// steps can accompany the transaction.
fn sponsored_intent(
    owner: solana_sdk::pubkey::Pubkey,
    native: bool,
) -> cow_settlement_interface::data::intent::OrderIntent {
    let buy_mint = solana_sdk::pubkey::Pubkey::new_from_array([0x55; 32]);
    let (sell_mint, sell_token_account, buy_token_account) = if native {
        let sell_mint = spl_token_interface::native_mint::ID;
        (sell_mint, ata(owner, sell_mint), ata(owner, buy_mint))
    } else {
        (
            solana_sdk::pubkey::Pubkey::new_from_array([0x66; 32]),
            solana_sdk::pubkey::Pubkey::new_from_array([0x33; 32]),
            ata(owner, buy_mint),
        )
    };
    cow_settlement_interface::data::intent::OrderIntent {
        owner,
        sell: cow_settlement_interface::data::intent::TokenAsset {
            mint: sell_mint,
            token_account: sell_token_account,
        },
        buy: cow_settlement_interface::data::intent::Asset::TokenProgram(
            cow_settlement_interface::data::intent::TokenAsset {
                mint: buy_mint,
                token_account: buy_token_account,
            },
        ),
        sell_amount: std::num::NonZeroU64::new(1_000).unwrap(),
        buy_amount: std::num::NonZeroU64::new(2_000).unwrap(),
        valid_to: u32::try_from(chrono::Utc::now().timestamp() + 3600).unwrap(),
        flags: cow_settlement_interface::data::intent::Flags {
            kind: cow_settlement_interface::data::intent::OrderKind::Sell,
            partially_fillable: false,
        },
        app_data: [0; 32],
    }
}

/// A sponsored intent buying `buy_amount` lamports of native SOL into the
/// owner's wallet.
fn native_buy_intent(
    owner: solana_sdk::pubkey::Pubkey,
    buy_amount: u64,
) -> cow_settlement_interface::data::intent::OrderIntent {
    let mut intent = sponsored_intent(owner, false);
    intent.buy = cow_settlement_interface::data::intent::Asset::Native(owner);
    intent.buy_amount = std::num::NonZeroU64::new(buy_amount).unwrap();
    intent
}

/// The owner's associated token account for `mint` under the SPL Token
/// program.
fn ata(
    owner: solana_sdk::pubkey::Pubkey,
    mint: solana_sdk::pubkey::Pubkey,
) -> solana_sdk::pubkey::Pubkey {
    spl_associated_token_account_interface::address::get_associated_token_address_with_program_id(
        &owner,
        &mint,
        &spl_token_interface::ID,
    )
}

/// The settlement program the tests run against: not the id the interface
/// crate was compiled for, so a crate constant leaking into a check fails.
fn program_id() -> solana_sdk::pubkey::Pubkey {
    solana_sdk::pubkey::Pubkey::new_from_array([0x53; 32])
}

/// The settlement state PDA delegations must target.
fn state_pda() -> solana_sdk::pubkey::Pubkey {
    solana_sdk::pubkey::Pubkey::find_program_address(
        &cow_settlement_interface::pda::state::STATE_PDA_SEEDS,
        &program_id(),
    )
    .0
}

/// The full whitelisted preparation prefix for a native-sell intent: create
/// the wSOL account (the owner pays its rent, as placement requires), fund
/// it, sync it, delegate it.
fn full_preparations(
    owner: solana_sdk::pubkey::Pubkey,
    intent: &cow_settlement_interface::data::intent::OrderIntent,
) -> Vec<solana_sdk::instruction::Instruction> {
    vec![
        spl_associated_token_account_interface::instruction::create_associated_token_account_idempotent(
            &owner,
            &owner,
            &intent.sell.mint,
            &spl_token_interface::ID,
        ),
        solana_system_interface::instruction::transfer(&owner, &intent.sell.token_account, 1_000),
        spl_token_interface::instruction::sync_native(
            &spl_token_interface::ID,
            &intent.sell.token_account,
        )
        .unwrap(),
        spl_token_interface::instruction::approve(
            &spl_token_interface::ID,
            &intent.sell.token_account,
            &state_pda(),
            &owner,
            &[],
            1_000,
        )
        .unwrap(),
    ]
}

/// A partially signed sponsored creation transaction: the given preparation
/// instructions in front of `CreateOrder`, the owner signed, the fee payer
/// slot (the funder) left as a placeholder.
fn creation_tx(
    funder: solana_sdk::pubkey::Pubkey,
    owner: &solana_sdk::signer::keypair::Keypair,
    intent: &cow_settlement_interface::data::intent::OrderIntent,
    preparations: Vec<solana_sdk::instruction::Instruction>,
    sign: bool,
) -> String {
    creation_tx_wrapped(funder, owner, intent, preparations, Vec::new(), sign)
}

/// A creation bundle carrying `trailing` instructions after `CreateOrder`,
/// the shape a wallet produces when it appends its own compute budget.
fn creation_tx_wrapped(
    funder: solana_sdk::pubkey::Pubkey,
    owner: &solana_sdk::signer::keypair::Keypair,
    intent: &cow_settlement_interface::data::intent::OrderIntent,
    preparations: Vec<solana_sdk::instruction::Instruction>,
    trailing: Vec<solana_sdk::instruction::Instruction>,
    sign: bool,
) -> String {
    let mut instructions = preparations;
    instructions.push(create_order(funder, owner.pubkey(), intent));
    instructions.extend(trailing);
    signed_creation_tx(funder, owner, instructions, sign)
}

/// The `CreateOrder` instruction creating `intent` for `owner` at the
/// funder's expense.
fn create_order(
    funder: solana_sdk::pubkey::Pubkey,
    owner: solana_sdk::pubkey::Pubkey,
    intent: &cow_settlement_interface::data::intent::OrderIntent,
) -> solana_sdk::instruction::Instruction {
    cow_settlement_client::instruction::CreateOrder {
        program_id: program_id(),
        owner,
        created_by: funder,
        intent,
    }
    .into()
}

/// `instructions` as a creation transaction with the funder as fee payer and
/// the owner signed when `sign`.
fn signed_creation_tx(
    funder: solana_sdk::pubkey::Pubkey,
    owner: &solana_sdk::signer::keypair::Keypair,
    instructions: Vec<solana_sdk::instruction::Instruction>,
    sign: bool,
) -> String {
    let message = solana_sdk::message::Message::new_with_blockhash(
        &instructions,
        Some(&funder),
        &solana_sdk::hash::Hash::new_unique(),
    );
    let signers = usize::from(message.header.num_required_signatures);
    let serialized = message.serialize();
    let mut signatures = vec![solana_sdk::signature::Signature::default(); signers];
    if sign {
        let keys = &message.account_keys;
        for (index, key) in keys.iter().take(signers).enumerate() {
            if *key == owner.pubkey() {
                signatures[index] = owner.sign_message(&serialized);
            }
        }
    }
    let tx = solana_sdk::transaction::VersionedTransaction {
        signatures,
        message: solana_sdk::message::VersionedMessage::Legacy(message),
    };
    base64::prelude::BASE64_STANDARD.encode(bincode::serialize(&tx).unwrap())
}

/// The buy-account creation step for the intent.
fn destination_creation(
    funder: solana_sdk::pubkey::Pubkey,
    owner: solana_sdk::pubkey::Pubkey,
    intent: &cow_settlement_interface::data::intent::OrderIntent,
) -> solana_sdk::instruction::Instruction {
    spl_associated_token_account_interface::instruction::create_associated_token_account_idempotent(
        &funder,
        &owner,
        &buy_mint(intent),
        &spl_token_interface::ID,
    )
}

/// The intent's buy mint. A native SOL buy has none, so it panics.
fn buy_mint(
    intent: &cow_settlement_interface::data::intent::OrderIntent,
) -> solana_sdk::pubkey::Pubkey {
    let cow_settlement_interface::data::intent::Asset::TokenProgram(buy) = &intent.buy else {
        panic!("a native SOL buy has no mint");
    };
    buy.mint
}

/// A sponsored creation transaction with an arbitrary SPL sell and no
/// preparation steps in front of `CreateOrder`.
fn sponsored_creation_tx(
    funder: solana_sdk::pubkey::Pubkey,
    owner: &solana_sdk::signer::keypair::Keypair,
    sign: bool,
) -> String {
    let intent = sponsored_intent(owner.pubkey(), false);
    creation_tx(funder, owner, &intent, vec![], sign)
}

/// A well-formed bundle whose priority fee is above the sponsored ceiling.
/// Neither factor is outlandish alone, the product is: 500000 micro-lamports
/// over the compute ceiling comes to 700000 lamports.
fn overpriced_creation_tx(
    funder: solana_sdk::pubkey::Pubkey,
    owner: &solana_sdk::signer::keypair::Keypair,
) -> String {
    let intent = sponsored_intent(owner.pubkey(), false);
    creation_tx_wrapped(
        funder,
        owner,
        &intent,
        vec![],
        vec![
            solana_compute_budget_interface::ComputeBudgetInstruction::set_compute_unit_price(
                500_000,
            ),
            solana_compute_budget_interface::ComputeBudgetInstruction::set_compute_unit_limit(
                1_400_000,
            ),
        ],
        true,
    )
}

/// The sponsored creation as a v1 message: its 5 SOL priority fee sits in the
/// message config, where no compute-budget instruction shows it.
fn v1_creation_tx(
    funder: solana_sdk::pubkey::Pubkey,
    owner: &solana_sdk::signer::keypair::Keypair,
) -> String {
    use solana_sdk::message::{VersionedMessage, v1};
    let intent = sponsored_intent(owner.pubkey(), false);
    let instructions = [create_order(funder, owner.pubkey(), &intent)];
    let message = VersionedMessage::V1(
        v1::Message::try_compile_with_config(
            &funder,
            &instructions,
            solana_sdk::hash::Hash::new_unique(),
            v1::TransactionConfig::empty().with_priority_fee(5_000_000_000),
        )
        .unwrap(),
    );
    let serialized = message.serialize();
    let signers = usize::from(message.header().num_required_signatures);
    let signatures = message.static_account_keys()[..signers]
        .iter()
        .map(|key| {
            if *key == owner.pubkey() {
                owner.sign_message(&serialized)
            } else {
                solana_sdk::signature::Signature::default()
            }
        })
        .collect();
    let tx = solana_sdk::transaction::VersionedTransaction {
        signatures,
        message,
    };
    base64::prelude::BASE64_STANDARD.encode(bincode::serialize(&tx).unwrap())
}

/// A Lighthouse instruction over one account, the shape Phantom injects.
fn lighthouse(
    discriminator: u8,
    account: solana_sdk::pubkey::Pubkey,
) -> solana_sdk::instruction::Instruction {
    solana_sdk::instruction::Instruction::new_with_bytes(
        solana_sdk::pubkey::Pubkey::from_str_const("L2TExMFKdjpN9kozasaurPirfHy9P8sbXoAN1qA3S95"),
        &[discriminator],
        vec![solana_sdk::instruction::AccountMeta::new_readonly(
            account, false,
        )],
    )
}

/// A bundle carrying a Lighthouse instruction outside the assertion range:
/// `MemoryWrite`, which funds an account from a payer the template cannot
/// vouch for.
fn lighthouse_memory_creation_tx(
    funder: solana_sdk::pubkey::Pubkey,
    owner: &solana_sdk::signer::keypair::Keypair,
) -> String {
    let intent = sponsored_intent(owner.pubkey(), false);
    creation_tx_wrapped(
        funder,
        owner,
        &intent,
        vec![],
        // Discriminator 0 is `MemoryWrite`.
        vec![lighthouse(0, owner.pubkey())],
        true,
    )
}

async fn post_order(addr: SocketAddr, transaction: String) -> (reqwest::StatusCode, String) {
    post_order_body(
        addr,
        serde_json::json!({ "partiallySignedTx": transaction }),
    )
    .await
}

async fn post_order_body(
    addr: SocketAddr,
    body: serde_json::Value,
) -> (reqwest::StatusCode, String) {
    let response = reqwest::Client::new()
        .post(format!("http://{addr}/api/v1/orders"))
        .json(&body)
        .send()
        .await
        .unwrap();
    let status = response.status();
    let json: serde_json::Value = response.json().await.unwrap();
    (
        status,
        json["errorType"].as_str().unwrap_or_default().to_owned(),
    )
}

/// Placement validation runs before any database access: every rejection
/// here answers without a live pool.
#[tokio::test]
async fn create_order_rejects_invalid_submissions() {
    let funder = solana_sdk::pubkey::Pubkey::new_unique();
    let owner = solana_sdk::signer::keypair::Keypair::new();

    // The feature is off without sponsoring config.
    let plain = spawn_server().await;
    let (status, kind) = post_order(plain, "AAAA".to_owned()).await;
    assert_eq!(
        (status, kind.as_str()),
        (reqwest::StatusCode::BAD_REQUEST, "SponsoringDisabled")
    );

    let addr =
        spawn_sponsored_server(PgPool::connect_lazy("postgresql://").unwrap(), funder, true).await;
    for (transaction, expected) in [
        ("bm90IGEgdHg=".to_owned(), "InvalidTransaction"),
        // The owner as fee payer: the funder must front the fees.
        (
            sponsored_creation_tx(owner.pubkey(), &owner, true),
            "WrongFeePayer",
        ),
        // Nobody signed: the owner's signature is required.
        (
            sponsored_creation_tx(funder, &owner, false),
            "InvalidSignature",
        ),
        // The funder pays the priority fee, so an outsized price is refused.
        (overpriced_creation_tx(funder, &owner), "InvalidTransaction"),
        // Nor can a v1 message hide one in its config.
        (v1_creation_tx(funder, &owner), "InvalidTransaction"),
        // A lighthouse assertion is fine, anything outside that range is not.
        (
            lighthouse_memory_creation_tx(funder, &owner),
            "InvalidTransaction",
        ),
    ] {
        let (status, kind) = post_order(addr, transaction).await;
        assert_eq!(
            (status, kind.as_str()),
            (reqwest::StatusCode::BAD_REQUEST, expected)
        );
    }

    // Intent-level rejections: the funder as owner, equal mints, and a
    // validTo outside the validity bounds.
    let mut funder_owned = sponsored_intent(owner.pubkey(), false);
    funder_owned.owner = funder;
    let mut same_token = sponsored_intent(owner.pubkey(), false);
    same_token.buy = cow_settlement_interface::data::intent::Asset::TokenProgram(
        cow_settlement_interface::data::intent::TokenAsset {
            mint: same_token.sell.mint,
            token_account: same_token.buy.encode().1,
        },
    );
    let mut expiring = sponsored_intent(owner.pubkey(), false);
    expiring.valid_to = u32::try_from(chrono::Utc::now().timestamp() + 30).unwrap();
    let mut distant = sponsored_intent(owner.pubkey(), false);
    distant.valid_to =
        u32::try_from(chrono::Utc::now().timestamp() + 2 * 365 * 24 * 60 * 60).unwrap();
    for (intent, expected) in [
        (funder_owned, "InvalidTransaction"),
        (same_token, "SameBuyAndSellToken"),
        (expiring, "InsufficientValidTo"),
        (distant, "ExcessiveValidTo"),
    ] {
        let transaction = creation_tx(funder, &owner, &intent, vec![], true);
        let (status, kind) = post_order(addr, transaction).await;
        assert_eq!(
            (status, kind.as_str()),
            (reqwest::StatusCode::BAD_REQUEST, expected)
        );
    }

    // A zero sell amount, which only the encoded bytes can carry.
    let intent = sponsored_intent(owner.pubkey(), false);
    let mut create = create_order(funder, owner.pubkey(), &intent);
    let sell_amount = create.data.len()
        - cow_settlement_interface::data::intent::EncodedOrderIntent::SIZE
        + cow_settlement_interface::data::intent::fixtures::SELL_AMOUNT_OFFSET;
    create.data[sell_amount..sell_amount + size_of::<u64>()].fill(0);
    let transaction = signed_creation_tx(funder, &owner, vec![create], true);
    let (status, kind) = post_order(addr, transaction).await;
    assert_eq!(
        (status, kind.as_str()),
        (reqwest::StatusCode::BAD_REQUEST, "ZeroAmount")
    );

    // wSOL sold for native SOL.
    let mut unwrap = native_buy_intent(owner.pubkey(), 1_000_000_000);
    unwrap.sell.mint = spl_token_interface::native_mint::ID;
    let (status, kind) = post_order(addr, creation_tx(funder, &owner, &unwrap, vec![], true)).await;
    assert_eq!(
        (status, kind.as_str()),
        (reqwest::StatusCode::BAD_REQUEST, "SameBuyAndSellToken")
    );

    // A well-formed transaction whose blockhash already died.
    let addr = spawn_sponsored_server(
        PgPool::connect_lazy("postgresql://").unwrap(),
        funder,
        false,
    )
    .await;
    let (status, kind) = post_order(addr, sponsored_creation_tx(funder, &owner, true)).await;
    assert_eq!(
        (status, kind.as_str()),
        (reqwest::StatusCode::BAD_REQUEST, "BlockhashExpired")
    );
}

/// A placement's app data must agree with the intent, which signed the hash:
/// a document hashing to it passes on to the account checks, with or without
/// its hash alongside, and so does the zero hash, which names the empty
/// document, named or implied by no app data at all. A document or hash that
/// disagrees is refused, as is a hash as the document, a hash without its
/// document, and a document that is not a JSON object.
#[tokio::test]
async fn create_order_checks_the_app_data() {
    let funder = solana_sdk::pubkey::Pubkey::new_unique();
    let owner = solana_sdk::signer::keypair::Keypair::new();
    let document = r#"{"appCode":"test"}"#;
    let hash = app_data::hash_full_app_data(document.as_bytes());
    let hash_hex = const_hex::encode_prefixed(hash);
    let other_hex = format!("0x{}", "11".repeat(32));
    let zero_hex = format!("0x{}", "00".repeat(32));
    let mut intent = sponsored_intent(owner.pubkey(), false);
    intent.app_data = hash;
    let transaction = creation_tx(funder, &owner, &intent, vec![], true);
    let mut zero_intent = sponsored_intent(owner.pubkey(), false);
    zero_intent.app_data = [0; 32];
    let zero_transaction = creation_tx(funder, &owner, &zero_intent, vec![], true);
    let body = |transaction: &str, app_data: Option<&str>, app_data_hash: Option<&str>| {
        let mut body = serde_json::json!({ "partiallySignedTx": transaction });
        if let Some(app_data) = app_data {
            body["appData"] = serde_json::json!(app_data);
        }
        if let Some(app_data_hash) = app_data_hash {
            body["appDataHash"] = serde_json::json!(app_data_hash);
        }
        body
    };

    // Refusals answer before any chain probe.
    let addr = spawn_sponsored_server(
        PgPool::connect_lazy("postgresql://").unwrap(),
        funder,
        false,
    )
    .await;
    for (app_data, app_data_hash, expected) in [
        (Some(r#"{"appCode":"other"}"#), None, "AppDataHashMismatch"),
        (
            Some(document),
            Some(other_hex.as_str()),
            "AppDataHashMismatch",
        ),
        (Some(other_hex.as_str()), None, "AppDataHashMismatch"),
        (
            Some(hash_hex.as_str()),
            Some(hash_hex.as_str()),
            "InvalidAppData",
        ),
        (None, Some(hash_hex.as_str()), "InvalidRequestBody"),
        (Some("[]"), None, "InvalidAppData"),
    ] {
        let (status, kind) =
            post_order_body(addr, body(&transaction, app_data, app_data_hash)).await;
        assert_eq!(
            (status, kind.as_str()),
            (reqwest::StatusCode::BAD_REQUEST, expected),
            "{app_data:?} {app_data_hash:?}"
        );
    }

    // The mock answers each probe once, so every passing placement gets a
    // fresh server.
    for (transaction, app_data, app_data_hash) in [
        (&transaction, Some(document), None),
        (&transaction, Some(document), Some(hash_hex.as_str())),
        (&zero_transaction, None, None),
        (&zero_transaction, Some(zero_hex.as_str()), None),
    ] {
        let addr = spawn_sponsored_server(
            PgPool::connect_lazy("postgresql://").unwrap(),
            funder,
            false,
        )
        .await;
        let (status, kind) =
            post_order_body(addr, body(transaction, app_data, app_data_hash)).await;
        assert_eq!(
            (status, kind.as_str()),
            (reqwest::StatusCode::BAD_REQUEST, PASSED_ACCOUNT_CHECKS),
            "{app_data:?} {app_data_hash:?}"
        );
    }
}

/// A native SOL buy pays a wallet the System Program owns and leaves it
/// rent-exempt: an account of another program is rejected, and so is a payout
/// that can leave the wallet under the minimum. A passing order moves on to
/// the blockhash check. The lookup answers the sell mint, nothing for the
/// Clock sysvar, then the wallet.
#[tokio::test]
async fn create_order_checks_the_native_buy_wallet() {
    let funder = solana_sdk::pubkey::Pubkey::new_unique();
    let owner = solana_sdk::signer::keypair::Keypair::new();
    let floor = solana_sdk::rent::Rent::default().minimum_balance(0);
    let system = solana_system_interface::program::ID;
    let wallet = |lamports, owner| {
        Some(solana_sdk::account::Account {
            lamports,
            owner,
            ..Default::default()
        })
    };
    let buy = |buy_amount| native_buy_intent(owner.pubkey(), buy_amount);
    let partial = |buy_amount| {
        let mut intent = buy(buy_amount);
        intent.flags.partially_fillable = true;
        intent
    };
    for (index, (account, intent, expected)) in [
        (
            wallet(2_039_280, spl_token_interface::ID),
            buy(1_000_000_000),
            "InvalidNativeBuy",
        ),
        (
            wallet(2_039_280, system),
            buy(1_000_000_000),
            PASSED_ACCOUNT_CHECKS,
        ),
        (None, buy(floor - 1), "InvalidNativeBuy"),
        (None, partial(1_000_000_000), "InvalidNativeBuy"),
        (
            wallet(1_000, system),
            buy(floor - 1_000),
            PASSED_ACCOUNT_CHECKS,
        ),
        (
            wallet(1_000, system),
            buy(floor - 1_001),
            "InvalidNativeBuy",
        ),
        (wallet(floor, system), partial(1), PASSED_ACCOUNT_CHECKS),
    ]
    .into_iter()
    .enumerate()
    {
        let addr = spawn_sponsored_server_with(
            PgPool::connect_lazy("postgresql://").unwrap(),
            funder,
            sponsored_mocks(
                false,
                accounts_response(&[Some(mint_account(spl_token_interface::ID)), None, account]),
            ),
        )
        .await;
        let transaction = creation_tx(funder, &owner, &intent, vec![], true);
        let (status, kind) = post_order(addr, transaction).await;
        assert_eq!(
            (status, kind.as_str()),
            (reqwest::StatusCode::BAD_REQUEST, expected),
            "case {index}"
        );
    }
}

/// The buy token account must be one the settlement can pay out to as is,
/// or the owner's associated token account under the buy mint's token
/// program, which the settlement creates when missing. A custom receiver's
/// missing account cannot be placed. A passing order moves on to the
/// blockhash check. The lookup answers the sell and buy mints, the Clock
/// sysvar, then the buy token account.
#[tokio::test]
async fn create_order_checks_the_buy_token_account() {
    let funder = solana_sdk::pubkey::Pubkey::new_unique();
    let owner = solana_sdk::signer::keypair::Keypair::new();
    let receiver = solana_sdk::pubkey::Pubkey::new_unique();

    let paid_to_owner = sponsored_intent(owner.pubkey(), false);
    let mint = buy_mint(&paid_to_owner);
    let mut paid_to_receiver = sponsored_intent(owner.pubkey(), false);
    paid_to_receiver.buy = cow_settlement_interface::data::intent::Asset::TokenProgram(
        cow_settlement_interface::data::intent::TokenAsset {
            mint,
            token_account: ata(receiver, mint),
        },
    );

    let receiver_token_account = solana_testlib::token_account_json(&mint, &receiver);
    let lamports_only = solana_testlib::account_json(&solana_sdk::account::Account {
        lamports: solana_sdk::rent::Rent::default().minimum_balance(0),
        owner: solana_system_interface::program::ID,
        ..Default::default()
    });
    // The lookup reads the sell mint, the buy mint, the Clock sysvar, then
    // the buy token account.
    let lookup = |token_program, buy_account: Option<serde_json::Value>| {
        let mint = solana_testlib::account_json(&mint_account(token_program));
        solana_testlib::multiple_accounts_json([
            mint.clone(),
            mint,
            serde_json::Value::Null,
            buy_account.unwrap_or(serde_json::Value::Null),
        ])
    };
    let spl_token = spl_token_interface::ID;
    let token_2022 = spl_token_2022_interface::ID;

    for (name, intent, accounts, expected) in [
        (
            "owner's missing ATA: the settlement creates it",
            &paid_to_owner,
            lookup(spl_token, None),
            PASSED_ACCOUNT_CHECKS,
        ),
        (
            "owner's ATA holding only lamports: the idempotent create allocates over it",
            &paid_to_owner,
            lookup(spl_token, Some(lamports_only)),
            PASSED_ACCOUNT_CHECKS,
        ),
        (
            "owner's SPL Token ATA on a Token-2022 mint: the settlement would create the \
             Token-2022 one",
            &paid_to_owner,
            lookup(token_2022, None),
            "InvalidTransaction",
        ),
        (
            "receiver's missing ATA: nothing can derive whose wallet to create it for",
            &paid_to_receiver,
            lookup(spl_token, None),
            "InvalidTransaction",
        ),
        (
            "receiver's existing token account",
            &paid_to_receiver,
            lookup(spl_token, Some(receiver_token_account)),
            PASSED_ACCOUNT_CHECKS,
        ),
    ] {
        let addr = spawn_sponsored_server_with(
            PgPool::connect_lazy("postgresql://").unwrap(),
            funder,
            sponsored_mocks(false, accounts),
        )
        .await;
        let transaction = creation_tx(funder, &owner, intent, vec![], true);
        let (status, kind) = post_order(addr, transaction).await;
        assert_eq!(
            (status, kind.as_str()),
            (reqwest::StatusCode::BAD_REQUEST, expected),
            "{name}"
        );
    }
}

/// A bundle that creates the buy token account is rejected, whoever pays
/// for it, whichever creation variant and token program it uses, whoever
/// owns the account, and wherever it sits in the template. Validation
/// rejects it, so no RPC probe is consumed.
#[tokio::test]
async fn create_order_rejects_creating_the_buy_token_account() {
    let funder = solana_sdk::pubkey::Pubkey::new_unique();
    let owner = solana_sdk::signer::keypair::Keypair::new();
    let receiver = solana_sdk::pubkey::Pubkey::new_unique();
    let addr =
        spawn_sponsored_server(PgPool::connect_lazy("postgresql://").unwrap(), funder, true).await;
    let owned = sponsored_intent(owner.pubkey(), false);
    let mint = buy_mint(&owned);
    let with_buy_account = |token_account| {
        let mut intent = sponsored_intent(owner.pubkey(), false);
        intent.buy = cow_settlement_interface::data::intent::Asset::TokenProgram(
            cow_settlement_interface::data::intent::TokenAsset {
                mint,
                token_account,
            },
        );
        intent
    };
    let received = with_buy_account(ata(receiver, mint));
    let token_2022 = with_buy_account(
        spl_associated_token_account_interface::address::get_associated_token_address_with_program_id(
            &owner.pubkey(),
            &mint,
            &spl_token_2022_interface::ID,
        ),
    );
    let approve = spl_token_interface::instruction::approve(
        &spl_token_interface::ID,
        &owned.sell.token_account,
        &state_pda(),
        &owner.pubkey(),
        &[],
        owned.sell_amount.get(),
    )
    .unwrap();

    for (index, (intent, preparations)) in [
        // Paid by the funder.
        (&owned, vec![destination_creation(funder, owner.pubkey(), &owned)]),
        // Paid by the owner.
        (
            &owned,
            vec![
                spl_associated_token_account_interface::instruction::create_associated_token_account_idempotent(
                    &owner.pubkey(),
                    &owner.pubkey(),
                    &mint,
                    &spl_token_interface::ID,
                ),
            ],
        ),
        // The non-idempotent creation.
        (
            &owned,
            vec![
                spl_associated_token_account_interface::instruction::create_associated_token_account(
                    &funder,
                    &owner.pubkey(),
                    &mint,
                    &spl_token_interface::ID,
                ),
            ],
        ),
        // Under Token-2022.
        (
            &token_2022,
            vec![
                spl_associated_token_account_interface::instruction::create_associated_token_account_idempotent(
                    &funder,
                    &owner.pubkey(),
                    &mint,
                    &spl_token_2022_interface::ID,
                ),
            ],
        ),
        // For a custom receiver.
        (&received, vec![destination_creation(funder, receiver, &received)]),
        // After a valid delegation.
        (
            &owned,
            vec![
                approve,
                destination_creation(funder, owner.pubkey(), &owned),
            ],
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let transaction = creation_tx(funder, &owner, intent, preparations, true);
        let response = reqwest::Client::new()
            .post(format!("http://{addr}/api/v1/orders"))
            .json(&serde_json::json!({ "partiallySignedTx": transaction }))
            .send()
            .await
            .unwrap();
        let status = response.status();
        let json: serde_json::Value = response.json().await.unwrap();
        assert_eq!(
            (
                status,
                json["errorType"].as_str(),
                json["description"].as_str()
            ),
            (
                reqwest::StatusCode::BAD_REQUEST,
                Some("InvalidTransaction"),
                Some("the bundle must not create the buy token account"),
            ),
            "case {index}"
        );
    }
}

/// A message header that leaves the owner outside the signer region is
/// rejected: on chain the creation would demand the owner's signature, so
/// accepting it would only defer the failure past a won auction.
#[tokio::test]
async fn create_order_requires_the_owner_as_signer() {
    let funder = solana_sdk::pubkey::Pubkey::new_unique();
    let owner = solana_sdk::signer::keypair::Keypair::new();
    let addr =
        spawn_sponsored_server(PgPool::connect_lazy("postgresql://").unwrap(), funder, true).await;
    let intent = sponsored_intent(owner.pubkey(), false);
    let instruction: solana_sdk::instruction::Instruction =
        cow_settlement_client::instruction::CreateOrder {
            program_id: program_id(),
            owner: owner.pubkey(),
            created_by: funder,
            intent: &intent,
        }
        .into();
    let mut message = solana_sdk::message::Message::new_with_blockhash(
        &[instruction],
        Some(&funder),
        &solana_sdk::hash::Hash::new_unique(),
    );
    // Demote the owner out of the signer region.
    message.header.num_required_signatures = 1;
    let tx = solana_sdk::transaction::VersionedTransaction {
        signatures: vec![solana_sdk::signature::Signature::default()],
        message: solana_sdk::message::VersionedMessage::Legacy(message),
    };
    let transaction = base64::prelude::BASE64_STANDARD.encode(bincode::serialize(&tx).unwrap());
    let (status, kind) = post_order(addr, transaction).await;
    assert_eq!(
        (status, kind.as_str()),
        (reqwest::StatusCode::BAD_REQUEST, "InvalidSignature")
    );
}

/// Preparation instructions outside the template, touching the funder, or
/// out of order are rejected. All rejections come from validation, so no
/// database or RPC probe is consumed.
#[tokio::test]
async fn create_order_checks_the_preparation_template() {
    let funder = solana_sdk::pubkey::Pubkey::new_unique();
    let owner_keypair = solana_sdk::signer::keypair::Keypair::new();
    let owner = owner_keypair.pubkey();
    let intent = sponsored_intent(owner, true);
    let addr =
        spawn_sponsored_server(PgPool::connect_lazy("postgresql://").unwrap(), funder, true).await;

    let transfer = |from: solana_sdk::pubkey::Pubkey| {
        solana_system_interface::instruction::transfer(&from, &intent.sell.token_account, 1_000)
    };
    let approve = |delegate: solana_sdk::pubkey::Pubkey| {
        spl_token_interface::instruction::approve(
            &spl_token_interface::ID,
            &intent.sell.token_account,
            &delegate,
            &owner,
            &[],
            1_000,
        )
        .unwrap()
    };

    for (preparations, expected) in [
        // A program outside the template never rides on the funder's fee.
        (
            vec![solana_sdk::instruction::Instruction::new_with_bytes(
                solana_sdk::pubkey::Pubkey::new_unique(),
                &[],
                vec![],
            )],
            "InvalidTransaction",
        ),
        // A transfer draining the funder instead of wrapping the owner's SOL.
        (vec![transfer(funder)], "InvalidTransaction"),
        // A delegation to anyone but the settlement state PDA.
        (
            vec![approve(solana_sdk::pubkey::Pubkey::new_unique())],
            "WrongDelegate",
        ),
        // An account creation for a mint the order does not trade.
        (
            vec![
                spl_associated_token_account_interface::instruction::create_associated_token_account_idempotent(
                    &funder,
                    &owner,
                    &solana_sdk::pubkey::Pubkey::new_unique(),
                    &spl_token_interface::ID,
                ),
            ],
            "InvalidTransaction",
        ),
        // Steps out of template order.
        (
            vec![approve(state_pda()), transfer(owner)],
            "InvalidTransaction",
        ),
        // A step twice.
        (
            vec![approve(state_pda()), approve(state_pda())],
            "InvalidTransaction",
        ),
    ] {
        let transaction = creation_tx(funder, &owner_keypair, &intent, preparations, true);
        let (status, kind) = post_order(addr, transaction).await;
        assert_eq!(
            (status, kind.as_str()),
            (reqwest::StatusCode::BAD_REQUEST, expected)
        );
    }

    // Wrap steps on an order that does not sell native SOL.
    let plain = sponsored_intent(owner, false);
    let wrap =
        solana_system_interface::instruction::transfer(&owner, &plain.sell.token_account, 1_000);
    let transaction = creation_tx(funder, &owner_keypair, &plain, vec![wrap], true);
    let (status, kind) = post_order(addr, transaction).await;
    assert_eq!(
        (status, kind.as_str()),
        (reqwest::StatusCode::BAD_REQUEST, "InvalidTransaction")
    );

    // A native SOL buy has no buy token account to create, not even a wSOL
    // one.
    let native = native_buy_intent(owner, 1_000_000_000);
    let creation =
        spl_associated_token_account_interface::instruction::create_associated_token_account_idempotent(
            &funder,
            &owner,
            &spl_token_interface::native_mint::ID,
            &spl_token_interface::ID,
        );
    let transaction = creation_tx(funder, &owner_keypair, &native, vec![creation], true);
    let (status, kind) = post_order(addr, transaction).await;
    assert_eq!(
        (status, kind.as_str()),
        (reqwest::StatusCode::BAD_REQUEST, "InvalidTransaction")
    );

    // A wrapped sell account owned by a stranger is rejected even when the
    // intent names it.
    let stranger = solana_sdk::pubkey::Pubkey::new_unique();
    let mut foreign_sell = sponsored_intent(owner, true);
    foreign_sell.sell.token_account = ata(stranger, foreign_sell.sell.mint);
    let preparations = vec![
        spl_associated_token_account_interface::instruction::create_associated_token_account_idempotent(
            &funder,
            &stranger,
            &foreign_sell.sell.mint,
            &spl_token_interface::ID,
        ),
    ];
    let transaction = creation_tx(funder, &owner_keypair, &foreign_sell, preparations, true);
    let (status, kind) = post_order(addr, transaction).await;
    assert_eq!(
        (status, kind.as_str()),
        (reqwest::StatusCode::BAD_REQUEST, "InvalidTransaction")
    );
}

/// A native SOL sell whose wSOL account creation the funder pays is refused:
/// nothing charges the rent back, and the owner closes the account after the
/// fill and keeps it.
#[tokio::test]
async fn create_order_rejects_a_funder_paid_sell_account_creation() {
    let funder = solana_sdk::pubkey::Pubkey::new_unique();
    let owner = solana_sdk::signer::keypair::Keypair::new();
    let addr =
        spawn_sponsored_server(PgPool::connect_lazy("postgresql://").unwrap(), funder, true).await;
    let intent = sponsored_intent(owner.pubkey(), true);
    let mut preparations = full_preparations(owner.pubkey(), &intent);
    preparations[0] = spl_associated_token_account_interface::instruction::create_associated_token_account_idempotent(
        &funder,
        &owner.pubkey(),
        &intent.sell.mint,
        &spl_token_interface::ID,
    );
    let transaction = creation_tx(funder, &owner, &intent, preparations, true);
    let (status, kind) = post_order(addr, transaction).await;
    assert_eq!(
        (status, kind.as_str()),
        (reqwest::StatusCode::BAD_REQUEST, "InvalidTransaction")
    );
}

/// Placement reads the order's mints: one the settlement program cannot move
/// answers `UnsupportedToken`, and a delegation through a token program other
/// than the mint's owner is an invalid transaction. Every case gets a fresh
/// server, since the mock answers the mint lookup once.
#[tokio::test]
async fn create_order_checks_the_mints_on_chain() {
    let funder = solana_sdk::pubkey::Pubkey::new_unique();
    let owner = solana_sdk::signer::keypair::Keypair::new();
    let intent = sponsored_intent(owner.pubkey(), false);
    let classic = Some(mint_account(spl_token_interface::ID));
    // No preparation steps.
    let plain = sponsored_creation_tx(funder, &owner, true);
    let token_2022_approve = creation_tx(
        funder,
        &owner,
        &intent,
        vec![
            spl_token_2022_interface::instruction::approve(
                &spl_token_2022_interface::ID,
                &intent.sell.token_account,
                &state_pda(),
                &owner.pubkey(),
                &[],
                1_000,
            )
            .unwrap(),
        ],
        true,
    );

    for (transaction, mints, expected) in [
        // The sell mint does not exist.
        (plain, [None, classic.clone()], "UnsupportedToken"),
        // The sell mint lives under SPL Token but its delegation goes
        // through Token-2022.
        (
            token_2022_approve,
            [classic.clone(), classic],
            "InvalidTransaction",
        ),
    ] {
        let addr = spawn_sponsored_server_with(
            PgPool::connect_lazy("postgresql://").unwrap(),
            funder,
            sponsored_mocks(true, accounts_response(&mints)),
        )
        .await;
        let (status, kind) = post_order(addr, transaction).await;
        assert_eq!(
            (status, kind.as_str()),
            (reqwest::StatusCode::BAD_REQUEST, expected)
        );
    }
}

/// The happy path lands the order and the duplicate is rejected.
#[tokio::test]
#[ignore = "needs the solana.* schema applied to the local database"]
async fn solana_db_create_order_persists_a_sponsored_order() {
    let pool = PgPool::connect("postgresql://").await.unwrap();
    sqlx::query(
        "TRUNCATE solana.order_pda, solana.orders, solana.order_quotes, solana.order_events \
         CASCADE",
    )
    .execute(&pool)
    .await
    .unwrap();
    let funder = solana_sdk::pubkey::Pubkey::new_unique();
    let owner = solana_sdk::signer::keypair::Keypair::new();
    let addr = spawn_sponsored_server(pool.clone(), funder, true).await;
    // The full preparation prefix in front of `CreateOrder`, as the frontend
    // sends it for a first-time native-SOL sell.
    let intent = sponsored_intent(owner.pubkey(), true);
    // Wrapped the way Phantom sends it: compute budget and Lighthouse
    // assertions before our instructions, and another assertion after.
    let mut preparations = vec![
        solana_compute_budget_interface::ComputeBudgetInstruction::set_compute_unit_price(10_000),
        // Discriminator 6 is `AssertAccountInfoMulti`.
        lighthouse(6, owner.pubkey()),
        lighthouse(6, funder),
    ];
    preparations.extend(full_preparations(owner.pubkey(), &intent));
    let transaction = creation_tx_wrapped(
        funder,
        &owner,
        &intent,
        preparations,
        vec![
            solana_compute_budget_interface::ComputeBudgetInstruction::set_compute_unit_limit(
                12_769,
            ),
            // Discriminator 10 is `AssertTokenAccountMulti`.
            lighthouse(10, owner.pubkey()),
        ],
        true,
    );
    let quote_id = db::save_quote(
        &pool,
        &db::Quote {
            sell_token: ByteArray(intent.sell.mint.to_bytes()),
            buy_token: ByteArray(intent.buy.encode().0.to_bytes()),
            sell_amount: intent.sell_amount.get(),
            buy_amount: intent.buy_amount.get(),
            kind: OrderKind::Sell,
            solver: ByteArray([0xDD; 32]),
            expiration: chrono::Utc::now() + chrono::Duration::seconds(60),
        },
    )
    .await
    .unwrap();

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/api/v1/orders"))
        .json(&serde_json::json!({ "partiallySignedTx": transaction.clone(), "quoteId": quote_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::CREATED);
    let uid: String = response.json().await.unwrap();
    assert!(uid.starts_with("0x") && uid.len() == 66);

    let (stored, expiry): (Vec<u8>, i64) =
        sqlx::query_as("SELECT presigned_transaction, last_valid_block_height FROM solana.orders")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!stored.is_empty());
    // Mocked height 100 plus the maximum blockhash age.
    assert_eq!(expiry, 250);
    let linked: (Vec<u8>, Option<i64>, String) =
        sqlx::query_as("SELECT order_uid, quote_id, sell_amount::text FROM solana.order_quotes")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(linked.0, const_hex::decode(&uid[2..]).unwrap());
    assert_eq!(linked.1, Some(quote_id));
    assert_eq!(linked.2, intent.sell_amount.to_string());

    // The duplicate check runs after the RPC probes and the mock answers
    // each probe once, so the duplicate goes through a fresh server over the
    // same database.
    let addr = spawn_sponsored_server(pool.clone(), funder, true).await;
    let (status, kind) = post_order(addr, transaction).await;
    assert_eq!(
        (status, kind.as_str()),
        (reqwest::StatusCode::BAD_REQUEST, "DuplicatedOrder")
    );

    // A named quote that does not match the order (wrong pair here) is
    // dropped: the order lands unlinked.
    let addr = spawn_sponsored_server(pool.clone(), funder, true).await;
    let other_owner = solana_sdk::signer::keypair::Keypair::new();
    let other = sponsored_intent(other_owner.pubkey(), false);
    let mismatched = db::save_quote(
        &pool,
        &db::Quote {
            sell_token: ByteArray([0x99; 32]),
            buy_token: ByteArray(other.buy.encode().0.to_bytes()),
            sell_amount: other.sell_amount.get(),
            buy_amount: other.buy_amount.get(),
            kind: OrderKind::Sell,
            solver: ByteArray([0xDD; 32]),
            expiration: chrono::Utc::now() + chrono::Duration::seconds(60),
        },
    )
    .await
    .unwrap();
    let transaction = creation_tx(funder, &other_owner, &other, vec![], true);
    let response = reqwest::Client::new()
        .post(format!("http://{addr}/api/v1/orders"))
        .json(&serde_json::json!({ "partiallySignedTx": transaction, "quoteId": mismatched }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::CREATED);
    let copies: i64 = sqlx::query_scalar("SELECT count(*) FROM solana.order_quotes")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(copies, 1, "the mismatched quote must not be copied");

    // An expired quote is dropped even when everything else matches.
    let addr = spawn_sponsored_server(pool.clone(), funder, true).await;
    let late_owner = solana_sdk::signer::keypair::Keypair::new();
    let late = sponsored_intent(late_owner.pubkey(), false);
    let expired = db::save_quote(
        &pool,
        &db::Quote {
            sell_token: ByteArray(late.sell.mint.to_bytes()),
            buy_token: ByteArray(late.buy.encode().0.to_bytes()),
            sell_amount: late.sell_amount.get(),
            buy_amount: late.buy_amount.get(),
            kind: OrderKind::Sell,
            solver: ByteArray([0xDD; 32]),
            expiration: chrono::Utc::now() - chrono::Duration::seconds(1),
        },
    )
    .await
    .unwrap();
    let transaction = creation_tx(funder, &late_owner, &late, vec![], true);
    let response = reqwest::Client::new()
        .post(format!("http://{addr}/api/v1/orders"))
        .json(&serde_json::json!({ "partiallySignedTx": transaction, "quoteId": expired }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::CREATED);
    let copies: i64 = sqlx::query_scalar("SELECT count(*) FROM solana.order_quotes")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(copies, 1, "the expired quote must not be copied");
}

/// The buy account may belong to a wallet other than the owner: the
/// receiver's existing account receives the payout and the order stores it.
/// The lookup answers the sell and buy mints, the Clock sysvar, then the
/// receiver's account.
#[tokio::test]
#[ignore = "needs the solana.* schema applied to the local database"]
async fn solana_db_create_order_accepts_a_custom_receiver() {
    let pool = PgPool::connect("postgresql://").await.unwrap();
    sqlx::query(
        "TRUNCATE solana.order_pda, solana.orders, solana.order_quotes, solana.order_events \
         CASCADE",
    )
    .execute(&pool)
    .await
    .unwrap();
    let funder = solana_sdk::pubkey::Pubkey::new_unique();
    let owner = solana_sdk::signer::keypair::Keypair::new();
    let receiver = solana_sdk::pubkey::Pubkey::new_unique();
    let mut intent = sponsored_intent(owner.pubkey(), false);
    let buy_mint = intent.buy.encode().0;
    intent.buy = cow_settlement_interface::data::intent::Asset::TokenProgram(
        cow_settlement_interface::data::intent::TokenAsset {
            mint: buy_mint,
            token_account: ata(receiver, buy_mint),
        },
    );
    let transaction = creation_tx(funder, &owner, &intent, vec![], true);
    let classic = solana_testlib::account_json(&mint_account(spl_token_interface::ID));
    let addr = spawn_sponsored_server_with(
        pool.clone(),
        funder,
        sponsored_mocks(
            true,
            solana_testlib::multiple_accounts_json([
                classic.clone(),
                classic,
                serde_json::Value::Null,
                solana_testlib::token_account_json(&buy_mint, &receiver),
            ]),
        ),
    )
    .await;

    let (status, _) = post_order(addr, transaction).await;
    assert_eq!(status, reqwest::StatusCode::CREATED);
    let stored: Vec<u8> = sqlx::query_scalar("SELECT buy_token_account FROM solana.orders")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored, intent.buy.encode().1.to_bytes().to_vec());
}

/// Token-2022 mints place like classic ones: the delegation goes through
/// Token-2022, and the buy account is the owner's missing Token-2022
/// associated token account, which the settlement creates. The lookup
/// answers the sell and buy mints, the Clock sysvar, then the buy token
/// account.
#[tokio::test]
#[ignore = "needs the solana.* schema applied to the local database"]
async fn solana_db_create_order_accepts_token_2022_mints() {
    let pool = PgPool::connect("postgresql://").await.unwrap();
    sqlx::query(
        "TRUNCATE solana.order_pda, solana.orders, solana.order_quotes, solana.order_events \
         CASCADE",
    )
    .execute(&pool)
    .await
    .unwrap();
    let funder = solana_sdk::pubkey::Pubkey::new_unique();
    let owner = solana_sdk::signer::keypair::Keypair::new();
    let mut intent = sponsored_intent(owner.pubkey(), false);
    let buy_mint = intent.buy.encode().0;
    intent.buy = cow_settlement_interface::data::intent::Asset::TokenProgram(
        cow_settlement_interface::data::intent::TokenAsset {
            mint: buy_mint,
            token_account:
                spl_associated_token_account_interface::address::get_associated_token_address_with_program_id(
                    &owner.pubkey(),
                    &buy_mint,
                    &spl_token_2022_interface::ID,
                ),
        },
    );
    let preparations = vec![
        spl_token_2022_interface::instruction::approve(
            &spl_token_2022_interface::ID,
            &intent.sell.token_account,
            &state_pda(),
            &owner.pubkey(),
            &[],
            intent.sell_amount.get(),
        )
        .unwrap(),
    ];
    let transaction = creation_tx(funder, &owner, &intent, preparations, true);
    let token_2022 = Some(mint_account(spl_token_2022_interface::ID));
    let addr = spawn_sponsored_server_with(
        pool.clone(),
        funder,
        sponsored_mocks(
            true,
            accounts_response(&[token_2022.clone(), token_2022, None, None]),
        ),
    )
    .await;

    let (status, _) = post_order(addr, transaction).await;
    assert_eq!(status, reqwest::StatusCode::CREATED);
    let stored: Vec<u8> = sqlx::query_scalar("SELECT buy_token_account FROM solana.orders")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored, intent.buy.encode().1.to_bytes().to_vec());
}

/// A native SOL buy lands without a buy account creation: the payout goes
/// to the wallet itself, and creates it when the lookup finds none. The
/// lookup answers the sell mint, then nothing for the Clock sysvar and the
/// wallet.
#[tokio::test]
#[ignore = "needs the solana.* schema applied to the local database"]
async fn solana_db_create_order_accepts_a_native_buy() {
    let pool = PgPool::connect("postgresql://").await.unwrap();
    sqlx::query(
        "TRUNCATE solana.order_pda, solana.orders, solana.order_quotes, solana.order_events \
         CASCADE",
    )
    .execute(&pool)
    .await
    .unwrap();
    let funder = solana_sdk::pubkey::Pubkey::new_unique();
    let owner = solana_sdk::signer::keypair::Keypair::new();
    let intent = native_buy_intent(owner.pubkey(), 890_880);
    let transaction = creation_tx(funder, &owner, &intent, vec![], true);
    let addr = spawn_sponsored_server_with(
        pool.clone(),
        funder,
        sponsored_mocks(
            true,
            accounts_response(&[Some(mint_account(spl_token_interface::ID)), None, None]),
        ),
    )
    .await;

    let (status, _) = post_order(addr, transaction).await;
    assert_eq!(status, reqwest::StatusCode::CREATED);
    let (buy_token, buy_token_account): (Vec<u8>, Vec<u8>) =
        sqlx::query_as("SELECT buy_token, buy_token_account FROM solana.orders")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(buy_token, [0; 32]);
    assert_eq!(buy_token_account, owner.pubkey().to_bytes());
}

/// The document a placement carries is stored with the order: the order
/// reports it as `fullAppData`, and the app data endpoint serves it by hash.
/// A later placement names the hash alone, a hash nothing is stored under is
/// refused.
#[tokio::test]
#[ignore = "needs the solana.* schema applied to the local database"]
async fn solana_db_create_order_stores_the_app_data() {
    let pool = PgPool::connect("postgresql://").await.unwrap();
    sqlx::query(
        "TRUNCATE solana.order_pda, solana.orders, solana.order_quotes, solana.order_events, \
         solana.app_data CASCADE",
    )
    .execute(&pool)
    .await
    .unwrap();
    let funder = solana_sdk::pubkey::Pubkey::new_unique();
    let owner = solana_sdk::signer::keypair::Keypair::new();
    let addr = spawn_sponsored_server(pool.clone(), funder, true).await;
    let document = r#"{"appCode":"CoW Swap","metadata":{"quote":{"slippageBips":50}}}"#;
    let hash = app_data::hash_full_app_data(document.as_bytes());
    let hash_hex = const_hex::encode_prefixed(hash);
    let mut intent = sponsored_intent(owner.pubkey(), false);
    intent.app_data = hash;
    let transaction = creation_tx(funder, &owner, &intent, vec![], true);

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/api/v1/orders"))
        .json(&serde_json::json!({
            "partiallySignedTx": transaction,
            "appData": document,
            "appDataHash": hash_hex,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::CREATED);
    let uid: String = response.json().await.unwrap();

    let order = get_order(addr, &uid).await;
    assert_eq!(order["appData"], hash_hex);
    assert_eq!(order["fullAppData"], document);
    let response = reqwest::get(format!("http://{addr}/api/v1/app_data/{hash_hex}"))
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let registered: serde_json::Value = response.json().await.unwrap();
    assert_eq!(registered, serde_json::json!({ "fullAppData": document }));

    // Each sponsored server answers its chain probes once.
    intent.valid_to += 1;
    let transaction = creation_tx(funder, &owner, &intent, vec![], true);
    let addr = spawn_sponsored_server(pool.clone(), funder, true).await;
    let response = reqwest::Client::new()
        .post(format!("http://{addr}/api/v1/orders"))
        .json(&serde_json::json!({ "partiallySignedTx": transaction, "appData": hash_hex }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::CREATED);
    let uid: String = response.json().await.unwrap();
    assert_eq!(get_order(addr, &uid).await["fullAppData"], document);

    intent.app_data = [0x77; 32];
    let transaction = creation_tx(funder, &owner, &intent, vec![], true);
    let addr = spawn_sponsored_server(pool, funder, true).await;
    for body in [
        serde_json::json!({ "partiallySignedTx": transaction }),
        serde_json::json!({
            "partiallySignedTx": transaction,
            "appData": const_hex::encode_prefixed([0x77; 32]),
        }),
    ] {
        let (status, kind) = post_order_body(addr, body).await;
        assert_eq!(
            (status, kind.as_str()),
            (reqwest::StatusCode::BAD_REQUEST, "InvalidAppData")
        );
    }
}

async fn get_order(addr: SocketAddr, uid: &str) -> serde_json::Value {
    let response = reqwest::get(format!("http://{addr}/api/v1/orders/{uid}"))
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    response.json().await.unwrap()
}

async fn get_status(addr: SocketAddr, uid: &str) -> serde_json::Value {
    let response = reqwest::get(format!("http://{addr}/api/v1/orders/{uid}/status"))
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    response.json().await.unwrap()
}

/// A pending sponsored creation reads as expired once the chain passes its
/// block height. The sponsored server's mock answers `getBlockHeight` with
/// 100, once, so every read goes through a fresh server.
#[tokio::test]
#[ignore = "needs the solana.* schema applied to the local database"]
async fn solana_db_orders_report_a_dead_creation_as_expired() {
    let pool = PgPool::connect("postgresql://").await.unwrap();
    sqlx::query(
        "TRUNCATE solana.order_pda, solana.orders, solana.order_quotes, solana.order_events, \
         solana.trades CASCADE",
    )
    .execute(&pool)
    .await
    .unwrap();
    let owner = solana_sdk::pubkey::Pubkey::new_unique();
    let order = db::SponsoredOrder {
        uid: ByteArray([0x11; 32]),
        owner: ByteArray(owner.to_bytes()),
        sell_token: ByteArray([0x66; 32]),
        buy_token: ByteArray([0x55; 32]),
        sell_token_account: ByteArray([0x33; 32]),
        buy_token_account: ByteArray([0x22; 32]),
        sell_amount: 1_000,
        buy_amount: 2_000,
        valid_to: u32::MAX,
        kind: OrderKind::Sell,
        partially_fillable: false,
        app_data: ByteArray([0x44; 32]),
        order_pda: ByteArray([0xB0; 32]),
        presigned_transaction: vec![0xC0; 128],
        last_valid_block_height: 100,
    };
    db::insert_sponsored_order(&pool, &order, None, "{}")
        .await
        .unwrap();
    let funder = solana_sdk::pubkey::Pubkey::new_unique();
    let uid = format!("0x{}", "11".repeat(32));

    // Alive at the deadline.
    let addr = spawn_sponsored_server(pool.clone(), funder, true).await;
    let json = get_order(addr, &uid).await;
    assert_eq!(json["status"], "open");
    assert_eq!(json["lastValidBlockHeight"], 100);
    let addr = spawn_sponsored_server(pool.clone(), funder, true).await;
    assert_eq!(get_status(addr, &uid).await["type"], "scheduled");

    sqlx::query("UPDATE solana.orders SET last_valid_block_height = 99")
        .execute(&pool)
        .await
        .unwrap();
    let addr = spawn_sponsored_server(pool.clone(), funder, true).await;
    let json = get_order(addr, &uid).await;
    assert_eq!(json["status"], "expired");

    // An `invalid` event alone reads as `open`, expiry outranks it.
    sqlx::query(
        "INSERT INTO solana.order_events (order_uid, timestamp, label) VALUES ($1, now(), $2)",
    )
    .bind(order.uid)
    .bind(OrderEventLabel::Invalid)
    .execute(&pool)
    .await
    .unwrap();
    let addr = spawn_sponsored_server(pool.clone(), funder, true).await;
    assert_eq!(get_status(addr, &uid).await["type"], "expired");

    let addr = spawn_sponsored_server(pool.clone(), funder, true).await;
    let listed: Vec<serde_json::Value> =
        reqwest::get(format!("http://{addr}/api/v1/account/{owner}/orders"))
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0]["status"], "expired");
}

/// Spawn the API server over `pool`, without sponsoring.
async fn spawn_server_on(pool: PgPool) -> SocketAddr {
    let api = Api {
        pool,
        ..mock_api().await
    };
    let (listener, addr) = api.bind().await.unwrap();
    let shutdown = CancellationToken::new();
    tokio::spawn(async move { api.serve(listener, shutdown).await.unwrap() });
    addr
}

async fn put_app_data(
    addr: SocketAddr,
    path: &str,
    document: &str,
) -> (reqwest::StatusCode, serde_json::Value) {
    let response = reqwest::Client::new()
        .put(format!("http://{addr}{path}"))
        .json(&serde_json::json!({ "fullAppData": document }))
        .send()
        .await
        .unwrap();
    let status = response.status();
    (status, response.json().await.unwrap())
}

/// The app data endpoints validate before touching the database: a document
/// that is not a JSON object or over the size limit, a hash the document
/// does not hash to, and a malformed hash are refused without a live pool.
#[tokio::test]
async fn app_data_rejects_invalid_uploads() {
    let addr = spawn_server().await;
    let oversized = format!(r#"{{"a":"{}"}}"#, "x".repeat(8192));
    let other = format!("/api/v1/app_data/0x{}", "11".repeat(32));
    for (path, document, expected) in [
        ("/api/v1/app_data", "not json", "AppDataInvalid"),
        ("/api/v1/app_data", "[]", "AppDataInvalid"),
        ("/api/v1/app_data", oversized.as_str(), "AppDataInvalid"),
        (other.as_str(), "{}", "AppDataHashMismatch"),
        ("/api/v1/app_data/0x11", "{}", "InvalidAppDataHash"),
    ] {
        let (status, json) = put_app_data(addr, path, document).await;
        assert_eq!(
            (status, json["errorType"].as_str()),
            (reqwest::StatusCode::BAD_REQUEST, Some(expected)),
            "{path} {document}"
        );
    }
    let response = reqwest::get(format!("http://{addr}/api/v1/app_data/zz"))
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
    let json: serde_json::Value = response.json().await.unwrap();
    assert_eq!(json["errorType"], "InvalidAppDataHash");
}

/// A new document answers `201` with its hash, the same document again
/// `200`, with or without the hash in the path, and the document reads back
/// by its hash. A hash another document is stored under refuses the upload.
#[tokio::test]
#[ignore = "needs the solana.* schema applied to the local database"]
async fn solana_db_app_data_round_trips() {
    let pool = PgPool::connect("postgresql://").await.unwrap();
    sqlx::query("TRUNCATE solana.app_data")
        .execute(&pool)
        .await
        .unwrap();
    let addr = spawn_server_on(pool.clone()).await;
    let document = r#"{"appCode":"CoW Swap","metadata":{}}"#;
    let hash = const_hex::encode_prefixed(app_data::hash_full_app_data(document.as_bytes()));
    let by_hash = format!("/api/v1/app_data/{hash}");

    let (status, json) = put_app_data(addr, "/api/v1/app_data", document).await;
    assert_eq!(
        (status, &json),
        (reqwest::StatusCode::CREATED, &serde_json::json!(hash))
    );
    let (status, json) = put_app_data(addr, "/api/v1/app_data", document).await;
    assert_eq!(
        (status, &json),
        (reqwest::StatusCode::OK, &serde_json::json!(hash))
    );
    let (status, json) = put_app_data(addr, &by_hash, document).await;
    assert_eq!(
        (status, &json),
        (reqwest::StatusCode::OK, &serde_json::json!(hash))
    );

    let response = reqwest::get(format!("http://{addr}{by_hash}"))
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let json: serde_json::Value = response.json().await.unwrap();
    assert_eq!(json, serde_json::json!({ "fullAppData": document }));
    let response = reqwest::get(format!(
        "http://{addr}/api/v1/app_data/0x{}",
        "22".repeat(32)
    ))
    .await
    .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::NOT_FOUND);

    // A quote resolves a hash to the stored document and refuses an unknown
    // one, like a placement does.
    let mut body = quote_body(serde_json::json!({"validFor": 1800}));
    body["appData"] = serde_json::json!(hash);
    let (status, kind) = post_quote(addr, body).await;
    assert_eq!(
        (status, kind.as_str()),
        (reqwest::StatusCode::NOT_FOUND, "NoLiquidity")
    );
    let mut body = quote_body(serde_json::json!({"validFor": 1800}));
    body["appData"] = serde_json::json!(format!("0x{}", "22".repeat(32)));
    let (status, kind) = post_quote(addr, body).await;
    assert_eq!(
        (status, kind.as_str()),
        (reqwest::StatusCode::BAD_REQUEST, "InvalidAppData")
    );

    // Only a keccak256 collision reaches this reply, so the row is seeded.
    sqlx::query("INSERT INTO solana.app_data (contract_app_data, full_app_data) VALUES ($1, $2)")
        .bind(ByteArray(app_data::hash_full_app_data(b"{}")))
        .bind(br#"{"a":1}"#.as_slice())
        .execute(&pool)
        .await
        .unwrap();
    let (status, json) = put_app_data(
        addr,
        &format!("/api/v1/app_data/{EMPTY_APP_DATA_HASH}"),
        "{}",
    )
    .await;
    assert_eq!(status, reqwest::StatusCode::BAD_REQUEST);
    assert_eq!(json["errorType"], "AppDataMismatch");
}
