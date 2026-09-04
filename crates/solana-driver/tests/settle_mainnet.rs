//! Live integration test: create an order on Solana mainnet, feed it through
//! the driver + Jupiter solver, and assert the settlement transaction lands.
//!
//! This test is `#[ignore]` by default — it hits mainnet, costs SOL, and needs
//! a funded keypair. Run on demand:
//!
//! ```text
//! cargo nextest run -p solana-driver --run-ignored ignored-only --test settle_mainnet --nocapture
//! # optional:
//! JUPITER_API_KEY=...
//! ```
//!
//! Everything the test needs — the RPC endpoint, the settlement program ID,
//! and the solver signer keypair — comes from a driver config file. The path
//! is set by `SOLANA_DRIVER_TEST_CONFIG` (typically
//! `testing.settle-mainnet.toml`).
//! The solver keypair doubles as the user who creates the order: the
//! settlement program does not require the user and the solver to be
//! different identities, so one funded keypair plays both roles. This
//! keypair must hold SOL (for fees + rent) and at least 15 units of USDC
//! or USDT (see `MIN_TOKEN_BALANCE`) — the test sells whichever of the
//! two the keypair holds more of.
//!
//! The test overrides the solver endpoint to the in-process engine's
//! ephemeral port but keeps the keypair from the config file.
//!
//! The test treats the driver as a black box: it spawns the solver engine and
//! the driver API in-process, sends HTTP requests to `/solve` and `/settle`,
//! and polls the returned signature on-chain. No driver source code is
//! modified, and no transaction construction logic is duplicated in the test
//! — the driver builds, signs, and sends the settlement transaction itself.
//!
//! The test is safe to run repeatedly: `valid_to` is set to `now + 300s`, so
//! each run produces a unique order UID and a fresh order PDA. The SPL
//! `approve` on the sell token account is a set operation (not additive), so
//! re-approving the same delegate for the same amount is harmless.

// Required for `TokenAccount::unpack`.
use {
    cow_settlement_client::{
        cow_settlement_interface::{
            ID as PROGRAM_ID,
            Pubkey,
            data::intent::{Flags, OrderIntent, OrderKind},
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
        program_pack::Pack,
        signature::Signer,
        signer::keypair::read_keypair_file,
        transaction::Transaction,
    },
    solana_solvers::{
        api::Api as SolverApi,
        config::JupiterConfig,
        dex::{Dex, jupiter::Jupiter},
    },
    spl_token_interface::{instruction as token_ix, state::Account as TokenAccount},
    std::{sync::Arc, time::Duration},
    tokio_util::sync::CancellationToken,
};

const USDC_MINT: Pubkey = Pubkey::from_str_const("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
const USDT_MINT: Pubkey = Pubkey::from_str_const("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB");
const SPL_TOKEN_PROGRAM: Pubkey =
    Pubkey::from_str_const("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

// 10 sell tokens; both USDC and USDT use 6 decimals.
const SELL_AMOUNT: u64 = 10_000_000;

// Minimum balance on the chosen sell token (10 to sell + 5 safety margin
// for slippage, fees, or rate movement).
const MIN_TOKEN_BALANCE: u64 = 15_000_000;

// Cushion the buy-token settlement buffer must hold. Settle pays the user by
// Pushing the quoted buy amount out of the buffer: if the route delivers even
// slightly less than the quote, the Push fails with SPL Token `insufficient
// funds` (observed in practice).
const MIN_BUY_BUFFER_BALANCE: u64 = 1_000_000;

#[derive(Clone, Copy, Debug)]
struct Token {
    name: &'static str,
    mint: Pubkey,
}

struct Pair {
    sell: Token,
    buy: Token,
}

/// Auction deadline for the test (seconds from now). Default 60.
fn deadline_secs() -> i64 {
    std::env::var("SOLANA_DRIVER_TEST_DEADLINE")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|s: &i64| *s > 0)
        .unwrap_or(60)
}

/// Pre-flight check: print the user's pubkey, SOL balance, and USDC/USDT
/// token account state (balance + delegate if the account exists, or "account
/// does not exist" otherwise), and resolve the trade direction: the sell token
/// is whichever of USDC/USDT the user holds more of. Exit the test early if
/// neither balance reaches [`MIN_TOKEN_BALANCE`] — the buy token needs none,
/// and its ATA is created in `create_order_on_chain`. Also assert the buy
/// token's settlement buffer holds at least [`MIN_BUY_BUFFER_BALANCE`]:
/// the settle step pays the user out of it.
async fn preflight_check(rpc: &RpcClient, user: &dyn Signer) -> Pair {
    let user_pubkey = user.pubkey();
    println!("=== pre-flight check ===");
    println!("pubkey: {user_pubkey}");

    let sol_lamports = rpc
        .get_balance(&user_pubkey)
        .await
        .expect("fetch SOL balance");
    println!("SOL balance: {} SOL", sol_lamports as f64 / 1_000_000_000.0);

    let usdc = Token {
        name: "USDC",
        mint: USDC_MINT,
    };
    let usdt = Token {
        name: "USDT",
        mint: USDT_MINT,
    };
    let spl_token = SPL_TOKEN_PROGRAM;

    let mut balances: Vec<(Token, u64)> = Vec::new();
    for token in [usdc, usdt] {
        let ata =
            spl_associated_token_account_interface::address::get_associated_token_address_with_program_id(
                &user_pubkey,
                &token.mint,
                &spl_token,
            );
        let balance = match rpc.get_account(&ata).await {
            Ok(account) => {
                let token_account =
                    TokenAccount::unpack(&account.data).expect("unpack token account data");
                println!(
                    "{} balance: {:.6}, delegate: {:?}",
                    token.name,
                    token_account.amount as f64 / 1_000_000.0,
                    token_account.delegate,
                );
                token_account.amount
            }
            Err(_) => {
                println!("{} account does not exist", token.name);
                0
            }
        };
        balances.push((token, balance));
    }

    let [(usdc, usdc_balance), (usdt, usdt_balance)]: [(Token, u64); 2] =
        balances.try_into().expect("balances vec holds both tokens");
    let (sell, buy, sell_balance) = if usdc_balance >= usdt_balance {
        (usdc, usdt, usdc_balance)
    } else {
        (usdt, usdc, usdt_balance)
    };
    assert!(
        sell_balance >= MIN_TOKEN_BALANCE,
        "insufficient {}: have {sell_balance} ({:.6} {}), need at least {MIN_TOKEN_BALANCE} \
         ({:.6}) — 10 to sell + 5 safety margin; neither USDC nor USDT balance reaches the minimum",
        sell.name,
        sell_balance as f64 / 1_000_000.0,
        sell.name,
        MIN_TOKEN_BALANCE as f64 / 1_000_000.0,
    );
    println!("trade direction: sell {} -> buy {}", sell.name, buy.name);

    let (buy_buffer, _) = find_buffer_pda(&PROGRAM_ID, &buy.mint);
    let buy_buffer_balance = match rpc.get_account(&buy_buffer).await {
        Ok(account) => {
            let token_account =
                TokenAccount::unpack(&account.data).expect("unpack buffer token account data");
            println!(
                "{} buffer balance: {:.6} ({buy_buffer})",
                buy.name,
                token_account.amount as f64 / 1_000_000.0,
            );
            token_account.amount
        }
        Err(_) => {
            println!("{} buffer account does not exist ({buy_buffer})", buy.name);
            0
        }
    };
    assert!(
        buy_buffer_balance >= MIN_BUY_BUFFER_BALANCE,
        "insufficient {} buffer: have {buy_buffer_balance} ({:.6}), need at least \
         {MIN_BUY_BUFFER_BALANCE} ({:.6}) — settle pushes the quoted amount out of this buffer \
         and the cushion absorbs quote-vs-execution divergence",
        buy.name,
        buy_buffer_balance as f64 / 1_000_000.0,
        MIN_BUY_BUFFER_BALANCE as f64 / 1_000_000.0,
    );
    println!("pre-flight check passed");
    println!("=== end pre-flight check ===");
    Pair { sell, buy }
}

/// Spawn the real Jupiter solver engine on an ephemeral port.
async fn spawn_solver() -> (std::net::SocketAddr, CancellationToken) {
    let jupiter = Jupiter::new(&JupiterConfig {
        endpoint: "https://api.jup.ag".parse().unwrap(),
        api_key: std::env::var("JUPITER_API_KEY").ok(),
        slippage_bps: 50,
        enable_buy_orders: false,
    })
    .expect("build jupiter dex");

    let api = SolverApi {
        addr: "127.0.0.1:0".parse().unwrap(),
        dex: Arc::new(Dex::Jupiter(jupiter)),
    };
    let (listener, addr) = api.bind().await.expect("bind solver engine");
    println!("solver engine listening on {addr}");
    let shutdown = CancellationToken::new();
    let token = shutdown.clone();
    tokio::spawn(async move {
        let _ = api
            .serve(listener, async move {
                token.cancelled().await;
            })
            .await;
    });
    (addr, shutdown)
}

/// Spawn the driver API on an ephemeral port, pointed at mainnet.
///
/// Takes the already-loaded driver config and overrides the solver endpoint to
/// point at the in-process solver engine. This exercises the real
/// `config::load` → `Api` construction path.
async fn spawn_driver(
    config: config::Config,
    solver_addr: std::net::SocketAddr,
) -> (std::net::SocketAddr, CancellationToken) {
    // Override the solver endpoint to the in-process engine. Keep the
    // keypair from the config file.
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

/// Create the order on mainnet and return the domain `Order` the test will
/// feed into the driver's `/solve` endpoint.
///
/// The transaction:
/// 1. Creates the user's buy-token ATA if it does not exist (idempotent).
/// 2. Approves the state PDA as delegate on the user's sell-token account.
/// 3. Sends the `CreateOrder` instruction.
async fn create_order_on_chain(rpc: &RpcClient, user: &dyn Signer, pair: &Pair) -> Order {
    let program_id = PROGRAM_ID;
    let user_pubkey = user.pubkey();
    let spl_token = SPL_TOKEN_PROGRAM;

    // Derive the user's token accounts (ATAs under the SPL Token program).
    let sell_token_account =
        spl_associated_token_account_interface::address::get_associated_token_address_with_program_id(
            &user_pubkey,
            &pair.sell.mint,
            &spl_token,
        );
    let buy_token_account =
        spl_associated_token_account_interface::address::get_associated_token_address_with_program_id(
            &user_pubkey,
            &pair.buy.mint,
            &spl_token,
        );

    // The state PDA is the SPL delegate the settlement program uses to pull
    // sell tokens from the user's account during `BeginSettle`.
    let (state_pda, _) = find_state_pda(&program_id);

    let valid_to = {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        u32::try_from(now + 300).unwrap_or(u32::MAX)
    };

    let intent = OrderIntent {
        owner: user_pubkey,
        sell_token_account,
        sell_mint: pair.sell.mint,
        buy_token_account,
        buy_mint: pair.buy.mint,
        sell_amount: SELL_AMOUNT,
        buy_amount: 0,
        valid_to,
        flags: Flags {
            created_on_chain: true,
            kind: OrderKind::Sell,
            partially_fillable: false,
        },
        app_data: [0; 32],
    };

    let uid = intent.uid();
    let (order_pda, _) = find_order_pda(&program_id, &uid);

    let ixs = vec![
        spl_associated_token_account_interface::instruction::create_associated_token_account_idempotent(
            &user_pubkey,
            &user_pubkey,
            &pair.buy.mint,
            &spl_token,
        ),
        token_ix::approve(
            &spl_token,
            &sell_token_account,
            &state_pda,
            &user_pubkey,
            &[],
            SELL_AMOUNT,
        )
        .expect("build approve instruction"),
        CreateOrder {
            program_id,
            owner: user_pubkey,
            created_by: user_pubkey,
            intent: &intent,
        }
        .into(),
    ];

    let blockhash = rpc
        .get_latest_blockhash()
        .await
        .expect("fetch blockhash for order creation");
    println!(
        "create_order: {} instruction(s), blockhash={blockhash}",
        ixs.len()
    );
    let tx = Transaction::new_signed_with_payer(&ixs, Some(&user_pubkey), &[user], blockhash);
    let sig = rpc
        .send_and_confirm_transaction(&tx)
        .await
        .expect("create order transaction failed");
    println!("order created: sig={sig}, order_pda={order_pda}");

    // Return the domain Order matching the on-chain intent.
    Order {
        uid: OrderUid(uid.to_bytes()),
        owner: user_pubkey,
        sell_token: pair.sell.mint,
        buy_token: pair.buy.mint,
        sell_token_account,
        buy_token_account,
        sell_amount: SELL_AMOUNT,
        buy_amount: 0,
        valid_to,
        side: Side::Sell,
        partially_fillable: false,
        order_pda,
        app_data: [0; 32],
    }
}

/// Poll `getSignatureStatuses` until the transaction is confirmed or the
/// timeout expires. Returns `Ok(())` if confirmed, `Err` with the failure
/// reason otherwise.
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

/// End-to-end: create an order on mainnet, solve it through the driver +
/// Jupiter, settle it, and confirm the transaction landed.
#[tokio::test]
#[ignore = "hits Solana mainnet; needs funded keypairs and SOL"]
async fn settle_on_mainnet() {
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

    // The solver keypair doubles as the user. The test needs exactly one
    // funded keypair — it creates the order, approves the delegate, and the
    // driver signs the settlement tx with the same keypair.
    let keypair_path = &config.solvers[0].signer_keypair;
    let user = read_keypair_file(keypair_path).expect("failed to read solver/user keypair");

    // --- nonblocking RPC client for order creation and confirmation ---
    let rpc = RpcClient::new_with_commitment(
        config.rpc.endpoint.to_string(),
        CommitmentConfig::confirmed(),
    );

    // --- pre-flight: verify balances before spending anything ---
    let pair = preflight_check(&rpc, &user).await;

    let order = create_order_on_chain(&rpc, &user, &pair).await;

    // --- spawn the solver engine ---
    println!("spawning solver engine...");
    let (solver_addr, _solver_shutdown) = spawn_solver().await;

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
    // The driver rejects a deadline slot at or below the current slot, so
    // base it on a live slot fetch. 100 slots ≈ 40s at the driver's
    // SLOT_DURATION_MS (400ms) mapping.
    let current_slot = rpc.get_slot().await.expect("getSlot failed");
    let submission_deadline_slot = current_slot + 100;
    println!(
        "POST /settle (auctionId={}, solutionId={}, submissionDeadlineSlot={})...",
        auction.id.get(),
        solution_id,
        submission_deadline_slot,
    );
    let settle_response = reqwest::Client::new()
        .post(format!("http://{driver_addr}/jupiter-live/settle"))
        .json(&serde_json::json!({
            "auctionId": auction.id.get(),
            "solutionId": solution_id,
            "submissionDeadlineSlot": submission_deadline_slot,
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
        "/settle returned {settle_status}: {settle_body}",
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

    println!("settlement confirmed on mainnet: {tx_signature}");
}
