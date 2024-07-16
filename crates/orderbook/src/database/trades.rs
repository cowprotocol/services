use {
    crate::database::Postgres,
    anyhow::{Context, Result},
    database::{byte_array::ByteArray, trades::TradesQueryRow},
    ethcontract::H160,
    futures::stream::TryStreamExt,
    model::{fee_policy::FeePolicy, order::OrderUid, trade::Trade},
    number::conversions::big_decimal_to_big_uint,
    primitive_types::H256,
    std::convert::TryInto,
};

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
        .map_err(anyhow::Error::from)
        .and_then(|trade| async move {
            match trade.auction_id {
                Some(auction_id) => self.fee_policies(auction_id, trade.order_uid).await,
                None => Ok(vec![]),
            }
            .and_then(|fee_policies| trade_from(trade, fee_policies))
        })
        .try_collect::<Vec<Trade>>()
        .await
    }
}

fn trade_from(row: TradesQueryRow, fee_policies: Vec<FeePolicy>) -> Result<Trade> {
    let block_number = row
        .block_number
        .try_into()
        .context("block_number is not u32")?;
    let log_index = row.log_index.try_into().context("log_index is not u32")?;
    let order_uid = OrderUid(row.order_uid.0);
    let buy_amount = big_decimal_to_big_uint(&row.buy_amount)
        .context("buy_amount is not an unsigned integer")?;
    let sell_amount = big_decimal_to_big_uint(&row.sell_amount)
        .context("sell_amount is not an unsigned integer")?;
    let sell_amount_before_fees = big_decimal_to_big_uint(&row.sell_amount_before_fees)
        .context("sell_amount_before_fees is not an unsigned integer")?;
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
        fee_policies,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_trade() {
        trade_from(TradesQueryRow::default(), vec![]).unwrap();
    }
}
