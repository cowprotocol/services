//! Integration tests for the HTTP API server.

use {
    base64::Engine,
    cow_solana_rpc::{Mocks, RpcRequest, SolanaRPC},
    solana_orderbook::infra::{api::Api, quoter::Quoter},
    solana_sdk::signer::Signer,
    sqlx::PgPool,
    std::{net::SocketAddr, time::Duration},
    tokio_util::sync::CancellationToken,
};

fn mock_api() -> Api {
    Api {
        addr: "0.0.0.0:0".parse().unwrap(),
        // A lazy pool never connects unless queried, and `/healthz` does not
        // query, so the tests run without a database.
        pool: PgPool::connect_lazy("postgresql://").unwrap(),
        quoter: dead_quoter(),
        validation: Default::default(),
        quote_expiry: Duration::from_secs(60),
        sponsoring: None,
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
        ..mock_api()
    };
    let (listener, addr) = api.bind().await.unwrap();
    // A token that is never cancelled keeps the server alive for the test.
    let shutdown = CancellationToken::new();
    tokio::spawn(async move { api.serve(listener, shutdown).await.unwrap() });
    addr
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

#[tokio::test]
async fn shuts_down_cleanly_on_signal() {
    let api = mock_api();
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
    let mut zero_amount = quote_body(serde_json::json!({"validFor": 1800}));
    zero_amount["sellAmountBeforeFee"] = serde_json::json!("0");

    for (body, expected) in [
        (
            quote_body(serde_json::json!({"validFor": 10})),
            "InsufficientValidTo",
        ),
        (
            quote_body(serde_json::json!({"validFor": 4 * 60 * 60})),
            "ExcessiveValidTo",
        ),
        (same_tokens, "SameBuyAndSellToken"),
        (zero_amount, "ZeroAmount"),
    ] {
        let (status, kind) = post_quote(addr, body).await;
        assert_eq!(
            (status, kind.as_str()),
            (reqwest::StatusCode::BAD_REQUEST, expected)
        );
    }
}

/// The full happy path: the driver's amounts come back in the EVM response
/// shape with the request's own fields echoed.
#[tokio::test]
async fn quote_answers_in_the_evm_shape() {
    let driver = spawn_mock_driver(serde_json::json!({
        "sellAmount": "10000000",
        "buyAmount": "1234567",
        "solver": "9VXC6LH9eXMBpXLQnxMYAGkjs59Zon2ACciJwQ6iMzNB",
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
    body["appData"] = serde_json::json!(format!("0x{}", "11".repeat(32)));

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
                "feeAmount": "0",
                "kind": "sell",
                "partiallyFillable": false,
            },
            "from": body["from"],
            "expiration": json["expiration"],
            "id": null,
            "verified": false,
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
}

/// With several drivers configured, the best answer wins: the largest buy
/// amount for a sell order.
#[tokio::test]
async fn quote_picks_the_best_driver_answer() {
    let worse = spawn_mock_driver(serde_json::json!({
        "sellAmount": "10000000",
        "buyAmount": "1500000",
        "solver": "9VXC6LH9eXMBpXLQnxMYAGkjs59Zon2ACciJwQ6iMzNB",
    }))
    .await;
    let better = spawn_mock_driver(serde_json::json!({
        "sellAmount": "10000000",
        "buyAmount": "2000000",
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
    assert_eq!(json["quote"]["buyAmount"], "2000000");
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
/// height probes.
async fn spawn_sponsored_server(
    pool: PgPool,
    funder: solana_sdk::pubkey::Pubkey,
    blockhash_valid: bool,
) -> SocketAddr {
    let mocks = Mocks::from([
        (
            RpcRequest::IsBlockhashValid,
            serde_json::json!({
                "context": { "slot": 1u64, "apiVersion": "2.0.0" },
                "value": blockhash_valid,
            }),
        ),
        (RpcRequest::GetBlockHeight, serde_json::json!(100u64)),
    ]);
    let api = Api {
        pool,
        sponsoring: Some(solana_orderbook::infra::api::Sponsoring {
            funder,
            settlement_program: cow_settlement_interface::id(),
            rpc: SolanaRPC::new_mock_with_mocks(mocks),
        }),
        ..mock_api()
    };
    let (listener, addr) = api.bind().await.unwrap();
    let shutdown = CancellationToken::new();
    tokio::spawn(async move { api.serve(listener, shutdown).await.unwrap() });
    addr
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
        buy_token_account,
        sell_token_account,
        buy_mint,
        sell_mint,
        sell_amount: 1_000,
        buy_amount: 2_000,
        valid_to: u32::MAX,
        flags: cow_settlement_interface::data::intent::Flags {
            created_on_chain: true,
            kind: cow_settlement_interface::data::intent::OrderKind::Sell,
            partially_fillable: false,
        },
        app_data: [0x44; 32],
    }
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

/// The settlement state PDA delegations must target.
fn state_pda() -> solana_sdk::pubkey::Pubkey {
    cow_settlement_interface::pda::state::find_state_pda(&cow_settlement_interface::id()).0
}

/// The full whitelisted preparation prefix for a native-sell intent: create
/// the wSOL account, fund it, sync it, delegate it, create the buy account.
fn full_preparations(
    funder: solana_sdk::pubkey::Pubkey,
    owner: solana_sdk::pubkey::Pubkey,
    intent: &cow_settlement_interface::data::intent::OrderIntent,
) -> Vec<solana_sdk::instruction::Instruction> {
    vec![
        spl_associated_token_account_interface::instruction::create_associated_token_account_idempotent(
            &funder,
            &owner,
            &intent.sell_mint,
            &spl_token_interface::ID,
        ),
        solana_system_interface::instruction::transfer(&owner, &intent.sell_token_account, 1_000),
        spl_token_interface::instruction::sync_native(
            &spl_token_interface::ID,
            &intent.sell_token_account,
        )
        .unwrap(),
        spl_token_interface::instruction::approve(
            &spl_token_interface::ID,
            &intent.sell_token_account,
            &state_pda(),
            &owner,
            &[],
            1_000,
        )
        .unwrap(),
        spl_associated_token_account_interface::instruction::create_associated_token_account(
            &funder,
            &owner,
            &intent.buy_mint,
            &spl_token_interface::ID,
        ),
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
    let mut instructions = preparations;
    instructions.push(
        cow_settlement_client::instructions::CreateOrder {
            program_id: cow_settlement_interface::id(),
            owner: owner.pubkey(),
            created_by: funder,
            intent,
        }
        .into(),
    );
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

/// The mandatory buy-account creation step for the intent.
fn destination_creation(
    funder: solana_sdk::pubkey::Pubkey,
    owner: solana_sdk::pubkey::Pubkey,
    intent: &cow_settlement_interface::data::intent::OrderIntent,
) -> solana_sdk::instruction::Instruction {
    spl_associated_token_account_interface::instruction::create_associated_token_account_idempotent(
        &funder,
        &owner,
        &intent.buy_mint,
        &spl_token_interface::ID,
    )
}

/// A sponsored creation transaction with an arbitrary SPL sell and only the
/// mandatory buy-account creation in front of `CreateOrder`.
fn sponsored_creation_tx(
    funder: solana_sdk::pubkey::Pubkey,
    owner: &solana_sdk::signer::keypair::Keypair,
    sign: bool,
) -> String {
    let intent = sponsored_intent(owner.pubkey(), false);
    let destination = destination_creation(funder, owner.pubkey(), &intent);
    creation_tx(funder, owner, &intent, vec![destination], sign)
}

async fn post_order(addr: SocketAddr, transaction: String) -> (reqwest::StatusCode, String) {
    let response = reqwest::Client::new()
        .post(format!("http://{addr}/api/v1/orders"))
        .json(&serde_json::json!({ "transaction": transaction }))
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
    ] {
        let (status, kind) = post_order(addr, transaction).await;
        assert_eq!(
            (status, kind.as_str()),
            (reqwest::StatusCode::BAD_REQUEST, expected)
        );
    }

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
        solana_system_interface::instruction::transfer(&from, &intent.sell_token_account, 1_000)
    };
    let approve = |delegate: solana_sdk::pubkey::Pubkey| {
        spl_token_interface::instruction::approve(
            &spl_token_interface::ID,
            &intent.sell_token_account,
            &delegate,
            &owner,
            &[],
            1_000,
        )
        .unwrap()
    };

    for (preparations, expected) in [
        // The buy-account creation is mandatory.
        (vec![], "InvalidTransaction"),
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
        solana_system_interface::instruction::transfer(&owner, &plain.sell_token_account, 1_000);
    let transaction = creation_tx(funder, &owner_keypair, &plain, vec![wrap], true);
    let (status, kind) = post_order(addr, transaction).await;
    assert_eq!(
        (status, kind.as_str()),
        (reqwest::StatusCode::BAD_REQUEST, "InvalidTransaction")
    );
}

/// The happy path lands the order and the duplicate is rejected.
#[tokio::test]
#[ignore = "needs the solana.* schema applied to the local database"]
async fn solana_db_create_order_persists_a_sponsored_order() {
    let pool = PgPool::connect("postgresql://").await.unwrap();
    sqlx::query("TRUNCATE solana.order_pda, solana.orders, solana.order_events CASCADE")
        .execute(&pool)
        .await
        .unwrap();
    let funder = solana_sdk::pubkey::Pubkey::new_unique();
    let owner = solana_sdk::signer::keypair::Keypair::new();
    let addr = spawn_sponsored_server(pool.clone(), funder, true).await;
    // The full preparation prefix in front of `CreateOrder`, as the frontend
    // sends it for a first-time native-SOL sell.
    let intent = sponsored_intent(owner.pubkey(), true);
    let preparations = full_preparations(funder, owner.pubkey(), &intent);
    let transaction = creation_tx(funder, &owner, &intent, preparations, true);

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/api/v1/orders"))
        .json(&serde_json::json!({ "transaction": transaction.clone() }))
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

    // The mock RPC answers each probe once, so the duplicate goes through a
    // fresh server over the same database.
    let addr = spawn_sponsored_server(pool.clone(), funder, true).await;
    let (status, kind) = post_order(addr, transaction).await;
    assert_eq!(
        (status, kind.as_str()),
        (reqwest::StatusCode::BAD_REQUEST, "DuplicatedOrder")
    );
}
