//! The `block` subcommand: print the settlement transactions of a block.

use {
    crate::{
        chain::{ChainTrade, SettlementSource, fetch_settlements, format_sources, offset},
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

pub async fn block_cmd(
    provider: &impl Provider,
    sources: &[SettlementSource],
    chain_id: u64,
    block: u64,
    window: u64,
    filter: &FilterArgs,
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
