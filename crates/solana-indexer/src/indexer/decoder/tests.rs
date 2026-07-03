use {
    super::{build_account_keys, relevant_instructions},
    crate::types::wire::{
        CompiledInstruction,
        InnerInstruction,
        InnerInstructions,
        Message,
        SubscribeUpdateTransactionInfo,
        Transaction,
        TransactionStatusMeta,
    },
    solana_sdk::pubkey::Pubkey,
};

fn pubkey(n: u8) -> Pubkey {
    Pubkey::new_from_array([n; 32])
}

fn key_bytes(key: Pubkey) -> Vec<u8> {
    key.to_bytes().to_vec()
}

fn compiled(program_id_index: u32, accounts: Vec<u8>, data: Vec<u8>) -> CompiledInstruction {
    CompiledInstruction {
        program_id_index,
        accounts,
        data,
    }
}

fn inner(program_id_index: u32, accounts: Vec<u8>, data: Vec<u8>) -> InnerInstruction {
    InnerInstruction {
        program_id_index,
        accounts,
        data,
        stack_height: None,
    }
}

/// Build a transaction-update fixture from resolved pubkeys: the static account
/// keys, the ALT-loaded writable and readonly addresses, the top-level
/// instructions, and the inner-instruction groups.
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

/// One realistic transaction that exercises the four things that can actually
/// break: the account list is `static ++ ALT-writable ++ ALT-readonly` (so the
/// ALT indices only resolve if the regions are concatenated in that order),
/// settlement is reachable only as a CPI (so inner instructions must be
/// walked), an untracked program is present (so the filter must drop it), and
/// each kept instruction records its position.
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
            compiled(0, vec![1], vec![0]),
            compiled(3, vec![1, 4], vec![1, 2, 3]),
        ],
        // settlement invoked as a CPI under top-level instruction 0
        vec![InnerInstructions {
            index: 0,
            instructions: vec![inner(2, vec![1], vec![7])],
        }],
    );

    let relevant = relevant_instructions(&tx, &settlement, &solflow);

    // router dropped; solflow (top-level) and settlement (CPI) kept, in that order.
    assert_eq!(relevant.len(), 2);

    assert_eq!(relevant[0].program, solflow);
    assert_eq!(relevant[0].instruction_index, 1);
    assert_eq!(relevant[0].inner_index, None);
    assert_eq!(relevant[0].accounts, vec![acct_a, acct_b]);
    assert_eq!(relevant[0].data, vec![1, 2, 3]);

    assert_eq!(relevant[1].program, settlement);
    assert_eq!(relevant[1].instruction_index, 0);
    assert_eq!(relevant[1].inner_index, Some(0));
    assert_eq!(relevant[1].accounts, vec![acct_a]);
    assert_eq!(relevant[1].data, vec![7]);
}

/// Malformed stream data must drop rather than panic: an out-of-range account
/// index on a program-matched instruction, an out-of-range program index, and a
/// wrong-length key that becomes the zero pubkey and matches nothing.
#[test]
fn malformed_instructions_are_dropped_without_panicking() {
    let (settlement, solflow) = (pubkey(1), pubkey(2));
    // Account list: [settlement(0), <5-byte key -> zero pubkey>(1)].
    let tx = SubscribeUpdateTransactionInfo {
        transaction: Some(Transaction {
            message: Some(Message {
                account_keys: vec![key_bytes(settlement), vec![1, 2, 3, 4, 5]],
                instructions: vec![
                    // settlement matches, but account index 5 is out of range
                    compiled(0, vec![5], vec![0]),
                    // program index 9 is out of range
                    compiled(9, vec![0], vec![0]),
                    // program index 1 is the zeroed bad key, matching nothing
                    compiled(1, vec![0], vec![0]),
                ],
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };

    assert_eq!(build_account_keys(&tx), vec![settlement, Pubkey::default()]);
    assert!(relevant_instructions(&tx, &settlement, &solflow).is_empty());
}
