//! RPC backfill: recover the gap between the persisted watermark and the
//! live tip when it exceeds the stream's replay window. The node's deep
//! transaction history is scanned back to the watermark and every tracked
//! transaction runs through the same decode and flush path the stream uses.
//! The watermark ends at the scanned tip, back inside the replay window, so
//! the next subscription resumes without a hole.

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
    solana_sdk::{bs58, message::VersionedMessage, pubkey::Pubkey},
    solana_transaction_status_client_types::{UiInstruction, option_serializer::OptionSerializer},
    std::{collections::BTreeMap, time::Duration},
};

/// Attempts before the remaining gap is declared lost.
const BACKFILL_ATTEMPTS: usize = 3;

/// Pause between backfill attempts.
const BACKFILL_RETRY: Duration = Duration::from_secs(5);

impl Decoder {
    /// Index every tracked transaction between the persisted watermark and
    /// the live tip from RPC history, then advance the watermark to that
    /// tip. A missing watermark is a cold start with nothing to recover.
    /// A failure retries, then leaves the watermark wherever the last
    /// complete slot flush put it and records the rest of the gap as a lost
    /// range.
    pub(crate) async fn backfill(&self) -> Result<(), PersistenceError> {
        let Some(watermark) = self.persistence.last_indexed_slot().await? else {
            return Ok(());
        };
        let tip = Slot(self.rpc.slot().await.map_err(PersistenceError::Rpc)?);
        if tip <= watermark {
            return Ok(());
        }
        let mut result = Ok(());
        for attempt in 1..=BACKFILL_ATTEMPTS {
            result = self.backfill_inner(tip).await;
            let Err(err) = &result else {
                return Ok(());
            };
            tracing::warn!(?err, attempt, "backfill attempt failed");
            if attempt < BACKFILL_ATTEMPTS {
                tokio::time::sleep(BACKFILL_RETRY).await;
            }
        }
        // Complete slots flushed before the failure moved the watermark, so
        // only the rest of the gap is lost. The fallback bounds guarantee a
        // recorded row even when the fresh reads fail too.
        let from = self
            .persistence
            .last_indexed_slot()
            .await
            .ok()
            .flatten()
            .unwrap_or(watermark);
        let through = self.rpc.slot().await.map_or(tip, Slot);
        if let Err(err) = self
            .persistence
            .record_lost_range(from, through, "backfill failed")
            .await
        {
            tracing::error!(?err, "failed to record the lost range");
        }
        result
    }

    async fn backfill_inner(&self, tip: Slot) -> Result<(), PersistenceError> {
        let Some(watermark) = self.persistence.last_indexed_slot().await? else {
            return Ok(());
        };
        if tip <= watermark {
            return Ok(());
        }

        // Signatures grouped by slot, ascending. Within one slot the node
        // lists signatures newest first, so the reversed walk restores
        // execution order per program. A transaction touching both programs
        // shows up in both scans, the per-slot list keeps one copy.
        let mut programs = vec![self.settlement_program];
        programs.extend(self.solflow_program);
        let mut slots: BTreeMap<Slot, Vec<Signature>> = BTreeMap::new();
        for program in programs {
            for (slot, signature) in self
                .signatures_since(&program, watermark)
                .await?
                .into_iter()
                .rev()
            {
                let signatures = slots.entry(slot).or_default();
                if !signatures.contains(&signature) {
                    signatures.push(signature);
                }
            }
        }

        for (slot, signatures) in slots {
            let mut buffer = SlotBuffer::default();
            for signature in signatures {
                let encoded = self.rpc.transaction(&signature).await.map_err(|err| {
                    tracing::error!(%signature, "failed to fetch a transaction");
                    PersistenceError::Rpc(err)
                })?;
                // The same decode the stream path runs: events buffer, a
                // failed decode dead-letters the whole transaction.
                match convert(encoded, signature) {
                    Some(info) => match self.decode(info, slot, signature) {
                        Ok(events) => buffer.events.extend(events),
                        Err(super::DecodeFailed) => buffer.dead_letters.push(signature),
                    },
                    None => {
                        tracing::warn!(%signature, "undecodable payload, dead-lettered");
                        buffer.dead_letters.push(signature);
                    }
                }
            }
            self.flush_slot(slot, buffer, true).await?;
        }
        // Slots past the last tracked transaction are quiet, the scan proved
        // them empty through the tip.
        self.persistence.write_last_indexed_slot(tip).await?;
        Ok(())
    }

    /// The program's (slot, signature) history above the watermark, newest
    /// first, paged from the node until a page dips to the watermark or the
    /// history runs out.
    async fn signatures_since(
        &self,
        program: &Pubkey,
        watermark: Slot,
    ) -> Result<Vec<(Slot, Signature)>, PersistenceError> {
        let mut entries: Vec<(Slot, Signature)> = Vec::new();
        let mut before = None;
        loop {
            let (page, more) = self
                .rpc
                .signatures_for_address(program, before)
                .await
                .map_err(PersistenceError::Rpc)?;
            // A page with no parsable signature cannot advance the cursor,
            // so fail the scan rather than loop on the same page.
            if page.is_empty() && more {
                tracing::error!(%program, "signature page with no parsable entry");
                return Err(PersistenceError::Unavailable);
            }
            let reached_watermark = page.iter().any(|(_, slot)| Slot(*slot) <= watermark);
            before = page.last().map(|(signature, _)| *signature);
            entries.extend(
                page.into_iter()
                    .take_while(|(_, slot)| Slot(*slot) > watermark)
                    .map(|(signature, slot)| (Slot(slot), signature)),
            );
            if reached_watermark || !more {
                return Ok(entries);
            }
        }
    }
}

/// Rebuild the stream's wire shape from an RPC-fetched transaction, mapping
/// exactly what the decoder reads: account keys with the ALT-loaded
/// addresses, top-level and inner instructions, and the error marker.
/// `None` when the payload cannot be decoded.
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
            // The decoder only checks whether an error is present (a
            // reverted transaction emits no events), so the RPC error maps
            // to an empty marker rather than re-encoded bytes.
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
