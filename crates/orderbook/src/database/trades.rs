use {
    crate::database::Postgres,
    alloy::primitives::{Address, B256},
    anyhow::{Context, Result},
    database::{byte_array::ByteArray, trades::TradesQueryRow},
    futures::stream::TryStreamExt,
    model::{fee_policy::ExecutedProtocolFee, order::OrderUid, trade::Trade},
    number::conversions::big_decimal_to_big_uint,
    std::convert::TryInto,
};

#[async_trait::async_trait]
pub trait TradeRetrieving: Send + Sync {
    async fn trades(&self, filter: &TradeFilter) -> Result<Vec<Trade>>;
}

#[async_trait::async_trait]
pub trait TradeRetrievingPaginated: Send + Sync {
    async fn trades_paginated(&self, filter: &PaginatedTradeFilter) -> Result<Vec<Trade>>;
}

/// Any default value means that this field is unfiltered.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct TradeFilter {
    pub owner: Option<Address>,
    pub order_uid: Option<OrderUid>,
}

/// Trade filter with pagination support (for v2 API).
#[derive(Debug, Default, Eq, PartialEq)]
pub struct PaginatedTradeFilter {
    pub owner: Option<Address>,
    pub order_uid: Option<OrderUid>,
    pub offset: u64,
    pub limit: u64,
}

#[async_trait::async_trait]
impl TradeRetrieving for Postgres {
    async fn trades(&self, filter: &TradeFilter) -> Result<Vec<Trade>> {
        let timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["trades"])
            .start_timer();

        let mut ex = self.pool.acquire().await?;
        // For v1 API, return all results without pagination (use large default values)
        let trades = database::trades::trades(
            &mut ex,
            filter.owner.map(|owner| ByteArray(owner.0.0)).as_ref(),
            filter.order_uid.map(|uid| ByteArray(uid.0)).as_ref(),
            0,
            i64::MAX,
        )
        .into_inner()
        .map_err(anyhow::Error::from)
        .try_collect::<Vec<TradesQueryRow>>()
        .await?;
        timer.stop_and_record();

        let auction_order_uids = trades
            .iter()
            .filter_map(|t| t.auction_id.map(|auction_id| (auction_id, t.order_uid)))
            .collect::<Vec<_>>();

        if auction_order_uids.len() >= u16::MAX as usize {
            // We use these ids as arguments for an SQL query and sqlx only allows
            // u16::MAX arguments. To avoid a panic later on we return an error here.
            anyhow::bail!("query response too large");
        }

        let executed_protocol_fees = self
            .executed_protocol_fees(auction_order_uids.as_slice())
            .await?;

        trades
            .into_iter()
            .map(|trade| {
                let executed_protocol_fees = trade
                    .auction_id
                    .map(|auction_id| {
                        executed_protocol_fees
                            .get(&(auction_id, trade.order_uid))
                            .cloned()
                            .unwrap_or_default()
                    })
                    .unwrap_or_default();
                trade_from(trade, executed_protocol_fees)
            })
            .collect::<Result<Vec<_>>>()
    }
}

#[async_trait::async_trait]
impl TradeRetrievingPaginated for Postgres {
    async fn trades_paginated(&self, filter: &PaginatedTradeFilter) -> Result<Vec<Trade>> {
        let timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["trades_paginated"])
            .start_timer();

        let mut ex = self.pool.acquire().await?;
        let trades = database::trades::trades(
            &mut ex,
            filter.owner.map(|owner| ByteArray(owner.0.0)).as_ref(),
            filter.order_uid.map(|uid| ByteArray(uid.0)).as_ref(),
            filter
                .offset
                .try_into()
                .context("offset too large for database")?,
            filter
                .limit
                .try_into()
                .context("limit too large for database")?,
        )
        .into_inner()
        .map_err(anyhow::Error::from)
        .try_collect::<Vec<TradesQueryRow>>()
        .await?;
        timer.stop_and_record();

        let auction_order_uids = trades
            .iter()
            .filter_map(|t| t.auction_id.map(|auction_id| (auction_id, t.order_uid)))
            .collect::<Vec<_>>();

        if auction_order_uids.len() >= u16::MAX as usize {
            // We use these ids as arguments for an SQL query and sqlx only allows
            // u16::MAX arguments. To avoid a panic later on we return an error here.
            anyhow::bail!("query response too large");
        }

        let executed_protocol_fees = self
            .executed_protocol_fees(auction_order_uids.as_slice())
            .await?;

        trades
            .into_iter()
            .map(|trade| {
                let executed_protocol_fees = trade
                    .auction_id
                    .map(|auction_id| {
                        executed_protocol_fees
                            .get(&(auction_id, trade.order_uid))
                            .cloned()
                            .unwrap_or_default()
                    })
                    .unwrap_or_default();
                trade_from(trade, executed_protocol_fees)
            })
            .collect::<Result<Vec<_>>>()
    }
}

fn trade_from(
    row: TradesQueryRow,
    executed_protocol_fees: Vec<ExecutedProtocolFee>,
) -> Result<Trade> {
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
    let owner = Address::from_slice(&row.owner.0);
    let buy_token = Address::from_slice(&row.buy_token.0);
    let sell_token = Address::from_slice(&row.sell_token.0);
    let tx_hash = row.tx_hash.map(|hash| B256::from_slice(&hash.0));
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
        executed_protocol_fees,
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
