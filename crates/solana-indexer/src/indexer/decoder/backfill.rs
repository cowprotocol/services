//! RPC backfill: recover the gap between the persisted watermark and the
//! live tip when it exceeds the stream provider's replay window.
//!
//! The node's transaction history is deep where the stream's replay is
//! shallow, so recovery scans the tracked programs' signatures back to the
//! watermark, re-fetches each transaction, and pushes it through the same
//! decode and flush path the stream uses. The watermark advances with every
//! flushed slot and ends at the scanned tip, which puts the next stream
//! subscription back inside the replay window.

use {
    super::{Decoder, SlotBuffer},
    crate::types::{
        Signature,
        errors::PersistenceError,
        slot::Slot,
        wire::{
            CompiledInstruction,
            InnerInstruction,
            InnerInstructions,
            Message,
            MessageAddressTableLookup,
            MessageHeader,
            SubscribeUpdateTransactionInfo,
            Transaction,
            TransactionError,
            TransactionStatusMeta,
        },
    },
    cow_solana_rpc::EncodedConfirmedTransactionWithStatusMeta,
    solana_sdk::{bs58, message::VersionedMessage},
    solana_transaction_status_client_types::{UiInstruction, option_serializer::OptionSerializer},
    std::collections::BTreeMap,
};

impl Decoder {
    /// Index every tracked transaction between the persisted watermark and
    /// the live tip from RPC history, then advance the watermark to that
    /// tip. A missing watermark is a cold start with nothing to recover.
    ///
    /// A failure leaves the watermark wherever the last complete slot flush
    /// put it and records the remaining gap as a lost range, so the caller
    /// can fall back to a live-tip subscription without losing track of the
    /// hole.
    pub(crate) async fn backfill(&self) -> Result<(), PersistenceError> {
        let result = self.backfill_inner().await;
        if result.is_err()
            && let (Ok(Some(from)), Ok(tip)) = (
                self.persistence.last_indexed_slot().await,
                self.rpc.slot().await,
            )
            && let Err(err) = self
                .persistence
                .record_lost_range(from, Slot(tip), "backfill failed")
                .await
        {
            tracing::error!(?err, "failed to record the lost range");
        }
        result
    }

    async fn backfill_inner(&self) -> Result<(), PersistenceError> {
        let Some(watermark) = self.persistence.last_indexed_slot().await? else {
            return Ok(());
        };
        let tip = Slot(self.rpc.slot().await.map_err(PersistenceError::Rpc)?);
        if tip <= watermark {
            return Ok(());
        }

        // Newest-first pages per program, walked back to the watermark, then
        // reversed into execution order. Within one slot the node lists
        // signatures newest first, so the reversal restores intra-slot order
        // per program. A transaction touching both programs appears in both
        // scans, the slot buffer's idempotent writes absorb the duplicate.
        let mut programs = vec![self.settlement_program];
        programs.extend(self.solflow_program);
        let mut entries: Vec<(Slot, Signature)> = Vec::new();
        for program in programs {
            let mut before = None;
            'pages: loop {
                let page = self
                    .rpc
                    .signatures_for_address(&program, before)
                    .await
                    .map_err(PersistenceError::Rpc)?;
                let last_page = page.len() < cow_solana_rpc::SolanaRPC::SIGNATURES_PAGE;
                before = page.last().map(|(signature, _)| *signature);
                for (signature, slot) in page {
                    if Slot(slot) <= watermark {
                        break 'pages;
                    }
                    entries.push((Slot(slot), signature));
                }
                if last_page {
                    break;
                }
            }
        }
        entries.reverse();
        entries.sort_by_key(|(slot, _)| *slot);
        entries.dedup_by_key(|(_, signature)| *signature);

        let mut pending: BTreeMap<Slot, SlotBuffer> = BTreeMap::new();
        for (slot, signature) in entries {
            let encoded = self
                .rpc
                .transaction(&signature)
                .await
                .map_err(PersistenceError::Rpc)?;
            let buffer = pending.entry(slot).or_default();
            match convert(encoded, signature) {
                // The same decode the stream path runs: events buffer, a
                // failed decode dead-letters the whole transaction.
                Some(info) => match self.decode(info, slot, signature) {
                    Ok(events) => buffer.events.extend(events),
                    Err(super::DecodeFailed) => buffer.dead_letters.push(signature),
                },
                None => buffer.dead_letters.push(signature),
            }
        }
        for (slot, buffer) in pending {
            self.flush_slot(slot, buffer, true).await?;
        }
        // Slots past the last tracked transaction are quiet, the scan proved
        // them empty through the tip.
        self.persistence.write_last_indexed_slot(tip).await?;
        Ok(())
    }
}

/// Rebuild the stream's wire shape from an RPC-fetched transaction, mapping
/// exactly what the decoder reads: account keys with the ALT-loaded
/// addresses, top-level and inner instructions, and the error marker.
/// `None` when the payload cannot be decoded, which dead-letters the
/// transaction for another replay attempt.
pub(super) fn convert(
    encoded: EncodedConfirmedTransactionWithStatusMeta,
    signature: Signature,
) -> Option<SubscribeUpdateTransactionInfo> {
    let meta = encoded.transaction.meta?;
    let transaction = encoded.transaction.transaction.decode()?;
    let message = &transaction.message;

    let header = message.header();
    let instructions = message
        .instructions()
        .iter()
        .map(|instruction| CompiledInstruction {
            program_id_index: u32::from(instruction.program_id_index),
            accounts: instruction.accounts.clone(),
            data: instruction.data.clone(),
        })
        .collect();
    let inner_instructions = match &meta.inner_instructions {
        OptionSerializer::Some(groups) => groups
            .iter()
            .map(|group| {
                let instructions = group
                    .instructions
                    .iter()
                    .filter_map(|instruction| {
                        let UiInstruction::Compiled(compiled) = instruction else {
                            return None;
                        };
                        Some(InnerInstruction {
                            program_id_index: u32::from(compiled.program_id_index),
                            accounts: compiled.accounts.clone(),
                            data: bs58::decode(&compiled.data).into_vec().ok()?,
                            stack_height: compiled.stack_height,
                        })
                    })
                    .collect();
                InnerInstructions {
                    index: u32::from(group.index),
                    instructions,
                }
            })
            .collect(),
        _ => Vec::new(),
    };
    let decode_addresses = |addresses: &[String]| {
        addresses
            .iter()
            .filter_map(|address| bs58::decode(address).into_vec().ok())
            .collect::<Vec<_>>()
    };
    let (loaded_writable_addresses, loaded_readonly_addresses) = match &meta.loaded_addresses {
        OptionSerializer::Some(loaded) => (
            decode_addresses(&loaded.writable),
            decode_addresses(&loaded.readonly),
        ),
        _ => (Vec::new(), Vec::new()),
    };

    Some(SubscribeUpdateTransactionInfo {
        signature: signature.as_ref().to_vec(),
        is_vote: false,
        transaction: Some(Transaction {
            signatures: transaction
                .signatures
                .iter()
                .map(|signature| signature.as_ref().to_vec())
                .collect(),
            message: Some(Message {
                header: Some(MessageHeader {
                    num_required_signatures: u32::from(header.num_required_signatures),
                    num_readonly_signed_accounts: u32::from(header.num_readonly_signed_accounts),
                    num_readonly_unsigned_accounts: u32::from(
                        header.num_readonly_unsigned_accounts,
                    ),
                }),
                account_keys: message
                    .static_account_keys()
                    .iter()
                    .map(|key| key.to_bytes().to_vec())
                    .collect(),
                recent_blockhash: message.recent_blockhash().to_bytes().to_vec(),
                instructions,
                versioned: matches!(message, VersionedMessage::V0(_)),
                address_table_lookups: message
                    .address_table_lookups()
                    .unwrap_or_default()
                    .iter()
                    .map(|lookup| MessageAddressTableLookup {
                        account_key: lookup.account_key.to_bytes().to_vec(),
                        writable_indexes: lookup.writable_indexes.clone(),
                        readonly_indexes: lookup.readonly_indexes.clone(),
                    })
                    .collect(),
                ..Default::default()
            }),
        }),
        meta: Some(TransactionStatusMeta {
            // Only presence is read: a reverted transaction emits no events.
            err: meta
                .err
                .as_ref()
                .map(|_| TransactionError { err: Vec::new() }),
            inner_instructions,
            loaded_writable_addresses,
            loaded_readonly_addresses,
            ..Default::default()
        }),
        index: 0,
    })
}
