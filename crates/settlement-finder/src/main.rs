use {
    alloy_primitives::{Address, B256, Bytes, U256, hex},
    alloy_provider::Provider,
    alloy_rpc_types::{Filter, Log},
    alloy_sol_types::SolEventInterface,
    anyhow::{Context, Result, ensure},
    bigdecimal::BigDecimal,
    clap::Parser,
    contracts::GPv2Settlement::{self, GPv2Settlement::GPv2SettlementEvents},
    number::conversions::u256_to_big_decimal,
    serde_json::json,
    sqlx::{Connection, PgConnection},
    std::collections::BTreeMap,
};

/// Cross-checks the `settlements` and `trades` DB tables against the chain,
/// e.g. for the trades tx_hash backfill (see database/backfills/).
///
/// Block mode (default): looks for GPv2 settlement transactions in the given
/// block and prints the Settlement and Trade events found there. If the block
/// contains no settlement, brute-force searches the neighboring blocks for
/// one. Strict mode: when any of the trade filters (--order-uid, --owner,
/// --sell-token, --buy-token, --sell-amount, --buy-amount, --fee-amount) is
/// given, only transactions containing a Trade event matching all of them are
/// reported, and the neighbor search triggers when the target block has no
/// such trade. Exits with 1 if nothing is found.
///
/// DB mode (--db): finds trades in the database that no settlement event
/// resolves to (the same association as backfill_trades_tx_hash.sql) and
/// searches the chain for the settlement transaction that actually contains
/// each of them — first in the block the DB recorded, then in the neighboring
/// blocks. An event only counts as a match if its order uid, amounts and
/// tokens (from the orders/jit_orders tables) equal the DB data, since with
/// failed reorg protection the DB may describe a fill that never made it
/// on-chain. Exits with 1 if any trade could not be uniquely located.
///
/// Results are printed as text (block mode) or a table (DB mode); pass --json
/// for machine-readable output. Progress goes to stderr.
#[derive(Parser)]
struct Args {
    /// Block number to inspect. Required unless --db is used.
    #[arg(required_unless_present = "db", conflicts_with = "db")]
    block: Option<u64>,

    /// RPC endpoint of the chain to inspect.
    #[arg(long, env = "RPC_URL")]
    rpc_url: String,

    /// Address of the settlement contract. Defaults to the canonical
    /// deployment on the connected chain.
    #[arg(long)]
    settlement: Option<Address>,

    /// How many blocks before and after the target block to search when the
    /// target block itself contains no match. 0 disables the search.
    #[arg(long, default_value_t = 25)]
    window: u64,

    /// Postgres connection string of the database to cross-check; enables DB
    /// mode.
    #[arg(long, env = "DB_URL")]
    db: Option<String>,

    /// Print results as a JSON document on stdout.
    #[arg(long)]
    json: bool,

    /// Only report Trade events with this order uid (56 bytes hex).
    #[arg(long, conflicts_with = "db")]
    order_uid: Option<Bytes>,

    /// Only report Trade events with this order owner.
    #[arg(long, conflicts_with = "db")]
    owner: Option<Address>,

    /// Only report Trade events selling this token.
    #[arg(long, conflicts_with = "db")]
    sell_token: Option<Address>,

    /// Only report Trade events buying this token.
    #[arg(long, conflicts_with = "db")]
    buy_token: Option<Address>,

    /// Only report Trade events with exactly this executed sell amount
    /// (atoms, fees included).
    #[arg(long, conflicts_with = "db")]
    sell_amount: Option<U256>,

    /// Only report Trade events with exactly this executed buy amount
    /// (atoms).
    #[arg(long, conflicts_with = "db")]
    buy_amount: Option<U256>,

    /// Only report Trade events with exactly this executed fee amount
    /// (atoms of the sell token).
    #[arg(long, conflicts_with = "db")]
    fee_amount: Option<U256>,
}

/// A Trade event as found on-chain.
struct ChainTrade {
    log_index: u64,
    owner: Address,
    sell_token: Address,
    buy_token: Address,
    sell_amount: U256,
    buy_amount: U256,
    fee_amount: U256,
    order_uid: Vec<u8>,
}

/// All events a settlement transaction emitted, in log order.
struct SettlementTx {
    block: u64,
    tx_hash: B256,
    tx_index: Option<u64>,
    settlements: Vec<(u64, Address)>,
    trades: Vec<ChainTrade>,
}

/// Strict-mode filters of block mode. When any is set, only Trade events
/// matching all of them count.
struct TradeFilter {
    order_uid: Option<Bytes>,
    owner: Option<Address>,
    sell_token: Option<Address>,
    buy_token: Option<Address>,
    sell_amount: Option<U256>,
    buy_amount: Option<U256>,
    fee_amount: Option<U256>,
}

impl TradeFilter {
    fn is_active(&self) -> bool {
        self.order_uid.is_some()
            || self.owner.is_some()
            || self.sell_token.is_some()
            || self.buy_token.is_some()
            || self.sell_amount.is_some()
            || self.buy_amount.is_some()
            || self.fee_amount.is_some()
    }

    fn matches(&self, trade: &ChainTrade) -> bool {
        fn check<T: PartialEq>(filter: &Option<T>, value: &T) -> bool {
            filter.as_ref().is_none_or(|filter| filter == value)
        }
        self.order_uid
            .as_ref()
            .is_none_or(|uid| uid.as_ref() == trade.order_uid)
            && check(&self.owner, &trade.owner)
            && check(&self.sell_token, &trade.sell_token)
            && check(&self.buy_token, &trade.buy_token)
            && check(&self.sell_amount, &trade.sell_amount)
            && check(&self.buy_amount, &trade.buy_amount)
            && check(&self.fee_amount, &trade.fee_amount)
    }

    /// Drops non-matching trades and, if the filter is active, transactions
    /// without any matching trade.
    fn apply(&self, txs: &mut Vec<SettlementTx>) {
        if !self.is_active() {
            return;
        }
        for tx in txs.iter_mut() {
            tx.trades.retain(|trade| self.matches(trade));
        }
        txs.retain(|tx| !tx.trades.is_empty());
    }
}

/// Groups settlement contract logs by transaction, dropping transactions that
/// did not emit a Settlement event. A transaction may contain multiple
/// settlements (e.g. a settlement calling settle() again in an interaction).
fn group_by_tx(logs: &[Log]) -> Vec<SettlementTx> {
    let mut txs: Vec<SettlementTx> = Vec::new();
    for log in logs {
        let (Some(tx_hash), Some(log_index), Some(block)) =
            (log.transaction_hash, log.log_index, log.block_number)
        else {
            continue;
        };
        let Ok(event) = GPv2SettlementEvents::decode_log(&log.inner) else {
            continue;
        };
        let tx = match txs.iter_mut().find(|tx| tx.tx_hash == tx_hash) {
            Some(tx) => tx,
            None => {
                txs.push(SettlementTx {
                    block,
                    tx_hash,
                    tx_index: log.transaction_index,
                    settlements: Vec::new(),
                    trades: Vec::new(),
                });
                txs.last_mut().unwrap()
            }
        };
        match event.data {
            GPv2SettlementEvents::Settlement(settlement) => {
                tx.settlements.push((log_index, settlement.solver));
            }
            GPv2SettlementEvents::Trade(trade) => {
                tx.trades.push(ChainTrade {
                    log_index,
                    owner: trade.owner,
                    sell_token: trade.sellToken,
                    buy_token: trade.buyToken,
                    sell_amount: trade.sellAmount,
                    buy_amount: trade.buyAmount,
                    fee_amount: trade.feeAmount,
                    order_uid: trade.orderUid.to_vec(),
                });
            }
            _ => (),
        }
    }
    txs.retain(|tx| !tx.settlements.is_empty());
    txs
}

async fn fetch_settlements(
    provider: &impl Provider,
    address: Address,
    from_block: u64,
    to_block: u64,
) -> Result<Vec<SettlementTx>> {
    let filter = Filter::new()
        .address(address)
        .from_block(from_block)
        .to_block(to_block);
    let logs = provider
        .get_logs(&filter)
        .await
        .context("could not fetch logs")?;
    Ok(group_by_tx(&logs))
}

fn offset(block: u64, target_block: u64) -> i64 {
    block.cast_signed() - target_block.cast_signed()
}

/// Formats the distance to the target block, e.g. ", -3" for 3 blocks before
/// it and "" for the target block itself.
fn offset_suffix(block: u64, target_block: u64) -> String {
    match offset(block, target_block) {
        0 => String::new(),
        offset => format!(", {offset:+}"),
    }
}

fn trade_json(trade: &ChainTrade) -> serde_json::Value {
    json!({
        "log_index": trade.log_index,
        "order_uid": hex::encode_prefixed(&trade.order_uid),
        "owner": trade.owner.to_string(),
        "sell_token": trade.sell_token.to_string(),
        "buy_token": trade.buy_token.to_string(),
        "sell_amount": trade.sell_amount.to_string(),
        "buy_amount": trade.buy_amount.to_string(),
        "fee_amount": trade.fee_amount.to_string(),
    })
}

fn print_tx(tx: &SettlementTx, target_block: u64) {
    print!(
        "\ntx {} (block {}{}",
        tx.tx_hash,
        tx.block,
        offset_suffix(tx.block, target_block)
    );
    match tx.tx_index {
        Some(tx_index) => println!(", tx_index {tx_index})"),
        None => println!(")"),
    }
    for (log_index, solver) in &tx.settlements {
        println!("  Settlement log_index {log_index:>4} solver {solver}");
    }
    for trade in &tx.trades {
        println!(
            "  Trade      log_index {:>4} order_uid {}",
            trade.log_index,
            hex::encode_prefixed(&trade.order_uid)
        );
    }
}

async fn block_mode(
    provider: &impl Provider,
    address: Address,
    chain_id: u64,
    block: u64,
    window: u64,
    filter: &TradeFilter,
    json: bool,
) -> Result<()> {
    let subject = match filter.is_active() {
        true => "matching trade",
        false => "settlement",
    };

    let mut txs = fetch_settlements(provider, address, block, block).await?;
    filter.apply(&mut txs);

    let mut searched_blocks = None;
    if txs.is_empty() && window > 0 {
        let from = block.saturating_sub(window);
        let to = block.saturating_add(window);
        eprintln!("no {subject} in block {block}, searching blocks {from}..={to}");
        txs = fetch_settlements(provider, address, from, to).await?;
        filter.apply(&mut txs);
        txs.sort_by_key(|tx| (tx.block, tx.tx_index));
        searched_blocks = Some((from, to));
    }

    if json {
        let doc = json!({
            "chain_id": chain_id,
            "contract": address.to_string(),
            "target_block": block,
            "searched_blocks": searched_blocks
                .map(|(from, to)| json!({"from": from, "to": to})),
            "transactions": txs.iter().map(|tx| json!({
                "tx_hash": tx.tx_hash.to_string(),
                "block": tx.block,
                "offset": offset(tx.block, block),
                "tx_index": tx.tx_index,
                "settlements": tx.settlements.iter().map(|(log_index, solver)| json!({
                    "log_index": log_index,
                    "solver": solver.to_string(),
                })).collect::<Vec<_>>(),
                "trades": tx.trades.iter().map(trade_json).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&doc)?);
    } else if !txs.is_empty() {
        println!(
            "chain {chain_id} (contract {address}): {} settlement transaction(s)",
            txs.len()
        );
        for tx in &txs {
            print_tx(tx, block);
        }
    }

    if txs.is_empty() {
        eprintln!(
            "no {subject} in block {block} (window ±{window}) on chain {chain_id} (contract \
             {address})"
        );
        std::process::exit(1);
    }
    Ok(())
}

/// A DB trade that no settlement event resolves to, together with the token
/// and owner data of its order (NULL if the order is in neither the orders
/// nor the jit_orders table).
#[derive(sqlx::FromRow)]
struct DbTrade {
    block_number: i64,
    log_index: i64,
    order_uid: Vec<u8>,
    sell_amount: BigDecimal,
    buy_amount: BigDecimal,
    fee_amount: BigDecimal,
    owner: Option<Vec<u8>>,
    sell_token: Option<Vec<u8>>,
    buy_token: Option<Vec<u8>>,
}

/// Same trade <-> settlement association as backfill_trades_tx_hash.sql; does
/// not reference trades.tx_hash so it also runs against pre-V112 databases.
const ORPHANED_TRADES_QUERY: &str = r#"
SELECT
    t.block_number,
    t.log_index,
    t.order_uid,
    t.sell_amount,
    t.buy_amount,
    t.fee_amount,
    COALESCE(o.owner, j.owner) AS owner,
    COALESCE(o.sell_token, j.sell_token) AS sell_token,
    COALESCE(o.buy_token, j.buy_token) AS buy_token
FROM trades t
LEFT JOIN orders o ON o.uid = t.order_uid
LEFT JOIN jit_orders j ON j.uid = t.order_uid
WHERE NOT EXISTS (
    SELECT 1
    FROM settlements s
    WHERE s.block_number = t.block_number
    AND   s.log_index > t.log_index
)
ORDER BY t.block_number, t.log_index
"#;

/// An on-chain Trade event with the same order uid as a DB trade. `diffs`
/// lists the fields whose values differ from the DB data; an exact match has
/// no diffs.
struct Candidate {
    block: u64,
    tx_hash: B256,
    log_index: u64,
    diffs: Vec<String>,
}

/// Compares a DB trade against an on-chain Trade event. Returns None if the
/// order uids differ, otherwise the list of mismatching fields.
fn compare(db: &DbTrade, event: &ChainTrade) -> Option<Vec<String>> {
    if db.order_uid != event.order_uid {
        return None;
    }
    let mut diffs = Vec::new();
    {
        let mut check_amount = |name: &str, chain: &U256, db_value: &BigDecimal| {
            if u256_to_big_decimal(chain) != *db_value {
                diffs.push(format!("{name} chain {chain} != db {db_value}"));
            }
        };
        check_amount("sell_amount", &event.sell_amount, &db.sell_amount);
        check_amount("buy_amount", &event.buy_amount, &db.buy_amount);
        check_amount("fee_amount", &event.fee_amount, &db.fee_amount);
    }
    {
        let mut check_address = |name: &str, chain: &Address, db_value: &Option<Vec<u8>>| {
            if let Some(db_value) = db_value
                && db_value.as_slice() != chain.as_slice()
            {
                diffs.push(format!(
                    "{name} chain {chain} != db {}",
                    hex::encode_prefixed(db_value)
                ));
            }
        };
        check_address("sell_token", &event.sell_token, &db.sell_token);
        check_address("buy_token", &event.buy_token, &db.buy_token);
        check_address("owner", &event.owner, &db.owner);
    }
    Some(diffs)
}

fn candidates(db_trade: &DbTrade, txs: &[SettlementTx]) -> Vec<Candidate> {
    txs.iter()
        .flat_map(|tx| {
            tx.trades.iter().filter_map(|event| {
                Some(Candidate {
                    block: tx.block,
                    tx_hash: tx.tx_hash,
                    log_index: event.log_index,
                    diffs: compare(db_trade, event)?,
                })
            })
        })
        .collect()
}

/// The outcome of locating one orphaned DB trade on-chain.
struct TradeReport {
    /// The block the DB recorded the trade at.
    block: u64,
    trade: DbTrade,
    /// Exact matches: same uid, amounts and (when known) tokens/owner.
    matches: Vec<Candidate>,
    /// Same uid but different data.
    near_misses: Vec<Candidate>,
}

impl TradeReport {
    fn status(&self) -> &'static str {
        match self.matches.len() {
            0 => "not_found",
            1 => "located",
            _ => "ambiguous",
        }
    }

    fn order_note(&self) -> &'static str {
        match self.trade.sell_token {
            Some(_) => "",
            None => "order not in orders/jit_orders; matched on uid and amounts only",
        }
    }
}

fn print_report_table(reports: &[TradeReport]) {
    let row = |db_block: &str,
               db_log: &str,
               status: &str,
               block: &str,
               offset: &str,
               log_index: &str,
               tx_hash: &str,
               order_uid: &str,
               diffs: &str| {
        println!(
            "{db_block:<10}  {db_log:>6}  {status:<9}  {block:>10}  {offset:>6}  {log_index:>9}  \
             {tx_hash:<66}  {order_uid:<114}  {diffs}"
        );
    };
    row(
        "db_block",
        "db_log",
        "status",
        "block",
        "offset",
        "log_index",
        "tx_hash",
        "order_uid",
        "diffs",
    );
    row(
        &"-".repeat(10),
        &"-".repeat(6),
        &"-".repeat(9),
        &"-".repeat(10),
        &"-".repeat(6),
        &"-".repeat(9),
        &"-".repeat(66),
        &"-".repeat(114),
        &"-".repeat(5),
    );
    for report in reports {
        let db_block = report.block.to_string();
        let db_log = report.trade.log_index.to_string();
        let uid = hex::encode_prefixed(&report.trade.order_uid);
        let note = report.order_note();
        let candidate_row = |status: &str, candidate: &Candidate, diffs: &str| {
            row(
                &db_block,
                &db_log,
                status,
                &candidate.block.to_string(),
                &format!("{:+}", offset(candidate.block, report.block)),
                &candidate.log_index.to_string(),
                &candidate.tx_hash.to_string(),
                &uid,
                diffs,
            );
        };
        if report.matches.is_empty() && report.near_misses.is_empty() {
            row(
                &db_block,
                &db_log,
                report.status(),
                "",
                "",
                "",
                "",
                &uid,
                note,
            );
        }
        for candidate in &report.matches {
            candidate_row(report.status(), candidate, note);
        }
        for candidate in &report.near_misses {
            let mut diffs = candidate.diffs.join("; ");
            if !note.is_empty() {
                diffs = format!("{diffs}; {note}");
            }
            candidate_row("uid_only", candidate, &diffs);
        }
    }
}

fn candidate_json(candidate: &Candidate, target_block: u64) -> serde_json::Value {
    json!({
        "tx_hash": candidate.tx_hash.to_string(),
        "block": candidate.block,
        "offset": offset(candidate.block, target_block),
        "log_index": candidate.log_index,
        "diffs": candidate.diffs,
    })
}

async fn db_mode(
    provider: &impl Provider,
    address: Address,
    chain_id: u64,
    db_url: &str,
    window: u64,
    json: bool,
) -> Result<()> {
    let mut db = PgConnection::connect(db_url)
        .await
        .context("could not connect to database")?;
    let trades: Vec<DbTrade> = sqlx::query_as(ORPHANED_TRADES_QUERY)
        .fetch_all(&mut db)
        .await
        .context("could not query trades without settlements")?;

    let mut by_block: BTreeMap<u64, Vec<DbTrade>> = BTreeMap::new();
    for trade in trades {
        let block = u64::try_from(trade.block_number).context("negative block number")?;
        by_block.entry(block).or_default().push(trade);
    }
    eprintln!(
        "chain {chain_id} (contract {address}): {} DB trade(s) without settlement event across {} \
         block(s)",
        by_block.values().map(Vec::len).sum::<usize>(),
        by_block.len()
    );

    let mut reports = Vec::new();
    for (block, group) in by_block {
        eprintln!("checking block {block} ({} orphaned trades)", group.len());
        let target = fetch_settlements(provider, address, block, block).await?;
        // Fetched lazily, only when a trade is not found in the target block,
        // and reused by all trades of the block.
        let mut neighborhood: Option<Vec<SettlementTx>> = None;
        for db_trade in group {
            let mut cands = candidates(&db_trade, &target);
            if !cands.iter().any(|c| c.diffs.is_empty()) && window > 0 {
                let txs = match &neighborhood {
                    Some(txs) => txs,
                    None => {
                        let from = block.saturating_sub(window);
                        let to = block.saturating_add(window);
                        eprintln!("  searching neighbor blocks {from}..={to}");
                        neighborhood.insert(fetch_settlements(provider, address, from, to).await?)
                    }
                };
                cands = candidates(&db_trade, txs);
            }
            let (matches, near_misses) = cands.into_iter().partition(|c| c.diffs.is_empty());
            reports.push(TradeReport {
                block,
                trade: db_trade,
                matches,
                near_misses,
            });
        }
    }

    let count = |status: &str| reports.iter().filter(|r| r.status() == status).count();
    let (located, ambiguous, missing) = (count("located"), count("ambiguous"), count("not_found"));

    if json {
        let doc = json!({
            "chain_id": chain_id,
            "contract": address.to_string(),
            "window": window,
            "orphaned_trades": reports.iter().map(|r| json!({
                "db_block": r.block,
                "db_log_index": r.trade.log_index,
                "order_uid": hex::encode_prefixed(&r.trade.order_uid),
                "order_in_db": r.trade.sell_token.is_some(),
                "db_sell_amount": r.trade.sell_amount.to_string(),
                "db_buy_amount": r.trade.buy_amount.to_string(),
                "db_fee_amount": r.trade.fee_amount.to_string(),
                "status": r.status(),
                "matches": r.matches.iter()
                    .map(|c| candidate_json(c, r.block)).collect::<Vec<_>>(),
                "near_misses": r.near_misses.iter()
                    .map(|c| candidate_json(c, r.block)).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
            "summary": {
                "located": located,
                "ambiguous": ambiguous,
                "not_found": missing,
            },
        });
        println!("{}", serde_json::to_string_pretty(&doc)?);
    } else if reports.is_empty() {
        println!("every DB trade resolves to a settlement event, nothing to do");
    } else {
        print_report_table(&reports);
        println!("\nsummary: {located} located, {ambiguous} ambiguous, {missing} not found");
    }

    if ambiguous + missing > 0 {
        std::process::exit(1);
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    if let Some(uid) = &args.order_uid {
        ensure!(
            uid.len() == 56,
            "--order-uid must be 56 bytes, got {}",
            uid.len()
        );
    }
    let (provider, _wallet) = ethrpc::alloy::unbuffered_provider(&args.rpc_url, None);

    let chain_id = provider
        .get_chain_id()
        .await
        .context("could not fetch chain id")?;
    let address = match args.settlement {
        Some(address) => address,
        None => GPv2Settlement::deployment_address(&chain_id).with_context(|| {
            format!("no known settlement deployment for chain {chain_id}, pass --settlement")
        })?,
    };

    match (&args.db, args.block) {
        (Some(db), _) => db_mode(&provider, address, chain_id, db, args.window, args.json).await,
        (None, Some(block)) => {
            let filter = TradeFilter {
                order_uid: args.order_uid,
                owner: args.owner,
                sell_token: args.sell_token,
                buy_token: args.buy_token,
                sell_amount: args.sell_amount,
                buy_amount: args.buy_amount,
                fee_amount: args.fee_amount,
            };
            block_mode(
                &provider,
                address,
                chain_id,
                block,
                args.window,
                &filter,
                args.json,
            )
            .await
        }
        (None, None) => unreachable!("clap requires block unless --db is used"),
    }
}
