//! Decodes base64 wire transactions, such as the `bundle` field of a `failed
//! quote bundle` trace, into each instruction's program, kind, accounts and
//! amounts.
//!
//! To capture bundles, add `solana_orderbook::infra::verifier=trace` to the
//! orderbook's log filter through its log filter socket (see "Changing Log
//! Filters" in the README), then pass the `bundle` field of a `failed quote
//! bundle` log:
//!
//! ```sh
//! cargo run -p solana-orderbook --example decode_bundle -- '["AQ...", "AQ..."]'
//! ```

use {
    anyhow::{Context, Result},
    base64::{Engine, prelude::BASE64_STANDARD},
    clap::Parser,
    cow_settlement_interface::{
        SettlementInstruction,
        data::intent::OrderIntent,
        instruction::{
            InstructionInputParsing,
            create_order::CreateOrderInput,
            settle::{BeginSettleInput, FinalizeSettleInput},
        },
        recover_discriminator,
    },
    cow_solana_rpc::{CommitmentConfig, SolanaRPC},
    solana_sdk::{message::VersionedMessage, pubkey::Pubkey, transaction::VersionedTransaction},
    solana_system_interface::instruction::SystemInstruction,
    spl_token_2022_interface::instruction::TokenInstruction,
    std::time::Duration,
    url::Url,
};

/// Bytes before the first address of an address lookup table account.
const LOOKUP_TABLE_META_SIZE: usize = 56;

/// Accounts listed per instruction, the rest are only counted.
const SHOWN_ACCOUNTS: usize = 8;

#[derive(Parser)]
struct Args {
    /// Base64 wire transactions, or JSON arrays of them.
    #[arg(required = true)]
    transactions: Vec<String>,

    /// Resolves the lookup tables of v0 messages.
    #[arg(long, env = "SOLANA_RPC_URL")]
    rpc_url: Option<Url>,

    /// Settlement program the transactions call. Staging deploys its own.
    #[arg(long, env, default_value_t = cow_settlement_interface::ID)]
    settlement_program: Pubkey,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let rpc = args.rpc_url.map(|url| {
        SolanaRPC::new_with_timeout_and_commitment(
            &url,
            Duration::from_secs(10),
            CommitmentConfig::confirmed(),
            "decode_bundle",
        )
    });
    let mut transactions = Vec::new();
    for arg in args.transactions {
        if arg.trim_start().starts_with('[') {
            transactions.extend(serde_json::from_str::<Vec<String>>(&arg)?);
        } else {
            transactions.push(arg);
        }
    }
    for (n, transaction) in transactions.iter().enumerate() {
        let bytes = BASE64_STANDARD.decode(transaction.trim())?;
        // Landed settlements are v1 messages, which bincode can't read.
        let transaction: VersionedTransaction = wincode::deserialize(&bytes)?;
        let keys = account_keys(&transaction.message, rpc.as_ref()).await?;
        let version = match &transaction.message {
            VersionedMessage::Legacy(_) => "legacy",
            VersionedMessage::V0(_) => "v0",
            VersionedMessage::V1(_) => "v1",
        };
        println!("transaction {n}: {version}, fee payer {}", keys[0]);
        if let VersionedMessage::V1(message) = &transaction.message {
            println!("  config {:?}", message.config);
        }
        for (index, instruction) in transaction.message.instructions().iter().enumerate() {
            let program = keys[usize::from(instruction.program_id_index)];
            let accounts = instruction
                .accounts
                .iter()
                .map(|&account| keys[usize::from(account)])
                .collect::<Vec<_>>();
            println!(
                "  {index} {}",
                describe(
                    program,
                    args.settlement_program,
                    &instruction.data,
                    &accounts
                )
            );
            for (position, account) in accounts.iter().take(SHOWN_ACCOUNTS).enumerate() {
                println!("      {position} {account}");
            }
            if accounts.len() > SHOWN_ACCOUNTS {
                println!("      +{} more", accounts.len() - SHOWN_ACCOUNTS);
            }
        }
    }
    Ok(())
}

/// The message's account keys in index order: the static keys, then the
/// writable and the readonly addresses its lookup tables load.
async fn account_keys(message: &VersionedMessage, rpc: Option<&SolanaRPC>) -> Result<Vec<Pubkey>> {
    let mut keys = message.static_account_keys().to_vec();
    let Some(lookups) = message
        .address_table_lookups()
        .filter(|lookups| !lookups.is_empty())
    else {
        return Ok(keys);
    };
    let rpc = rpc.context("a v0 message with lookup tables needs --rpc-url")?;
    let tables = rpc
        .multiple_accounts(lookups.iter().map(|lookup| lookup.account_key))
        .await?;
    let mut writable = Vec::new();
    let mut readonly = Vec::new();
    for lookup in lookups {
        let table = tables
            .get(&lookup.account_key)
            .with_context(|| format!("lookup table {} not found", lookup.account_key))?;
        let addresses = table
            .data
            .get(LOOKUP_TABLE_META_SIZE..)
            .unwrap_or_default()
            .as_chunks::<32>()
            .0
            .iter()
            .map(|address| Pubkey::new_from_array(*address))
            .collect::<Vec<_>>();
        let resolve = |index: &u8| {
            addresses
                .get(usize::from(*index))
                .copied()
                .with_context(|| format!("index {index} past lookup table {}", lookup.account_key))
        };
        for index in &lookup.writable_indexes {
            writable.push(resolve(index)?);
        }
        for index in &lookup.readonly_indexes {
            readonly.push(resolve(index)?);
        }
    }
    keys.extend(writable);
    keys.extend(readonly);
    Ok(keys)
}

/// The instruction's program, kind and arguments.
fn describe(
    program: Pubkey,
    settlement_program: Pubkey,
    data: &[u8],
    accounts: &[Pubkey],
) -> String {
    match program {
        solana_system_interface::program::ID => {
            format!(
                "System {:?}",
                bincode::deserialize::<SystemInstruction>(data)
            )
        }
        spl_token_interface::ID => format!("Token {:?}", TokenInstruction::unpack(data)),
        spl_token_2022_interface::ID => format!("Token-2022 {:?}", TokenInstruction::unpack(data)),
        spl_associated_token_account_interface::program::ID => match data.first() {
            None | Some(0) => "ATA Create".to_owned(),
            Some(1) => "ATA CreateIdempotent".to_owned(),
            Some(2) => "ATA RecoverNested".to_owned(),
            Some(other) => format!("ATA instruction {other}"),
        },
        solana_compute_budget_interface::ID => format!("ComputeBudget {}", compute_budget(data)),
        _ if program == settlement_program => format!("Settlement {}", settlement(data, accounts)),
        _ => format!("{program}, {} data bytes", data.len()),
    }
}

fn compute_budget(data: &[u8]) -> String {
    let u32_arg = || {
        data.get(1..5)
            .map(|bytes| u32::from_le_bytes(bytes.try_into().expect("4 bytes")))
    };
    match data.first() {
        Some(1) => format!("RequestHeapFrame {:?}", u32_arg()),
        Some(2) => format!("SetComputeUnitLimit {:?}", u32_arg()),
        Some(3) => format!(
            "SetComputeUnitPrice {:?}",
            data.get(1..9)
                .map(|bytes| u64::from_le_bytes(bytes.try_into().expect("8 bytes")))
        ),
        Some(4) => format!("SetLoadedAccountsDataSizeLimit {:?}", u32_arg()),
        _ => format!("{data:?}"),
    }
}

fn settlement(data: &[u8], accounts: &[Pubkey]) -> String {
    let kind = match recover_discriminator(data) {
        Ok((kind, _)) => kind,
        Err(err) => return format!("unknown instruction: {err:?}"),
    };
    let parsed = match kind {
        SettlementInstruction::BeginSettle => {
            BeginSettleInput::parse(data, accounts).map(|input| {
                let orders = input
                    .orders
                    .iter()
                    .map(|order| {
                        let pulls = order
                            .destinations
                            .iter()
                            .zip(order.amounts)
                            .map(|(destination, amount)| {
                                format!("{} to {destination}", u64::from_le_bytes(*amount))
                            })
                            .collect::<Vec<_>>();
                        format!(
                            "order {} from {} pulls [{}]",
                            order.order_pda,
                            order.sell_token_account,
                            pulls.join(", ")
                        )
                    })
                    .collect::<Vec<_>>();
                format!(
                    "BeginSettle finalize at {}, auction {}, solver {}, {}",
                    input.finalize_ix_index,
                    input.auction_id,
                    input.solver_account,
                    orders.join("; ")
                )
            })
        }
        SettlementInstruction::FinalizeSettle => {
            FinalizeSettleInput::parse(data, accounts).map(|input| {
                let pushes = input
                    .pushes
                    .iter()
                    .map(|push| {
                        format!(
                            "{} from {} to {}",
                            push.amount, push.source_buffer, push.destination
                        )
                    })
                    .collect::<Vec<_>>();
                format!(
                    "FinalizeSettle begin at {}, pushes [{}]",
                    input.begin_ix_index,
                    pushes.join(", ")
                )
            })
        }
        SettlementInstruction::CreateOrder => {
            CreateOrderInput::parse(data, accounts).map(|input| {
                format!(
                    "CreateOrder owner {}, created by {}, order {}, {:?}",
                    input.owner,
                    input.created_by,
                    input.order_pda,
                    OrderIntent::try_from(&input.intent_bytes)
                )
            })
        }
        other => Ok(format!("{other:?}")),
    };
    parsed.unwrap_or_else(|err| format!("{kind:?}, unparsable: {err:?}"))
}
