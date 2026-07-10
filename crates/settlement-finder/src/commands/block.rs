//! The `block` subcommand: print the settlement transactions of a block.

use {
    crate::{
        chain::{
            ChainTrade,
            SettlementSource,
            SettlementTx,
            fetch_settlements,
            format_sources,
            offset,
            offset_suffix,
        },
        filter::FilterArgs,
    },
    alloy_primitives::hex,
    alloy_provider::Provider,
    anyhow::Result,
    serde_json::json,
};

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

pub async fn block_cmd(
    provider: &impl Provider,
    sources: &[SettlementSource],
    chain_id: u64,
    block: u64,
    window: u64,
    filter: &FilterArgs,
    json: bool,
) -> Result<()> {
    filter.validate()?;
    let subject = match filter.is_active() {
        true => "matching trade",
        false => "settlement",
    };

    let mut txs = fetch_settlements(provider, sources, block, block).await?;
    filter.apply(&mut txs);

    let mut searched_blocks = None;
    if txs.is_empty() && window > 0 {
        let from = block.saturating_sub(window);
        let to = block.saturating_add(window);
        tracing::info!(
            subject,
            block,
            from,
            to,
            "no match in block, searching neighbors"
        );
        txs = fetch_settlements(provider, sources, from, to).await?;
        filter.apply(&mut txs);
        txs.sort_by_key(|tx| (tx.block, tx.tx_index));
        searched_blocks = Some((from, to));
    }

    if json {
        let doc = json!({
            "chain_id": chain_id,
            "contracts": sources.iter().map(ToString::to_string).collect::<Vec<_>>(),
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
            "chain {chain_id} (contract {}): {} settlement transaction(s)",
            format_sources(sources),
            txs.len()
        );
        for tx in &txs {
            print_tx(tx, block);
        }
    }

    if txs.is_empty() {
        tracing::warn!(
            subject,
            block,
            window,
            chain_id,
            contract = %format_sources(sources),
            "no match in block or window"
        );
        std::process::exit(1);
    }
    Ok(())
}
