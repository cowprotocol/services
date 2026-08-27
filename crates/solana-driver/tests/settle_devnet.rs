//! Live integration test on Solana devnet: create fake tokens, create an order,
//! feed it through the driver + a mock solver, and assert the settlement
//! transaction lands.
//!
//! The mock solver returns a single SPL `Transfer` instruction that moves buy
//! tokens from the solver's own ATA into the buy-mint buffer PDA.
//!
//! The keypair defined in the configuration file fills all the roles for this
//! test:
//! - creates and mints both fake tokens
//! - creates the order (as the user/owner), and
//! - settles it (as the solver).
//!
//! ```text
//! cargo nextest run -p solana-driver --run-ignored ignored-only --test settle_devnet --nocapture
//! ```

use {
    base64::Engine,
    cow_settlement_client::{
        cow_settlement_interface::{
            ID as PROGRAM_ID,
            Pubkey,
            data::intent::{OrderIntent, OrderKind},
            pda::{buffer::find_buffer_pda, order::find_order_pda, state::find_state_pda},
        },
        instructions::CreateOrder,
    },
    cow_solana_rpc::{CommitmentConfig, SolanaRPC},
    observe::tracing::init::initialize_reentrant,
    solana_driver::{
        domain::{Auction, Id, Order, Side, Slot, order_uid::OrderUid},
        infra::{api::Api, blockchain::Solana, config, solver::Solver},
    },
    solana_rpc_client::nonblocking::rpc_client::RpcClient,
    solana_sdk::{
        instruction::Instruction as SdkInstruction,
        pubkey::Pubkey as SdkPubkey,
        signature::Signer,
        signer::keypair::{Keypair, read_keypair_file},
        transaction::Transaction,
    },
    solana_system_interface::instruction as system_ix,
    spl_token_interface::instruction as token_ix,
    std::{str::FromStr, sync::Arc, time::Duration},
    tokio_util::sync::CancellationToken,
};

// The SPL Token program.
const SPL_TOKEN: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

// 6 decimals for both fake tokens (like USDC/USDT).
const DECIMALS: u8 = 6;
// 10 tokens, 6 decimals.
const SELL_AMOUNT: u64 = 10_000_000;

/// Auction deadline for the test (seconds from now). Default 60.
fn deadline_secs() -> i64 {
    std::env::var("SOLANA_DRIVER_TEST_DEADLINE")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|s: &i64| *s > 0)
        .unwrap_or(60)
}

/// Create a new mint account owned by `payer`, with `payer` as mint authority.
/// Returns the mint pubkey.
fn create_mint_ix(mint: &SdkPubkey, payer: &SdkPubkey, spl_token: &SdkPubkey) -> SdkInstruction {
    // Allocate space for a Mint account (82 bytes) + rent.
    let space = 82u64;
    let rent = solana_sdk::sysvar::rent::Rent::default().minimum_balance(space as usize);
    system_ix::create_account(payer, mint, rent, space, spl_token)
}

/// Setup: create two fake token mints and mint sell + buy tokens to the
/// payer's ATAs. Returns (sell_mint, buy_mint, sell_ata, buy_ata).
///
/// All instructions are batched into one transaction.
async fn setup_tokens_and_mints(
    rpc: &RpcClient,
    payer_keypair: &Keypair,
) -> (SdkPubkey, SdkPubkey, SdkPubkey, SdkPubkey) {
    let spl_token = SdkPubkey::from_str(SPL_TOKEN).unwrap();
    let payer_pk = payer_keypair.pubkey();

    // Generate two new mint keypairs.
    let sell_mint = Keypair::new();
    let buy_mint = Keypair::new();
    let sell_mint_pk = sell_mint.pubkey();
    let buy_mint_pk = buy_mint.pubkey();

    // Derive the payer's ATAs.
    let sell_ata =
        spl_associated_token_account_interface::address::get_associated_token_address_with_program_id(
            &payer_pk,
            &sell_mint_pk,
            &spl_token,
        );
    let buy_ata =
        spl_associated_token_account_interface::address::get_associated_token_address_with_program_id(
            &payer_pk,
            &buy_mint_pk,
            &spl_token,
        );

    let mut ixs = Vec::new();
    let mut signers: Vec<&dyn Signer> = vec![payer_keypair];

    // 1. Create sell mint account + initialize mint.
    ixs.push(create_mint_ix(&sell_mint_pk, &payer_pk, &spl_token));
    ixs.push(
        token_ix::initialize_mint(&spl_token, &sell_mint_pk, &payer_pk, None, DECIMALS).unwrap(),
    );

    // 2. Create buy mint account + initialize mint.
    ixs.push(create_mint_ix(&buy_mint_pk, &payer_pk, &spl_token));
    ixs.push(
        token_ix::initialize_mint(&spl_token, &buy_mint_pk, &payer_pk, None, DECIMALS).unwrap(),
    );

    // 3. Create the payer's sell ATA + mint sell tokens to it.
    ixs.push(
        spl_associated_token_account_interface::instruction::create_associated_token_account_idempotent(
            &payer_pk,
            &payer_pk,
            &sell_mint_pk,
            &spl_token,
        ),
    );
    ixs.push(
        token_ix::mint_to(
            &spl_token,
            &sell_mint_pk,
            &sell_ata,
            &payer_pk,
            &[],
            SELL_AMOUNT * 2, // Extra for fees/slippage.
        )
        .unwrap(),
    );

    // 4. Create the payer's buy ATA + mint buy tokens to it (solver liquidity).
    ixs.push(
        spl_associated_token_account_interface::instruction::create_associated_token_account_idempotent(
            &payer_pk,
            &payer_pk,
            &buy_mint_pk,
            &spl_token,
        ),
    );
    ixs.push(
        token_ix::mint_to(
            &spl_token,
            &buy_mint_pk,
            &buy_ata,
            &payer_pk,
            &[],
            SELL_AMOUNT * 2,
        )
        .unwrap(),
    );

    signers.push(&sell_mint);
    signers.push(&buy_mint);

    let blockhash = rpc
        .get_latest_blockhash()
        .await
        .expect("fetch blockhash for token setup");
    let tx = Transaction::new_signed_with_payer(&ixs, Some(&payer_pk), &signers, blockhash);
    rpc.send_and_confirm_transaction(&tx)
        .await
        .expect("token setup transaction failed");
    println!("tokens created: sell={sell_mint_pk}, buy={buy_mint_pk}");

    (sell_mint_pk, buy_mint_pk, sell_ata, buy_ata)
}

/// Create the order on chain: approve the state PDA as delegate on the sell
/// ATA, and send CreateOrder.
///
/// Returns the domain `Order` matching the on-chain intent.
async fn create_order_on_chain(
    rpc: &RpcClient,
    payer_keypair: &Keypair,
    sell_mint: SdkPubkey,
    buy_mint: SdkPubkey,
    sell_ata: SdkPubkey,
    buy_ata: SdkPubkey,
) -> Order {
    let program_id = PROGRAM_ID;
    let spl_token = SdkPubkey::from_str(SPL_TOKEN).unwrap();
    let payer_pk = payer_keypair.pubkey();
    let (state_pda, _) = find_state_pda(&program_id);

    let valid_to = {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        u32::try_from(now + 300).unwrap_or(u32::MAX)
    };

    let intent = OrderIntent {
        owner: payer_pk,
        sell_token_account: sell_ata,
        buy_token_account: buy_ata,
        sell_amount: SELL_AMOUNT,
        buy_amount: SELL_AMOUNT, // 1:1 swap.
        valid_to,
        kind: OrderKind::Sell,
        partially_fillable: false,
        app_data: [0; 32],
    };

    let uid = intent.uid();
    let (order_pda, _) = find_order_pda(&program_id, &uid);

    let ixs = vec![
        // Approve the state PDA as delegate on the sell token account.
        token_ix::approve(
            &spl_token,
            &sell_ata,
            &state_pda,
            &payer_pk,
            &[],
            SELL_AMOUNT,
        )
        .expect("build approve instruction"),
        // Create the order.
        CreateOrder {
            program_id,
            owner: payer_pk,
            created_by: payer_pk,
            intent: &intent,
        }
        .into(),
    ];

    let blockhash = rpc
        .get_latest_blockhash()
        .await
        .expect("fetch blockhash for order creation");
    let tx = Transaction::new_signed_with_payer(&ixs, Some(&payer_pk), &[payer_keypair], blockhash);
    let sig = rpc
        .send_and_confirm_transaction(&tx)
        .await
        .expect("create order transaction failed");
    println!("order created: sig={sig}, order_pda={order_pda}");

    Order {
        uid: OrderUid(uid.to_bytes()),
        owner: payer_pk,
        sell_token: sell_mint,
        buy_token: buy_mint,
        sell_token_account: sell_ata,
        buy_token_account: buy_ata,
        sell_amount: SELL_AMOUNT,
        buy_amount: SELL_AMOUNT,
        valid_to,
        side: Side::Sell,
        partially_fillable: false,
        order_pda,
        app_data: [0; 32],
    }
}

/// Spawn a mock solver engine that returns a single SPL `Transfer` instruction
/// moving buy tokens from the solver's ATA to the buy-mint buffer PDA.
async fn spawn_mock_solver(
    sell_mint: SdkPubkey,
    buy_mint: SdkPubkey,
    solver_pubkey: SdkPubkey,
    solver_buy_ata: SdkPubkey,
    program_id: Pubkey,
) -> (std::net::SocketAddr, CancellationToken) {
    let buy_buffer = find_buffer_pda(&program_id, &buy_mint).0;
    let spl_token = SdkPubkey::from_str(SPL_TOKEN).unwrap();

    // The mock solver returns one solution: a single trade for the order,
    // with one interaction — an SPL Transfer from the solver's buy ATA to
    // the buy-mint buffer PDA. This stands in for a real swap.
    let response = serde_json::json!({
        "solutions": [{
            // solution id 0 (first order, index 0)
            "id": 0,
            // 1:1 prices.
            "prices": {
                (sell_mint.to_string()): SELL_AMOUNT.to_string(),
                (buy_mint.to_string()): SELL_AMOUNT.to_string(),
            },
            "trades": [{
                // The order uid is filled in per-request below.
                "orderUid": "PLACEHOLDER",
                "executedAmount": SELL_AMOUNT.to_string(),
            }],
            "interactions": [{
                "programId": spl_token.to_string(),
                "accounts": [
                    {"pubkey": solver_buy_ata.to_string(), "isSigner": false, "isWritable": true},
                    {"pubkey": buy_buffer.to_string(), "isSigner": false, "isWritable": true},
                    {"pubkey": solver_pubkey.to_string(), "isSigner": true, "isWritable": false},
                ],
                "instructionData": base64::prelude::BASE64_STANDARD.encode({
                    let mut data = vec![3u8];
                        data.extend_from_slice(&SELL_AMOUNT.to_le_bytes());
                        data
                    }
                ),
            }],
            "addressLookupTables": [],
        }]
    });

    let app = axum::Router::new().route(
        "/solve",
        axum::routing::post(move |body: axum::body::Bytes| {
            let response = response.clone();
            async move {
                // Extract the order uid from the request and patch the
                // placeholder.
                let request: serde_json::Value = serde_json::from_slice(&body).unwrap();
                let order_uid = request["orders"][0]["uid"].as_str().unwrap().to_string();
                let patched = response.to_string().replace("PLACEHOLDER", &order_uid);
                axum::Json(serde_json::from_str::<serde_json::Value>(&patched).unwrap())
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock solver");
    let addr = listener.local_addr().unwrap();
    println!("mock solver listening on {addr}");
    let shutdown = CancellationToken::new();
    let token = shutdown.clone();
    tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                token.cancelled().await;
            })
            .await
            .ok();
    });
    (addr, shutdown)
}

/// Spawn the driver API on an ephemeral port.
///
/// Takes the already-loaded driver config and overrides the solver endpoint
/// to point at the in-process mock solver.
async fn spawn_driver(
    config: config::Config,
    solver_addr: std::net::SocketAddr,
) -> (std::net::SocketAddr, CancellationToken) {
    let solvers: Vec<Solver> = config
        .solvers
        .into_iter()
        .map(|s| {
            let endpoint = if s.name == "jupiter-live" {
                format!("http://{solver_addr}/").parse().unwrap()
            } else {
                s.endpoint
            };
            Solver::new(&config::Solver {
                name: s.name,
                endpoint,
                signer_keypair: s.signer_keypair,
                max_in_flight: s.max_in_flight,
            })
        })
        .collect::<Result<_, _>>()
        .expect("failed to load solver signer keypairs");

    let blockchain = Arc::new(Solana::new(
        SolanaRPC::new_with_timeout_and_commitment(
            &config.rpc.endpoint,
            config.rpc.request_timeout,
            CommitmentConfig::confirmed(),
        ),
        config.chain.settlement_program_id,
    ));
    let api = Api {
        addr: "127.0.0.1:0".parse().unwrap(),
        blockchain,
        solvers,
    };
    let (listener, addr) = api.bind().await.expect("bind driver API");
    println!("driver API listening on {addr}");
    let shutdown = CancellationToken::new();
    let token = shutdown.clone();
    tokio::spawn(async move {
        let _ = api.serve(listener, token).await;
    });
    (addr, shutdown)
}

/// Poll `getSignatureStatuses` until confirmed or timeout.
async fn confirm_transaction(
    rpc: &RpcClient,
    signature: &solana_sdk::signature::Signature,
    timeout: Duration,
) -> Result<(), String> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if tokio::time::Instant::now() > deadline {
            return Err(format!(
                "transaction {signature} did not confirm within {timeout:?}"
            ));
        }
        let statuses = rpc
            .get_signature_statuses(&[*signature])
            .await
            .map_err(|e| format!("getSignatureStatuses failed: {e}"))?
            .value;
        match &statuses[0] {
            None => {
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Some(status) => {
                if let Some(err) = &status.err {
                    return Err(format!("transaction {signature} failed: {err}"));
                }
                if status.confirmation_status.is_some() {
                    println!("transaction {signature} confirmed");
                    return Ok(());
                }
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
}

/// End-to-end: create fake tokens, create an order, solve via mock solver,
/// settle, and confirm on devnet.
#[tokio::test]
#[ignore = "hits Solana devnet; needs funded keypair and SOL"]
async fn settle_devnet() {
    // --- load driver config and initialize tracing ---
    let config_path = std::env::var("SOLANA_DRIVER_TEST_CONFIG").unwrap_or_else(|_| {
        panic!("SOLANA_DRIVER_TEST_CONFIG must be set to a driver config .toml file")
    });
    let config = config::load(std::path::Path::new(&config_path)).await;
    initialize_reentrant(&config.observe_config());

    // The program ID used for order creation comes from
    // `cow_settlement_interface::ID`. The driver gets it from the config
    // file. They must match — assert early.
    assert_eq!(
        config.chain.settlement_program_id, PROGRAM_ID,
        "settlement-program-id in {config_path} does not match cow_settlement_interface::ID",
    );

    // The solver keypair doubles as the user/payer: it creates+mints the
    // fake tokens, creates the order, and settles it. The settlement program
    // does not require the user and the solver to be different identities.
    let keypair_path = &config.solvers[0].signer_keypair;
    let payer = read_keypair_file(keypair_path).expect("failed to read solver/payer keypair");
    let payer_pk = payer.pubkey();

    let rpc = RpcClient::new_with_commitment(
        config.rpc.endpoint.to_string(),
        CommitmentConfig::confirmed(),
    );

    // --- setup: create two fake token mints and mint tokens ---
    println!("setting up tokens and mints...");
    let (sell_mint, buy_mint, sell_ata, buy_ata) = setup_tokens_and_mints(&rpc, &payer).await;

    // --- create the order on chain ---
    println!("creating order on chain...");
    let order = create_order_on_chain(&rpc, &payer, sell_mint, buy_mint, sell_ata, buy_ata).await;

    // --- spawn the mock solver ---
    let program_id = PROGRAM_ID;
    let spl_token = SdkPubkey::from_str(SPL_TOKEN).unwrap();
    let solver_buy_ata =
        spl_associated_token_account_interface::address::get_associated_token_address_with_program_id(
            &payer_pk,
            &buy_mint,
            &spl_token,
        );
    println!("spawning mock solver...");
    let (solver_addr, _solver_shutdown) =
        spawn_mock_solver(sell_mint, buy_mint, payer_pk, solver_buy_ata, program_id).await;

    // --- spawn the driver API ---
    println!("spawning driver API...");
    let (driver_addr, _driver_shutdown) = spawn_driver(config, solver_addr).await;

    // --- construct the fake auction ---
    let auction = Auction {
        id: Id::new(1).unwrap(),
        orders: vec![order.clone()],
        deadline_slot: Slot(0),
        deadline: chrono::Utc::now() + chrono::Duration::seconds(deadline_secs()),
    };

    // --- POST /solve ---
    println!("POST /solve...");
    let solve_response: serde_json::Value = reqwest::Client::new()
        .post(format!("http://{driver_addr}/jupiter-live/solve"))
        .json(&serde_json::json!({
            "id": auction.id.get(),
            "deadline": auction.deadline.to_rfc3339(),
            "orders": [{
                "uid": format!("0x{}", const_hex::encode(order.uid.0)),
                "owner": order.owner.to_string(),
                "sellToken": order.sell_token.to_string(),
                "buyToken": order.buy_token.to_string(),
                "sellTokenAccount": order.sell_token_account.to_string(),
                "buyTokenAccount": order.buy_token_account.to_string(),
                "sellAmount": order.sell_amount.to_string(),
                "buyAmount": order.buy_amount.to_string(),
                "validTo": order.valid_to,
                "kind": "sell",
                "partiallyFillable": order.partially_fillable,
                "orderPda": order.order_pda.to_string(),
                "appData": format!("0x{}", const_hex::encode(order.app_data)),
            }],
        }))
        .send()
        .await
        .expect("/solve request failed")
        .json()
        .await
        .expect("/solve response is not JSON");

    let solutions = solve_response["solutions"]
        .as_array()
        .expect("/solve returned no solutions array");
    assert!(!solutions.is_empty(), "/solve returned no solutions");
    let solution_id = solutions[0]["solutionId"]
        .as_u64()
        .expect("solutionId is not a number");
    println!(
        "/solve returned {} solution(s), id={solution_id}",
        solutions.len()
    );

    // --- POST /settle ---
    println!(
        "POST /settle (auctionId={}, solutionId={})...",
        auction.id.get(),
        solution_id,
    );
    let settle_response = reqwest::Client::new()
        .post(format!("http://{driver_addr}/jupiter-live/settle"))
        .json(&serde_json::json!({
            "auctionId": auction.id.get(),
            "solutionId": solution_id,
            "submissionDeadlineSlot": 0,
        }))
        .send()
        .await
        .expect("/settle request failed");

    let settle_status = settle_response.status();
    let settle_body: serde_json::Value = settle_response
        .json()
        .await
        .expect("/settle response is not JSON");

    assert!(
        settle_status.is_success(),
        "/settle returned {settle_status}: {settle_body}"
    );

    let tx_signature = settle_body["txSignature"]
        .as_str()
        .expect("/settle response has no txSignature");
    println!("/settle returned txSignature={tx_signature}");

    // --- confirm on chain ---
    println!("polling confirmation for {tx_signature}...");
    let signature: solana_sdk::signature::Signature = tx_signature
        .parse()
        .expect("txSignature is not a valid signature");
    confirm_transaction(&rpc, &signature, Duration::from_secs(60))
        .await
        .expect("settlement transaction did not land on chain");

    println!("settlement confirmed on devnet: {tx_signature}");
}
