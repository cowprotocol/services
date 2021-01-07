use crate::database::{Database, Trade as DbTrade};
use anyhow::{anyhow, Context, Error, Result};
use contracts::{
    g_pv_2_settlement::{event_data::Trade as ContractTrade, Event as ContractEvent},
    GPv2Settlement,
};
use ethcontract::{errors::ExecutionError, Event, EventMetadata};
use futures::{Stream, StreamExt, TryStreamExt};
use model::order::OrderUid;
use std::{convert::TryInto, ops::RangeInclusive, sync::Arc};
use web3::{Transport, Web3};

// We expect that there is never a reorg that changes more than the last n blocks.
const MAX_REORG_BLOCK_COUNT: u64 = 25;
// When we insert new trade events into the database we will insert at most this many in one
// transaction.
const INSERT_TRADE_BATCH_SIZE: usize = 250;

pub struct TradeEvents {
    contract: GPv2Settlement,
    db: Arc<dyn Database>,
}

impl TradeEvents {
    /// Get new events from the contract and insert them into the database.
    pub async fn update_events(&mut self) -> Result<()> {
        let range = self.event_block_range().await?;
        let events = self
            .past_events(&range)
            .await
            .context("failed to get past events")?
            .ready_chunks(INSERT_TRADE_BATCH_SIZE)
            .map(|chunk| chunk.into_iter().collect::<Result<Vec<_>, _>>());
        futures::pin_mut!(events);
        // We intentionally do not go with the obvious approach of deleting old events first and
        // then inserting new ones. Instead, we make sure that the deletion and the insertion of the
        // first batch of events happen in one transaction.
        // This is important for two reasons:
        // 1. It ensures that we only delete if we really have new events. Otherwise if fetching new
        //    events from the node fails for whatever reason we might keep deleting events over and
        //    over without inserting new ones resulting in the database table getting cleared.
        // 2. It ensures that other users of the database are unlikely to see an inconsistent state
        //    some events have been deleted but new ones not yet inserted. This is important in case
        //    for example another part of the code calculates the total executed amount of an order.
        //    If this happened right after deletion but before insertion, then the result would be
        //    wrong. In theory this could still happen if the last MAX_REORG_BLOCK_COUNT blocks had
        //    more than INSERT_TRADE_BATCH_SIZE trade events but this is unlikely.
        // There alternative solutions for 2. but this one is the most practical. For example, we
        // could keep all reorgable events in this struct and only store ones that are older than
        // MAX_REORG_BLOCK_COUNT in the database but then any code using trade events would have to
        // go through this class instead of being able to work with the database directly.
        // Or we could make the batch size unlimited but this runs into problems when we have not
        // updated it in a long time resulting in many missing events which we would all have to
        // in one transaction.
        let mut have_deleted_old_events = false;
        while let Some(trades) = events.next().await {
            let trades = trades.context("failed to get event")?;
            if !have_deleted_old_events {
                self.db
                    .replace_trades(*range.start(), &[])
                    .await
                    .context("failed to replace trades")?;
                have_deleted_old_events = true;
            } else {
                self.db
                    .insert_trades(trades.as_slice())
                    .await
                    .context("failed to insert trades")?;
            }
        }
        Ok(())
    }

    async fn event_block_range(&self) -> Result<RangeInclusive<u64>> {
        let web3 = self.web3();
        let last_handled_block = self
            .db
            .block_number_of_most_recent_trade()
            .await
            .context("failed to get last handle block")?;
        let current_block = web3
            .eth()
            .block_number()
            .await
            .context("failed to get current block")?
            .as_u64();
        let from_block = last_handled_block.saturating_sub(MAX_REORG_BLOCK_COUNT);
        anyhow::ensure!(
            from_block <= current_block,
            format!(
                "current block number according to node is {} which is more than {} blocks in the \
                 past compared to last handled block {}",
                current_block, MAX_REORG_BLOCK_COUNT, last_handled_block
            )
        );
        Ok(from_block..=current_block)
    }

    fn web3(&self) -> Web3<impl Transport> {
        self.contract.raw_instance().web3()
    }

    async fn past_events(
        &self,
        block_range: &RangeInclusive<u64>,
    ) -> Result<impl Stream<Item = Result<DbTrade>>, ExecutionError> {
        Ok(self
            .contract
            .all_events()
            .from_block((*block_range.start()).into())
            .to_block((*block_range.end()).into())
            .block_page_size(1000)
            .query_paginated()
            .await?
            .map_err(Error::from)
            .try_filter_map(|Event { data, meta }| async move {
                Ok(match data {
                    ContractEvent::Trade(trade) => Some((trade, meta)),
                })
            })
            .and_then(|(trade, meta)| async move {
                match meta {
                    Some(meta) => Ok((trade, meta)),
                    None => Err(anyhow!("event without metadata")),
                }
            })
            .and_then(|(trade, meta)| async move { convert_trade(&trade, &meta) }))
    }
}

fn convert_trade(trade: &ContractTrade, meta: &EventMetadata) -> Result<DbTrade> {
    let order_uid = OrderUid(
        trade
            .order_uid
            .as_slice()
            .try_into()
            .context("trade event order_uid has wrong number of bytes")?,
    );
    Ok(DbTrade {
        block_number: meta.block_number,
        log_index: meta.log_index as u64,
        order_uid,
        sell_amount: trade.sell_amount,
        buy_amount: trade.buy_amount,
        fee_amount: trade.fee_amount,
    })
}
