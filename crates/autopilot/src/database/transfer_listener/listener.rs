//! Real-time Transfer event listener that cancels orders when tokens are
//! transferred away.

use {
    crate::database::{Metrics, Postgres},
    alloy::{
        primitives::{Address, B256, b256},
        providers::Provider,
        rpc::types::Log,
    },
    anyhow::{Result, anyhow},
    chrono::{DateTime, Utc},
    database::{
        OrderUid,
        byte_array::ByteArray,
        order_events::{OrderEvent, OrderEventLabel, insert_order_event},
    },
    shared::ethrpc::Web3,
    sqlx::{PgConnection, QueryBuilder},
};

/// The ERC20 Transfer event signature hash:
/// keccak256("Transfer(address,address,uint256)")
const TRANSFER_SIGNATURE: B256 =
    b256!("ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef");

/// Represents a decoded Transfer event
#[derive(Clone, Copy, Debug)]
pub struct TransferEvent {
    /// The address sending tokens
    pub from: Address,
    /// The address receiving tokens
    pub to: Address,
    /// The token contract address
    pub token: Address,
    /// Block number where the transfer occurred
    pub block_number: u64,
}

impl TransferEvent {
    /// Decode a Transfer event from a log
    pub fn from_log(log: &Log) -> Option<Self> {
        // Transfer event has 3 topics: [signature_hash, from (indexed), to (indexed)]
        // and data contains the value (uint256)
        let topics = log.topics();
        if topics.len() < 3 {
            return None;
        }

        // Verify it's a Transfer event
        if topics[0].0 != TRANSFER_SIGNATURE {
            return None;
        }

        // Extract addresses from topics. Topics are padded to 32 bytes (B256),
        // so indexed address topics have the address in the last 20 bytes.
        // Use from_word which handles the conversion from a 32-byte word to address.
        let from = Address::from_word(topics[1]);
        let to = Address::from_word(topics[2]);
        let token = log.address();
        let block_number = log.block_number?;

        Some(TransferEvent {
            from,
            to,
            token,
            block_number,
        })
    }
}

pub struct TransferListener {
    db: Postgres,
    web3: Web3,
    /// Addresses to ignore/exclude from transfer event processing
    /// (e.g., settlement contract, vault relayer)
    ignored_addresses: std::collections::HashSet<Address>,
}

impl TransferListener {
    pub fn new(db: Postgres, web3: Web3, ignored_addresses: Vec<Address>) -> Self {
        Self {
            db,
            web3,
            ignored_addresses: ignored_addresses.into_iter().collect(),
        }
    }

    /// Fetch and process Transfer events from a specific block
    pub async fn process_block(&self, block_number: u64) -> Result<()> {
        // Fetch all receipts for this block (includes all logs)
        // This is more efficient than filtering on the RPC side
        let receipts = self
            .web3
            .alloy
            .get_block_receipts(block_number.into())
            .await?;

        // Collect all logs from receipts and manually filter for Transfer events that
        // are *NOT* related to our ignored contracts (settlement, vault
        // relayer, etc.)
        let logs: Vec<Log> = receipts
            .into_iter()
            .flat_map(|receipt| {
                receipt.into_iter().flat_map(|r| {
                    r.logs()
                        .iter()
                        .filter_map(|l| {
                            if l.topic0().eq(&Some(&TRANSFER_SIGNATURE)) {
                                // Extract the from and to addresses from topics
                                let topics = l.topics();
                                if topics.len() < 3 {
                                    return None;
                                }

                                let from = Address::from_word(topics[1]);
                                let to = Address::from_word(topics[2]);

                                // Exclude transfers involving any ignored addresses
                                if self.ignored_addresses.contains(&from)
                                    || self.ignored_addresses.contains(&to)
                                {
                                    return None;
                                }

                                Some(l.clone())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();

        tracing::debug!(logs_count = ?logs.len(), "Cancelling orders matching transfer events");

        if !logs.is_empty() {
            self.process_transfer_events(logs).await
        } else {
            Ok(())
        }
    }

    /// Process Transfer events and cancel matching orders
    /// All transfers from a block are processed in a single batch transaction
    pub async fn process_transfer_events(&self, logs: Vec<Log>) -> Result<()> {
        // Decode all transfer events
        let transfers: Vec<TransferEvent> =
            logs.iter().filter_map(TransferEvent::from_log).collect();

        if transfers.is_empty() {
            tracing::warn!("transfers empty after parsing logs");
            return Ok(());
        }

        tracing::debug!(
            transfers_count = transfers.len(),
            "processing transfer events"
        );

        // Find and cancel all matching live orders in a single transaction
        let mut ex = self.db.pool.begin().await?;
        let cancelled_count = cancel_matching_orders(&mut ex, &transfers).await?;
        ex.commit().await?;

        if cancelled_count > 0 {
            let _timer = Metrics::get()
                .database_queries
                .with_label_values(&["transfer_listener_cancel_orders"])
                .start_timer();

            tracing::debug!(
                "Transfer listener: cancelled {} orders from {} transfer events",
                cancelled_count,
                transfers.len()
            );
        } else {
            tracing::debug!("no orders were cancelled for {} transfers", transfers.len());
        }

        Ok(())
    }
}

/// Cancel all live orders matching the given transfers in a single batch
/// transaction. Uses the same `live_orders` logic as `solvable_orders` to
/// ensure we only cancel truly active orders (not expired, not invalidated,
/// etc).
async fn cancel_matching_orders(ex: &mut PgConnection, transfers: &[TransferEvent]) -> Result<u64> {
    if transfers.is_empty() {
        return Ok(0);
    }

    let now = Utc::now();

    // Build a dynamic query with all the (owner, sell_token) pairs
    let mut owner_tokens = Vec::new();
    for transfer in transfers {
        owner_tokens.push((transfer.from, transfer.token));
    }

    // Find all live orders matching any of the (owner, sell_token) pairs
    let order_uids = find_live_orders_to_cancel(ex, &owner_tokens).await?;

    if order_uids.is_empty() {
        tracing::debug!("no live orders matched any transfer events");
        return Ok(0);
    }

    // Update all orders' cancellation_timestamp in a single query
    update_cancellation_timestamps(ex, &order_uids, now).await?;

    // Insert cancellation events in a single batch
    insert_cancellation_events(ex, &order_uids, now).await?;

    Ok(order_uids.len() as u64)
}

/// Find all live orders that match any of the (owner, sell_token) pairs.
/// A live order is one that:
/// - Has NOT been cancelled via the API (cancellation_timestamp IS NULL)
/// - Has NOT been invalidated (various invalidation tables)
/// - For ethflow orders, has NOT been invalidated by ethflow-specific logic
async fn find_live_orders_to_cancel(
    ex: &mut PgConnection,
    owner_token_pairs: &[(Address, Address)],
) -> Result<Vec<OrderUid>> {
    // Collect all owners and tokens into separate vectors explicitly typed as bytea
    let owners: Vec<Vec<u8>> = owner_token_pairs
        .iter()
        .map(|(o, _)| o.0.to_vec())
        .collect();
    let tokens: Vec<Vec<u8>> = owner_token_pairs
        .iter()
        .map(|(_, t)| t.0.to_vec())
        .collect();

    // Note: We don't filter by valid_to here because transfers can happen at any
    // time, and an expired order should still be cancelled to reflect the
    // user's intent
    const QUERY: &str = r#"
SELECT o.uid
FROM orders o
WHERE o.cancellation_timestamp IS NULL
  AND (o.owner, o.sell_token) IN (
    SELECT DISTINCT sp.owner, sp.sell_token
    FROM (
        SELECT UNNEST($1::bytea[]) as owner, UNNEST($2::bytea[]) as sell_token
    ) sp
  )
  AND NOT EXISTS (SELECT 1 FROM invalidations i WHERE i.order_uid = o.uid)
  AND NOT EXISTS (SELECT 1 FROM onchain_order_invalidations oi WHERE oi.uid = o.uid)
  AND NOT EXISTS (SELECT 1 FROM onchain_placed_orders op WHERE op.uid = o.uid AND op.placement_error IS NOT NULL)
  AND (
    NOT EXISTS (SELECT 1 FROM ethflow_orders e WHERE e.uid = o.uid)
    OR EXISTS (
        SELECT 1 FROM ethflow_orders e
        WHERE e.uid = o.uid
        AND (e.valid_to IS NULL OR e.valid_to >= EXTRACT(EPOCH FROM NOW())::bigint)
    )
  )
    "#;

    let rows: Vec<(Vec<u8>,)> = sqlx::query_as(QUERY)
        .bind(&owners as &[Vec<u8>])
        .bind(&tokens as &[Vec<u8>])
        .fetch_all(ex)
        .await?;

    rows.into_iter()
        .map(|(uid_bytes,)| {
            let array: [u8; 56] = uid_bytes
                .try_into()
                .map_err(|_| anyhow!("Invalid order UID length"))?;
            Ok(ByteArray(array))
        })
        .collect()
}

/// Update the cancellation timestamp for all given orders
async fn update_cancellation_timestamps(
    ex: &mut PgConnection,
    order_uids: &[OrderUid],
    timestamp: DateTime<Utc>,
) -> Result<()> {
    if order_uids.is_empty() {
        return Ok(());
    }

    let mut query_builder: QueryBuilder<sqlx::Postgres> =
        QueryBuilder::new("UPDATE orders SET cancellation_timestamp = ");
    query_builder.push_bind(timestamp);
    query_builder.push(" WHERE uid IN (");

    let mut separated = query_builder.separated(", ");
    for order_uid in order_uids {
        separated.push_bind(order_uid.0.as_ref());
    }
    query_builder.push(")");

    query_builder.build().execute(ex).await?;

    Ok(())
}

/// Insert cancellation events for all given orders
async fn insert_cancellation_events(
    ex: &mut PgConnection,
    order_uids: &[OrderUid],
    timestamp: DateTime<Utc>,
) -> Result<()> {
    if order_uids.is_empty() {
        return Ok(());
    }

    // Use the standard insert_order_event function for each order to respect
    // the deduplication logic (don't insert if the last event is already Cancelled)
    for order_uid in order_uids {
        insert_order_event(
            ex,
            &OrderEvent {
                order_uid: *order_uid,
                timestamp,
                label: OrderEventLabel::Cancelled,
            },
        )
        .await?;

        tracing::debug!(
            ?order_uid,
            "Order cancelled due to transfer of order token from owner"
        );
    }

    Ok(())
}
