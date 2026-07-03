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

#[test]
fn top_level_settlement_instruction_is_kept() {
    let (settlement, solflow) = (pubkey(1), pubkey(2));
    let (acct_a, acct_b) = (pubkey(3), pubkey(4));
    // account list: [settlement(0), acct_a(1), acct_b(2)]
    let tx = tx_info(
        vec![settlement, acct_a, acct_b],
        vec![],
        vec![],
        vec![compiled(0, vec![1, 2], vec![9, 9, 9])],
        vec![],
    );

    let relevant = relevant_instructions(&tx, &settlement, &solflow);

    assert_eq!(relevant.len(), 1);
    assert_eq!(relevant[0].program, settlement);
    assert_eq!(relevant[0].instruction_index, 0);
    assert_eq!(relevant[0].inner_index, None);
    assert_eq!(relevant[0].accounts, vec![acct_a, acct_b]);
    assert_eq!(relevant[0].data, vec![9, 9, 9]);
}

#[test]
fn settlement_cpi_in_inner_instructions_is_kept() {
    let (settlement, solflow) = (pubkey(1), pubkey(2));
    let (other_program, acct) = (pubkey(5), pubkey(3));
    // account list: [other_program(0), settlement(1), acct(2)]. The settlement
    // call is a CPI, so it only appears in the inner instructions.
    let tx = tx_info(
        vec![other_program, settlement, acct],
        vec![],
        vec![],
        vec![compiled(0, vec![], vec![0])],
        vec![InnerInstructions {
            index: 0,
            instructions: vec![inner(1, vec![2], vec![7])],
        }],
    );

    let relevant = relevant_instructions(&tx, &settlement, &solflow);

    assert_eq!(relevant.len(), 1);
    assert_eq!(relevant[0].program, settlement);
    assert_eq!(relevant[0].instruction_index, 0);
    assert_eq!(relevant[0].inner_index, Some(0));
    assert_eq!(relevant[0].accounts, vec![acct]);
    assert_eq!(relevant[0].data, vec![7]);
}

#[test]
fn alt_loaded_program_is_resolved() {
    let (settlement, solflow) = (pubkey(1), pubkey(2));
    let (acct, readonly) = (pubkey(3), pubkey(6));
    // static [acct], ALT writable [settlement], ALT readonly [readonly], so the
    // full list is [acct(0), settlement(1), readonly(2)] and program_id_index 1
    // resolves into the ALT-loaded region.
    let tx = tx_info(
        vec![acct],
        vec![settlement],
        vec![readonly],
        vec![compiled(1, vec![0], vec![5])],
        vec![],
    );

    assert_eq!(build_account_keys(&tx), vec![acct, settlement, readonly]);

    let relevant = relevant_instructions(&tx, &settlement, &solflow);

    assert_eq!(relevant.len(), 1);
    assert_eq!(relevant[0].program, settlement);
    assert_eq!(relevant[0].instruction_index, 0);
    assert_eq!(relevant[0].inner_index, None);
    assert_eq!(relevant[0].accounts, vec![acct]);
}

#[test]
fn solflow_program_instruction_is_kept() {
    let (settlement, solflow) = (pubkey(1), pubkey(2));
    let (other_program, acct) = (pubkey(5), pubkey(3));
    // account list: [other_program(0), solflow(1), acct(2)]. Two top-level
    // instructions; only the second (index 1) targets a tracked program.
    let tx = tx_info(
        vec![other_program, solflow, acct],
        vec![],
        vec![],
        vec![
            compiled(0, vec![2], vec![0]),
            compiled(1, vec![2], vec![1, 2, 3]),
        ],
        vec![],
    );

    let relevant = relevant_instructions(&tx, &settlement, &solflow);

    assert_eq!(relevant.len(), 1);
    assert_eq!(relevant[0].program, solflow);
    assert_eq!(relevant[0].instruction_index, 1);
    assert_eq!(relevant[0].inner_index, None);
    assert_eq!(relevant[0].accounts, vec![acct]);
    assert_eq!(relevant[0].data, vec![1, 2, 3]);
}

#[test]
fn instruction_with_out_of_range_program_index_is_dropped() {
    let (settlement, solflow) = (pubkey(1), pubkey(2));
    // account list has one entry (index 0); program_id_index 5 is out of range.
    let tx = tx_info(
        vec![settlement],
        vec![],
        vec![],
        vec![compiled(5, vec![0], vec![0])],
        vec![],
    );

    assert!(relevant_instructions(&tx, &settlement, &solflow).is_empty());
}

#[test]
fn relevant_instruction_with_out_of_range_account_index_is_dropped() {
    let (settlement, solflow) = (pubkey(1), pubkey(2));
    // program resolves to settlement (index 0), but account index 9 is out of
    // range, so the whole instruction is dropped.
    let tx = tx_info(
        vec![settlement],
        vec![],
        vec![],
        vec![compiled(0, vec![9], vec![0])],
        vec![],
    );

    assert!(relevant_instructions(&tx, &settlement, &solflow).is_empty());
}

#[test]
fn wrong_length_account_key_is_zeroed_and_not_matched() {
    let (settlement, solflow) = (pubkey(1), pubkey(2));
    // A 5-byte static key at index 0 becomes the zero pubkey, keeping index
    // alignment. An instruction naming it as its program matches no tracked
    // program and is dropped.
    let tx = SubscribeUpdateTransactionInfo {
        transaction: Some(Transaction {
            message: Some(Message {
                account_keys: vec![vec![1, 2, 3, 4, 5]],
                instructions: vec![compiled(0, vec![], vec![0])],
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };

    assert_eq!(build_account_keys(&tx), vec![Pubkey::default()]);
    assert!(relevant_instructions(&tx, &settlement, &solflow).is_empty());
}
