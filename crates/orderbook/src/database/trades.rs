use crate::database::Postgres;
use anyhow::{anyhow, Context, Result};
use database::{byte_array::ByteArray, trades::TradesQueryRow};
use ethcontract::H160;
use futures::{stream::TryStreamExt, StreamExt};
use model::{order::OrderUid, trade::Trade};
use number_conversions::big_decimal_to_big_uint;
use primitive_types::H256;
use std::convert::TryInto;

#[async_trait::async_trait]
pub trait TradeRetrieving: Send + Sync {
    async fn trades(&self, filter: &TradeFilter) -> Result<Vec<Trade>>;
}

/// Any default value means that this field is unfiltered.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct TradeFilter {
    pub owner: Option<H160>,
    pub order_uid: Option<OrderUid>,
}

#[async_trait::async_trait]
impl TradeRetrieving for Postgres {
    async fn trades(&self, filter: &TradeFilter) -> Result<Vec<Trade>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["trades"])
            .start_timer();

        let mut ex = self.pool.acquire().await?;
        database::trades::trades(
            &mut ex,
            filter.owner.map(|owner| ByteArray(owner.0)).as_ref(),
            filter.order_uid.map(|uid| ByteArray(uid.0)).as_ref(),
        )
        .map(|result| match result {
            Ok(row) => trade_from(row),
            Err(err) => Err(anyhow::Error::from(err)),
        })
        .try_collect()
        .await
    }
}

fn trade_from(row: TradesQueryRow) -> Result<Trade> {
    let block_number = row
        .block_number
        .try_into()
        .context("block_number is not u32")?;
    let log_index = row.log_index.try_into().context("log_index is not u32")?;
    let order_uid = OrderUid(row.order_uid.0);
    let buy_amount = big_decimal_to_big_uint(&row.buy_amount)
        .ok_or_else(|| anyhow!("buy_amount is not an unsigned integer"))?;
    let sell_amount = big_decimal_to_big_uint(&row.sell_amount)
        .ok_or_else(|| anyhow!("sell_amount is not an unsigned integer"))?;
    let sell_amount_before_fees = big_decimal_to_big_uint(&row.sell_amount_before_fees)
        .ok_or_else(|| anyhow!("sell_amount_before_fees is not an unsigned integer"))?;
    let owner = H160(row.owner.0);
    let buy_token = H160(row.buy_token.0);
    let sell_token = H160(row.sell_token.0);
    let tx_hash = row.tx_hash.map(|hash| H256(hash.0));
    Ok(Trade {
        block_number,
        log_index,
        order_uid,
        buy_amount,
        sell_amount,
        sell_amount_before_fees,
        owner,
        buy_token,
        sell_token,
        tx_hash,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_trade() {
        trade_from(TradesQueryRow::default()).unwrap();
    }
}
