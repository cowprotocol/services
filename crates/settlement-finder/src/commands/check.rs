//! The `check` subcommand: locate orphaned DB trades on-chain and report how
//! their settlement is (mis)indexed in the DB.

use {
    crate::{
        chain::{SettlementSource, offset},
        db::{DbSettlement, db_settlements_by_tx, db_trade_exists},
        orphans::{Candidate, TradeReport, locate_orphans},
    },
    alloy_primitives::hex,
    alloy_provider::Provider,
    anyhow::{Context, Result},
    serde_json::json,
    sqlx::{Connection, PgConnection},
};

/// Where the DB has indexed the settlement of a located match, if anywhere.
enum SettlementInDb {
    /// No settlements row with the match's tx hash.
    Missing,
    /// A settlements row with the tx hash exists at the match's block.
    Indexed { log_index: i64 },
    /// A settlements row with the tx hash exists, but at another block: the
    /// settlement itself was indexed from an orphaned fork too.
    Elsewhere { block: i64, log_index: i64 },
}

impl SettlementInDb {
    fn classify(candidate: &Candidate, rows: &[DbSettlement]) -> Self {
        match rows
            .iter()
            .find(|row| row.block_number == candidate.block.cast_signed())
        {
            Some(row) => Self::Indexed {
                log_index: row.log_index,
            },
            None => match rows.first() {
                Some(row) => Self::Elsewhere {
                    block: row.block_number,
                    log_index: row.log_index,
                },
                None => Self::Missing,
            },
        }
    }

    fn cell(&self) -> String {
        match self {
            Self::Missing => "missing".to_string(),
            Self::Indexed { .. } => "indexed".to_string(),
            Self::Elsewhere { block, log_index } => format!("at {block}/{log_index}"),
        }
    }

    fn json(&self) -> serde_json::Value {
        match self {
            Self::Missing => json!({"status": "missing"}),
            Self::Indexed { log_index } => {
                json!({"status": "indexed", "log_index": log_index})
            }
            Self::Elsewhere { block, log_index } => {
                json!({"status": "elsewhere", "block": block, "log_index": log_index})
            }
        }
    }
}

/// DB state of a located match: where its settlement is indexed and whether a
/// trades row already exists at the located coordinates.
struct MatchDbState {
    settlement: SettlementInDb,
    trade_at_match: bool,
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

fn print_check_table(annotated: &[(TradeReport, Vec<MatchDbState>)]) {
    let row = |db_block: &str,
               db_log: &str,
               status: &str,
               block: &str,
               offset: &str,
               log_index: &str,
               db_settlement: &str,
               db_trade: &str,
               tx_hash: &str,
               order_uid: &str,
               diffs: &str| {
        println!(
            "{db_block:<10}  {db_log:>6}  {status:<9}  {block:>10}  {offset:>6}  {log_index:>9}  \
             {db_settlement:<18}  {db_trade:<8}  {tx_hash:<66}  {order_uid:<114}  {diffs}"
        );
    };
    row(
        "db_block",
        "db_log",
        "status",
        "block",
        "offset",
        "log_index",
        "db_settlement",
        "db_trade",
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
        &"-".repeat(18),
        &"-".repeat(8),
        &"-".repeat(66),
        &"-".repeat(114),
        &"-".repeat(5),
    );
    for (report, states) in annotated {
        let db_block = report.block.to_string();
        let db_log = report.trade.log_index.to_string();
        let uid = hex::encode_prefixed(&report.trade.order_uid);
        let note = report.order_note();
        let candidate_row =
            |status: &str, candidate: &Candidate, state: Option<&MatchDbState>, diffs: &str| {
                row(
                    &db_block,
                    &db_log,
                    status,
                    &candidate.block.to_string(),
                    &format!("{:+}", offset(candidate.block, report.block)),
                    &candidate.log_index.to_string(),
                    &state.map(|s| s.settlement.cell()).unwrap_or_default(),
                    state
                        .map(|s| {
                            if s.trade_at_match {
                                "present"
                            } else {
                                "absent"
                            }
                        })
                        .unwrap_or_default(),
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
                "",
                "",
                &uid,
                note,
            );
        }
        for (candidate, state) in report.matches.iter().zip(states) {
            candidate_row(report.status(), candidate, Some(state), note);
        }
        for candidate in &report.near_misses {
            let mut diffs = candidate.diffs.join("; ");
            if !note.is_empty() {
                diffs = format!("{diffs}; {note}");
            }
            candidate_row("uid_only", candidate, None, &diffs);
        }
    }
}

pub async fn check_cmd(
    provider: &impl Provider,
    sources: &[SettlementSource],
    chain_id: u64,
    db_url: &str,
    window: u64,
    max_orphan_blocks: u64,
    json: bool,
) -> Result<()> {
    let mut db = PgConnection::connect(db_url)
        .await
        .context("could not connect to database")?;
    let reports = locate_orphans(provider, sources, &mut db, window, max_orphan_blocks).await?;

    let mut annotated = Vec::new();
    for report in reports {
        let mut states = Vec::new();
        for candidate in &report.matches {
            let rows = db_settlements_by_tx(&mut db, candidate.tx_hash.as_slice()).await?;
            states.push(MatchDbState {
                settlement: SettlementInDb::classify(candidate, &rows),
                trade_at_match: db_trade_exists(&mut db, candidate.block, candidate.log_index)
                    .await?,
            });
        }
        annotated.push((report, states));
    }

    let count = |status: &str| {
        annotated
            .iter()
            .filter(|(r, _)| r.status() == status)
            .count()
    };
    let (located, ambiguous, missing) = (count("located"), count("ambiguous"), count("not_found"));

    if json {
        let doc = json!({
            "chain_id": chain_id,
            "contracts": sources.iter().map(ToString::to_string).collect::<Vec<_>>(),
            "window": window,
            "orphaned_trades": annotated.iter().map(|(r, states)| json!({
                "db_block": r.block,
                "db_log_index": r.trade.log_index,
                "order_uid": hex::encode_prefixed(&r.trade.order_uid),
                "order_in_db": r.trade.sell_token.is_some(),
                "db_sell_amount": r.trade.sell_amount.to_string(),
                "db_buy_amount": r.trade.buy_amount.to_string(),
                "db_fee_amount": r.trade.fee_amount.to_string(),
                "status": r.status(),
                "matches": r.matches.iter().zip(states).map(|(c, state)| {
                    let mut value = candidate_json(c, r.block);
                    let object = value.as_object_mut().unwrap();
                    object.insert("db_settlement".into(), state.settlement.json());
                    object.insert("db_trade_at_match".into(), state.trade_at_match.into());
                    value
                }).collect::<Vec<_>>(),
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
    } else if annotated.is_empty() {
        println!("every DB trade resolves to a settlement event, nothing to do");
    } else {
        print_check_table(&annotated);
        println!("\nsummary: {located} located, {ambiguous} ambiguous, {missing} not found");
    }

    if ambiguous + missing > 0 {
        std::process::exit(1);
    }
    Ok(())
}
