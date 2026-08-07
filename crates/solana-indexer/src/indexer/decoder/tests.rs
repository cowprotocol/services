use {
    super::{
        Decoder,
        PartialDecode,
        ResolvedOrder,
        build_account_keys,
        decode_settlement,
        relevant_instructions,
    },
    crate::{
        persistence::Persistence,
        types::{
            Signature,
            channel::StreamUpdate,
            events::{DecodedEvent, SettlementEvent, TradeDelta},
            order::OrderUid,
            slot::Slot,
            tx::TxContext,
            wire::{
                CompiledInstruction,
                InnerInstruction,
                InnerInstructions,
                Message,
                SubscribeUpdateTransactionInfo,
                Transaction,
                TransactionError,
                TransactionStatusMeta,
            },
        },
    },
    bytes::Bytes,
    settlement_interface::{
        Pubkey as InterfacePubkey,
        SettlementInstruction,
        data::intent::{OrderIntent, OrderKind},
        pda::order::find_order_pda,
    },
    solana_sdk::pubkey::Pubkey,
    tokio::sync::mpsc::Sender,
};

fn pubkey(n: u8) -> Pubkey {
    Pubkey::new_from_array([n; 32])
}

/// Compile client-built instructions into the proto transaction shape: the
/// account list starts with the fee payer, then every referenced key in
/// encounter order.
fn tx_from_instructions(
    payer: Pubkey,
    instructions: &[settlement_interface::Instruction],
) -> SubscribeUpdateTransactionInfo {
    let mut keys = vec![payer];
    let index_of = |keys: &mut Vec<Pubkey>, key: Pubkey| -> u8 {
        if let Some(index) = keys.iter().position(|k| *k == key) {
            return u8::try_from(index).unwrap();
        }
        keys.push(key);
        u8::try_from(keys.len() - 1).unwrap()
    };
    let compiled = instructions
        .iter()
        .map(|instruction| CompiledInstruction {
            program_id_index: u32::from(index_of(&mut keys, instruction.program_id)),
            accounts: instruction
                .accounts
                .iter()
                .map(|meta| index_of(&mut keys, meta.pubkey))
                .collect(),
            data: instruction.data.clone(),
        })
        .collect();
    tx_info(keys, vec![], vec![], compiled, vec![])
}

fn key_bytes(key: Pubkey) -> Vec<u8> {
    key.to_bytes().to_vec()
}

fn inner(
    program_id_index: u32,
    accounts: Vec<u8>,
    data: Vec<u8>,
    stack_height: Option<u32>,
) -> InnerInstruction {
    InnerInstruction {
        program_id_index,
        accounts,
        data,
        stack_height,
    }
}

/// Build a transaction-update fixture: static account keys, ALT-loaded writable
/// and readonly addresses, top-level instructions, and inner-instruction
/// groups.
fn tx_info(
    account_keys: Vec<Pubkey>,
    loaded_writable: Vec<Pubkey>,
    loaded_readonly: Vec<Pubkey>,
    instructions: Vec<CompiledInstruction>,
    inner_instructions: Vec<InnerInstructions>,
) -> SubscribeUpdateTransactionInfo {
    SubscribeUpdateTransactionInfo {
        transaction: Some(Transaction {
            message: Some(Message {
                account_keys: account_keys.into_iter().map(key_bytes).collect(),
                instructions,
                ..Default::default()
            }),
            ..Default::default()
        }),
        meta: Some(TransactionStatusMeta {
            inner_instructions,
            loaded_writable_addresses: loaded_writable.into_iter().map(key_bytes).collect(),
            loaded_readonly_addresses: loaded_readonly.into_iter().map(key_bytes).collect(),
            ..Default::default()
        }),
        ..Default::default()
    }
}

/// One realistic transaction: settlement reached only via a CPI, an untracked
/// program that must be dropped, and ALT-loaded programs so the account-list
/// order is exercised.
#[test]
fn resolves_settlement_and_solflow_across_top_level_and_cpi() {
    let (settlement, solflow, router) = (pubkey(1), pubkey(2), pubkey(9));
    let (acct_a, acct_b) = (pubkey(3), pubkey(4));
    // Full list: [router(0), acct_a(1)] ++ [settlement(2)] ++ [solflow(3),
    // acct_b(4)]
    let tx = tx_info(
        vec![router, acct_a],
        vec![settlement],
        vec![solflow, acct_b],
        // top-level: a router call (dropped) then a solflow call (kept, index 1)
        vec![
            CompiledInstruction {
                program_id_index: 0,
                accounts: vec![1],
                data: vec![0],
            },
            CompiledInstruction {
                program_id_index: 3,
                accounts: vec![1, 4],
                data: vec![1, 2, 3],
            },
        ],
        // settlement invoked as a CPI under top-level instruction 0
        vec![InnerInstructions {
            index: 0,
            instructions: vec![inner(2, vec![1], vec![7], None)],
        }],
    );

    // The ALT indices (2, 3) only resolve if the three regions are concatenated
    // static, then writable, then readonly.
    assert_eq!(
        build_account_keys(&tx),
        vec![router, acct_a, settlement, solflow, acct_b]
    );

    let relevant = relevant_instructions(&tx, &settlement, &solflow);

    // Execution order: top-level 0's settlement CPI runs before top-level 1's
    // solflow call. The router at top-level 0 is dropped.
    assert_eq!(relevant.len(), 2);

    assert_eq!(relevant[0].program_id, settlement);
    assert_eq!(relevant[0].instruction_index, 0);
    assert_eq!(relevant[0].inner_ix_path, vec![0]);
    assert_eq!(relevant[0].accounts, vec![1]);
    assert_eq!(relevant[0].data, Bytes::from(vec![7]));

    assert_eq!(relevant[1].program_id, solflow);
    assert_eq!(relevant[1].instruction_index, 1);
    assert!(relevant[1].inner_ix_path.is_empty());
    assert_eq!(relevant[1].accounts, vec![1, 4]);
    assert_eq!(relevant[1].data, Bytes::from(vec![1, 2, 3]));
}

/// A program index that does not resolve to a tracked program is dropped
/// (out of range, or a wrong-length key that becomes the zero pubkey). Account
/// indices are carried through unresolved, so a bad one does not drop the
/// instruction here.
#[test]
fn unresolvable_programs_dropped_account_indices_carried_through() {
    let (settlement, solflow) = (pubkey(1), pubkey(2));
    // Account list: [settlement(0), <5-byte key -> zero pubkey>(1)].
    let tx = SubscribeUpdateTransactionInfo {
        transaction: Some(Transaction {
            message: Some(Message {
                account_keys: vec![key_bytes(settlement), vec![1, 2, 3, 4, 5]],
                instructions: vec![
                    // program index 9 is out of range -> dropped
                    CompiledInstruction {
                        program_id_index: 9,
                        accounts: vec![0],
                        data: vec![0],
                    },
                    // program index 1 is the zeroed bad key -> untracked, dropped
                    CompiledInstruction {
                        program_id_index: 1,
                        accounts: vec![0],
                        data: vec![0],
                    },
                    // settlement, with an out-of-range account index carried as-is
                    CompiledInstruction {
                        program_id_index: 0,
                        accounts: vec![5],
                        data: vec![7],
                    },
                ],
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };

    assert_eq!(build_account_keys(&tx), vec![settlement, Pubkey::default()]);

    let relevant = relevant_instructions(&tx, &settlement, &solflow);
    assert_eq!(relevant.len(), 1);
    assert_eq!(relevant[0].program_id, settlement);
    assert_eq!(relevant[0].instruction_index, 2);
    assert_eq!(relevant[0].accounts, vec![5]);
}

/// CPIs nest deeper than one level. `stack_height` drives the per-level path,
/// and a dropped (untracked) inner still advances the sibling counter, so kept
/// siblings keep their true position.
#[test]
fn inner_ix_path_tracks_cpi_nesting_depth() {
    let (settlement, solflow, router, other) = (pubkey(1), pubkey(2), pubkey(9), pubkey(8));
    // static account list: [router(0), settlement(1), other(2), solflow(3),
    // acct(4)]
    let tx = tx_info(
        vec![router, settlement, other, solflow, pubkey(4)],
        vec![],
        vec![],
        // one top-level router call (dropped)
        vec![CompiledInstruction {
            program_id_index: 0,
            accounts: vec![4],
            data: vec![0],
        }],
        vec![InnerInstructions {
            index: 0,
            instructions: vec![
                inner(1, vec![4], vec![10], Some(2)), // settlement, depth 1 -> [0]     kept
                inner(2, vec![4], vec![11], Some(3)), // other,      depth 2 -> [0, 0]  dropped
                inner(1, vec![4], vec![12], Some(3)), // settlement, depth 2 -> [0, 1]  kept
                inner(3, vec![4], vec![13], Some(2)), // solflow,    depth 1 -> [1]     kept
            ],
        }],
    );

    let relevant = relevant_instructions(&tx, &settlement, &solflow);
    assert_eq!(relevant.len(), 3);

    assert_eq!(relevant[0].program_id, settlement);
    assert_eq!(relevant[0].inner_ix_path, vec![0]);
    assert_eq!(relevant[0].data, Bytes::from(vec![10]));

    // the dropped depth-2 CPI still advanced the counter, so this sibling is [0, 1]
    assert_eq!(relevant[1].program_id, settlement);
    assert_eq!(relevant[1].inner_ix_path, vec![0, 1]);
    assert_eq!(relevant[1].data, Bytes::from(vec![12]));

    // back to depth 1: the second direct CPI under the top-level
    assert_eq!(relevant[2].program_id, solflow);
    assert_eq!(relevant[2].inner_ix_path, vec![1]);
    assert_eq!(relevant[2].data, Bytes::from(vec![13]));
}

/// A corrupt `stack_height` from the stream must not drive an unbounded path
/// allocation: depth is clamped to `MAX_CPI_DEPTH`.
#[test]
fn corrupt_stack_height_is_clamped() {
    let (settlement, solflow) = (pubkey(1), pubkey(2));
    let tx = tx_info(
        vec![pubkey(9), settlement], // [router(0), settlement(1)]
        vec![],
        vec![],
        vec![CompiledInstruction {
            program_id_index: 0,
            accounts: vec![1],
            data: vec![0],
        }], // top-level router, dropped
        vec![InnerInstructions {
            index: 0,
            instructions: vec![inner(1, vec![1], vec![7], Some(10_000))],
        }],
    );

    let relevant = relevant_instructions(&tx, &settlement, &solflow);
    assert_eq!(relevant.len(), 1);
    assert_eq!(relevant[0].program_id, settlement);
    // depth 9999 clamped to 4, so the path is bounded, not 9999 elements
    assert_eq!(relevant[0].inner_ix_path, vec![0, 0, 0, 0]);
}

fn signature(n: u8) -> Signature {
    Signature::from([n; 64])
}

fn test_decoder(settlement: Pubkey, solflow: Pubkey) -> (Decoder, Sender<StreamUpdate>) {
    let (sender, rx) = tokio::sync::mpsc::channel(16);
    let decoder = Decoder::new(Persistence {}, rx, settlement, solflow);
    (decoder, sender)
}

/// A transaction carrying one settlement instruction, so draining it also
/// routes into `decode_settlement`.
fn stream_tx(slot: Slot, signature: Signature, settlement: Pubkey) -> StreamUpdate {
    let info = tx_info(
        vec![settlement, pubkey(8)],
        vec![],
        vec![],
        vec![CompiledInstruction {
            program_id_index: 0,
            accounts: vec![1],
            data: vec![0],
        }],
        vec![],
    );
    StreamUpdate::Tx {
        slot,
        signature,
        inner: Box::new(info),
    }
}

/// Verifies the run loop drains buffered updates and returns Ok when the
/// sender drops. Event content is not asserted here: the persistence bodies
/// are no-ops, so nothing is observable through them. Decode output is
/// asserted directly in
/// `decode_wraps_settlement_events_as_decoded`.
#[tokio::test]
async fn run_drains_transactions_until_the_sender_drops() {
    let (settlement, solflow) = (pubkey(1), pubkey(2));
    let (mut decoder, sender) = test_decoder(settlement, solflow);

    sender
        .send(stream_tx(Slot(7), signature(3), settlement))
        .await
        .unwrap();
    drop(sender);

    assert!(decoder.run().await.is_ok());
}

/// Build a `CreateOrder` transaction fixture with the client crate. Returns
/// the tx, the expected order UID (the hash of the encoded intent), and the
/// `created_by` account. The account-list owner (`pubkey(11)`) differs from
/// the intent owner (`[0x11; 32]`) so callers can pin that the event owner
/// comes from the intent data, not the accounts.
fn create_order_tx() -> (SubscribeUpdateTransactionInfo, OrderUid, Pubkey) {
    let settlement = pubkey(1);
    let created_by = pubkey(12);
    let intent = OrderIntent {
        owner: InterfacePubkey::new_from_array([0x11; 32]),
        buy_token_account: InterfacePubkey::new_from_array([0x22; 32]),
        sell_token_account: InterfacePubkey::new_from_array([0x33; 32]),
        sell_amount: 1_000,
        buy_amount: 2_000,
        valid_to: 42,
        kind: OrderKind::Sell,
        partially_fillable: false,
        app_data: [0x44; 32],
    };
    let instruction = settlement_client::instructions::CreateOrder {
        program_id: settlement,
        owner: pubkey(11),
        created_by,
        intent: &intent,
    }
    .into();
    let tx = tx_from_instructions(pubkey(9), &[instruction]);
    (tx, OrderUid(intent.uid().to_bytes()), created_by)
}

/// `decode` wraps settlement events as `DecodedEvent::Settlement` for `run`
/// to persist, and gates on the transaction meta: a set `meta.err` means a
/// revert that rolled back every account write, so nothing is emitted and
/// nothing is dead-lettered, while an absent meta carries no success flag at
/// all, so nothing is emitted and the transaction is dead-lettered. One
/// fixture throughout, the meta is the only difference.
#[test]
fn decode_wraps_events_and_gates_on_transaction_meta() {
    let (settlement, solflow) = (pubkey(1), pubkey(2));
    let (mut tx, expected_uid, created_by) = create_order_tx();
    let (decoder, _sender) = test_decoder(settlement, solflow);

    let (events, decode_failed) = decoder.decode(tx.clone(), Slot(5), signature(6));
    assert!(!decode_failed);
    assert_eq!(
        events,
        vec![DecodedEvent::Settlement(SettlementEvent::OrderCreated {
            order_uid: expected_uid,
            owner: Pubkey::new_from_array([0x11; 32]),
            created_by,
        })]
    );

    tx.meta.as_mut().unwrap().err = Some(TransactionError { err: vec![1] });
    let (events, decode_failed) = decoder.decode(tx.clone(), Slot(5), signature(6));
    assert!(!decode_failed);
    assert_eq!(events, vec![]);

    tx.meta = None;
    let (events, decode_failed) = decoder.decode(tx.clone(), Slot(5), signature(6));
    assert!(decode_failed);
    assert_eq!(events, vec![]);
}

/// A settlement instruction with an unknown discriminator sets the failure
/// flag and yields no event, while an instruction that decodes cleanly in the
/// same transaction still emits its event.
#[test]
fn unknown_discriminator_sets_failure_flag_and_keeps_good_events() {
    let (settlement, solflow) = (pubkey(1), pubkey(2));
    let (mut tx, expected_uid, created_by) = create_order_tx();
    // Prepend a settlement instruction whose discriminator byte matches no
    // known instruction.
    let message = tx.transaction.as_mut().unwrap().message.as_mut().unwrap();
    let settlement_index = message
        .account_keys
        .iter()
        .position(|key| key.as_slice() == settlement.to_bytes())
        .unwrap();
    message.instructions.insert(
        0,
        CompiledInstruction {
            program_id_index: u32::try_from(settlement_index).unwrap(),
            accounts: vec![1],
            data: vec![0xFF],
        },
    );

    let ctx = TxContext {
        slot: Slot(5),
        signature: signature(6),
        account_keys: build_account_keys(&tx),
        post_token_balances: vec![],
    };
    let instructions = relevant_instructions(&tx, &settlement, &solflow);
    let result = decode_settlement(&instructions, &ctx, |_| None);

    let Err(PartialDecode { events }) = result else {
        panic!("expected a partial decode, got {result:?}");
    };
    assert_eq!(
        events,
        vec![SettlementEvent::OrderCreated {
            order_uid: expected_uid,
            owner: Pubkey::new_from_array([0x11; 32]),
            created_by,
        }]
    );
}

/// A `BeginSettle` whose named `FinalizeSettle` is not present in the
/// transaction emits no event and sets the failure flag.
#[test]
fn unpaired_begin_settle_sets_failure_flag() {
    let (settlement, solflow) = (pubkey(1), pubkey(2));
    // Account list: [solver(0), settlement(1), sysvar(2), state(3), token(4)].
    let account_keys = vec![pubkey(10), settlement, pubkey(22), pubkey(23), pubkey(24)];

    // BeginSettle body with zero orders, naming finalize index 1 while the tx
    // has no instruction 1.
    let mut begin_data = vec![SettlementInstruction::BeginSettle.discriminator()];
    begin_data.extend_from_slice(&1u16.to_le_bytes());
    begin_data.extend_from_slice(&4242i64.to_le_bytes());
    begin_data.push(0);

    let tx = tx_info(
        account_keys,
        vec![],
        vec![],
        vec![CompiledInstruction {
            program_id_index: 1,
            accounts: vec![2, 3, 4],
            data: begin_data,
        }],
        vec![],
    );

    let ctx = TxContext {
        slot: Slot(5),
        signature: signature(6),
        account_keys: build_account_keys(&tx),
        post_token_balances: vec![],
    };
    let instructions = relevant_instructions(&tx, &settlement, &solflow);
    let result = decode_settlement(&instructions, &ctx, |_| None);

    assert_eq!(result, Err(PartialDecode { events: vec![] }));
}

/// A `BeginSettle` + `FinalizeSettle` pair built by the client crate decodes
/// to one `SettlementFinalized`, where:
///
/// - the auction id comes from the `BeginSettle` instruction data,
/// - the order's sell amount is the sum of its pulls (300 + 700, both taken
///   from the same order's sell account),
/// - the buy-side amount comes from the `FinalizeSettle` entry paired to its
///   order by position (order `i` is paid by entry `i`),
/// - the order UID comes from the injected resolver, keyed by the canonical
///   order PDA the builder derives,
/// - the solver is the fee payer.
#[test]
fn begin_and_finalize_settle_decode_to_settlement_finalized() {
    let (settlement, solflow) = (pubkey(1), pubkey(2));
    let solver = pubkey(10);
    let intent = OrderIntent {
        owner: InterfacePubkey::new_from_array([0x11; 32]),
        buy_token_account: InterfacePubkey::new_from_array([0x22; 32]),
        sell_token_account: InterfacePubkey::new_from_array([0x33; 32]),
        sell_amount: 1_000,
        buy_amount: 1_234,
        valid_to: 42,
        kind: OrderKind::Sell,
        partially_fillable: false,
        app_data: [0x44; 32],
    };
    let order_pda = find_order_pda(&settlement, &intent.uid()).0;

    let begin = settlement_client::instructions::BeginSettle {
        program_id: settlement,
        finalize_ix_index: 1,
        auction_id: 4242,
        orders: &[settlement_client::instructions::InitializedIntent {
            intent: &intent,
            pulls: &[
                settlement_client::instructions::Pull {
                    destination: pubkey(27),
                    amount: 300,
                },
                settlement_client::instructions::Pull {
                    destination: pubkey(28),
                    amount: 700,
                },
            ],
        }],
    }
    .into();
    let finalize = settlement_client::instructions::FinalizeSettle {
        program_id: settlement,
        begin_ix_index: 0,
        orders: &[settlement_client::instructions::FinalizedIntent {
            intent: &intent,
            mint: pubkey(30),
            amount: 1_234,
        }],
    }
    .into();
    let tx = tx_from_instructions(solver, &[begin, finalize]);

    let expected_uid = OrderUid([0x55; 32]);
    let resolve_order = |pda: &Pubkey| {
        (*pda == order_pda).then_some(ResolvedOrder {
            order_uid: expected_uid,
            order_fulfilled: true,
        })
    };

    let ctx = TxContext {
        slot: Slot(5),
        signature: signature(6),
        account_keys: build_account_keys(&tx),
        post_token_balances: vec![],
    };
    let instructions = relevant_instructions(&tx, &settlement, &solflow);
    let events = decode_settlement(&instructions, &ctx, resolve_order).expect("clean decode");

    assert_eq!(
        events,
        vec![SettlementEvent::SettlementFinalized {
            auction_id: 4242,
            solver,
            tx_signature: signature(6),
            slot: Slot(5),
            trades: vec![TradeDelta {
                order_uid: expected_uid,
                amount_withdrawn_delta: 1_000,
                amount_received_delta: 1_234,
                order_fulfilled: true,
            }],
        }]
    );
}
