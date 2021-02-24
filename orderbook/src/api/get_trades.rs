use crate::database::{Database, TradeFilter};
use anyhow::Result;
use futures::TryStreamExt;
use model::trade::Trade;

#[allow(dead_code)]
pub async fn get_trades(db: &Database, filter: &TradeFilter) -> Result<Vec<Trade>> {
    Ok(db.trades(filter).try_collect::<Vec<_>>().await?)
}
