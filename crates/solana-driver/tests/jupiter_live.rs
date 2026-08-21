//! Live integration test against the in-repo Jupiter solver engine.
//!
//! Spins up the real `solana-solvers` HTTP API in-process with Jupiter pointed
//! at the live swap API, then exercises the driver's `Solver` client against
//! it. This crosses the real driver <-> solver wire boundary (serialization on
//! both sides, the solve loop, and Jupiter quote/swap-instruction parsing).
//!
//! Network-dependent and non-deterministic (Jupiter routes/amounts vary), so
//! it is `#[ignore]` by default. Run on demand:
//!
//! ```text
//! cargo nextest run -p solana-driver --run-ignored ignored-only --test jupiter_live
//! # set JUPITER_API_KEY for rate-limit headroom:
//! JUPITER_API_KEY=... cargo nextest run -p solana-driver --run-ignored ignored-only --test jupiter_live
//! # override the auction deadline (default 15s):
//! SOLANA_DRIVER_TEST_DEADLINE=30 cargo nextest run -p solana-driver --run-ignored ignored-only --test jupiter_live
//! # show the full deserialized solution (printed by the test):
//! cargo nextest run -p solana-driver --run-ignored ignored-only --test jupiter_live --nocapture
//! ```

use {
    solana_driver::{
        domain::{Auction, Id, Order, Side, Slot, order_uid::OrderUid},
        infra::{config, solver::Solver},
        util::associated_token_address,
    },
    solana_sdk::{
        pubkey::Pubkey,
        signer::{Signer, keypair::read_keypair_file},
    },
    solana_solvers::{
        api::Api,
        config::JupiterConfig,
        dex::{Dex, jupiter::Jupiter},
    },
    solana_testlib::temp_keypair,
    std::{str::FromStr, sync::Arc},
    tokio_util::sync::CancellationToken,
};

// USDC and USDT, both 6-decimal stablecoins on Solana mainnet.
const USDC: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
const USDT: &str = "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB";

/// Auction deadline for the live test (seconds).
fn deadline() -> chrono::DateTime<chrono::Utc> {
    let secs = std::env::var("SOLANA_DRIVER_TEST_DEADLINE")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|s: &i64| *s > 0)
        .unwrap_or(15);
    chrono::Utc::now() + chrono::Duration::seconds(secs)
}

/// A sell of 10 USDC for USDT. The driver derives the buy-side ATA from the
/// solver's account, so the domain order carries no destination.
fn sell_auction() -> Auction {
    Auction {
        id: Id::new(1).unwrap(),
        orders: vec![Order {
            uid: OrderUid([8; 32]),
            owner: Pubkey::default(),
            sell_token: Pubkey::from_str(USDC).unwrap(),
            buy_token: Pubkey::from_str(USDT).unwrap(),
            sell_token_account: Pubkey::default(),
            buy_token_account: Pubkey::default(),
            sell_amount: 10_000_000,
            buy_amount: 0,
            valid_to: 0,
            side: Side::Sell,
            partially_fillable: false,
            order_pda: Pubkey::default(),
            app_data: [0; 32],
        }],
        deadline_slot: Slot(1),
        deadline: deadline(),
    }
}

/// Serve the real solver engine on an ephemeral port, returning the address
/// the driver should target and a token that stops the server when cancelled.
async fn spawn_solver() -> (std::net::SocketAddr, CancellationToken) {
    let jupiter = Jupiter::new(&JupiterConfig {
        endpoint: "https://api.jup.ag".parse().unwrap(),
        api_key: std::env::var("JUPITER_API_KEY").ok(),
        slippage_bps: 50,
        enable_buy_orders: false,
    })
    .expect("build jupiter dex");

    let api = Api {
        addr: "127.0.0.1:0".parse().unwrap(),
        dex: Arc::new(Dex::Jupiter(jupiter)),
    };
    let (listener, addr) = api.bind().await.expect("bind solver engine");
    let shutdown = CancellationToken::new();
    let token = shutdown.clone();
    tokio::spawn(async move {
        // Any shutdown future works; the token is never cancelled in the happy
        // path, the task is dropped with the test.
        let _ = api
            .serve(listener, async move {
                token.cancelled().await;
            })
            .await;
    });
    (addr, shutdown)
}

/// The driver's solver client posts the auction to a live Jupiter-backed
/// engine and maps the response back into domain solutions.
#[tokio::test]
#[ignore = "hits the live Jupiter swap API; needs network"]
async fn driver_solves_against_live_jupiter_engine() {
    let (addr, _shutdown) = spawn_solver().await;

    // Any valid pubkey works: Jupiter builds instructions for this account,
    // the swap only runs for real once the driver submits the settlement.
    let keypair_file = temp_keypair();
    let keypair_path = keypair_file.path().to_path_buf();
    let solver_account = read_keypair_file(&keypair_path).unwrap().pubkey();
    let solver = Solver::new(&config::Solver {
        name: "jupiter-live".to_string(),
        endpoint: format!("http://{addr}").parse().unwrap(),
        account: solver_account,
        signer_keypair: keypair_path,
        max_in_flight: std::num::NonZero::new(1).unwrap(),
    })
    .expect("solver construction should succeed");

    // `Solver::solve` posts the auction and deserializes the JSON response
    // into `domain::Solution`s; an `Ok` result proves the wire deserialization
    // succeeded.
    let solutions = solver
        .solve(&sell_auction())
        .await
        .expect("solve should succeed against live Jupiter");

    assert_eq!(solutions.len(), 1, "one solution for the single order");
    let solution = &solutions[0];

    // --- solution identity ---
    // Index 0 for the single order; the driver stamps `solver` with the
    // configured account.
    assert_eq!(solution.id, 0, "solution id is the order index");
    assert_eq!(solution.solver, solver_account);

    // --- trade ---
    // One trade fulfilling our order. The wire format carries no fee.
    assert_eq!(solution.trades.len(), 1);
    let trade = &solution.trades[0];
    assert_eq!(trade.order_uid, OrderUid([8; 32]));
    assert_eq!(trade.executed_sell, 10_000_000, "full sell amount filled");

    // --- interactions ---
    // The swap must arrive as real Solana instructions: every interaction
    // targets a non-default program, and at least one carries instruction data.
    assert!(
        !solution.interactions.is_empty(),
        "Jupiter must return at least the swap instruction"
    );
    for ix in &solution.interactions {
        assert!(
            ix.program_id != Pubkey::default(),
            "interaction targets a real program, not the zero address"
        );
    }
    assert!(
        solution.interactions.iter().any(|ix| !ix.data.is_empty()),
        "at least one interaction carries instruction data"
    );

    // The swap instructions must be built for our settlement signer and land
    // the buy output in the ATA the driver derived from it. This is the
    // end-to-end check that the `buy_destination` derivation flows through the
    // whole driver <-> solver <-> Jupiter path.
    let buy_destination =
        associated_token_address(&solver_account, &Pubkey::from_str(USDT).unwrap());
    let touched: Vec<Pubkey> = solution
        .interactions
        .iter()
        .flat_map(|ix| ix.accounts.iter().map(|a| a.pubkey))
        .collect();
    assert!(
        touched.contains(&solver_account),
        "swap instructions must reference the settlement signer {solver_account}; touched \
         accounts: {touched:?}"
    );
    assert!(
        touched.contains(&buy_destination),
        "swap instructions must send output to the derived buy ATA {buy_destination}; touched \
         accounts: {touched:?}"
    );

    // --- address lookup tables ---
    // A v0 transaction can carry zero address lookup tables, but a Jupiter
    // swap route touches enough accounts that it returns at least one.
    assert!(
        !solution.address_lookup_tables.is_empty(),
        "Jupiter swap route should return address lookup tables"
    );

    // --- compute estimate ---
    // The solver does not estimate compute units.
    assert_eq!(solution.cu_estimate, None);

    // Surface the full deserialized solution for `--nocapture` inspection.
    println!("deserialized solution: {solution:#?}");
}
