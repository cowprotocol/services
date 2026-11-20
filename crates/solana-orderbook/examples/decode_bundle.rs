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
//!
//! `--simulate` merges the transactions into one and simulates it with
//! `simulateTransaction`, which returns the logs `simulateBundle` drops for a
//! failed transaction. It reads the current state, not the state at the
//! logged `slot`, so simulate soon after the capture:
//!
//! ```sh
//! cargo run -p solana-orderbook --example decode_bundle -- --rpc-url <url> --simulate '["AQ...", "AQ..."]'
//! ```

use {
    anyhow::{Context, Result, bail},
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
    cow_solana_rpc::{CommitmentConfig, Response, RpcSimulateTransactionResult, SolanaRPC},
    solana_sdk::{
        hash::Hash,
        instruction::{AccountMeta, Instruction},
        message::{VersionedMessage, compiled_instruction::CompiledInstruction, v1},
        pubkey::Pubkey,
        signature::Signature,
        transaction::{TransactionError, VersionedTransaction},
    },
    solana_system_interface::instruction::SystemInstruction,
    spl_token_2022_interface::instruction::TokenInstruction,
    std::time::Duration,
    url::Url,
};

/// Bytes before the first address of an address lookup table account.
const LOOKUP_TABLE_META_SIZE: usize = 56;

/// Accounts listed per instruction, the rest are only counted.
const SHOWN_ACCOUNTS: usize = 8;

/// The most compute units a transaction may declare.
const MAX_COMPUTE_UNIT_LIMIT: u32 = 1_400_000;

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

    /// Also simulates the transactions merged into one, in order. The last
    /// must be a v1 settlement, whose config the merged transaction takes.
    #[arg(long)]
    simulate: bool,

    /// The merged transaction's compute unit limit, which all of its parts
    /// spend from.
    #[arg(long, default_value_t = MAX_COMPUTE_UNIT_LIMIT)]
    compute_unit_limit: u32,
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
    let mut decoded = Vec::new();
    for (n, transaction) in transactions.iter().enumerate() {
        let bytes = BASE64_STANDARD.decode(transaction.trim())?;
        // v1 messages need wincode, but orderbook v2.388 logs them in
        // bincode's serde layout.
        let transaction: VersionedTransaction =
            wincode::deserialize(&bytes).or_else(|_| bincode::deserialize(&bytes))?;
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
        decoded.push((transaction, keys));
    }
    if args.simulate {
        let rpc = rpc.as_ref().context("--simulate needs --rpc-url")?;
        simulate(
            rpc,
            &decoded,
            args.settlement_program,
            args.compute_unit_limit,
        )
        .await?;
    }
    Ok(())
}

/// Simulates `transactions`, each with its account keys, merged into one, and
/// prints where it fails and its logs.
async fn simulate(
    rpc: &SolanaRPC,
    transactions: &[(VersionedTransaction, Vec<Pubkey>)],
    settlement_program: Pubkey,
    compute_unit_limit: u32,
) -> Result<()> {
    let settlement = settlement_message(transactions)?;
    let (instructions, origins) = merge(transactions, settlement_program)?;
    let config = settlement
        .config
        .with_compute_unit_limit(compute_unit_limit);
    // The simulation replaces the blockhash and checks no signature.
    let merged = v1::Message::try_compile_with_config(
        &settlement.account_keys[0],
        &instructions,
        Hash::default(),
        config,
    )?;
    let transaction = VersionedTransaction {
        signatures: vec![Signature::default(); usize::from(merged.header.num_required_signatures)],
        message: VersionedMessage::V1(merged),
    };
    let response = rpc.simulate_unsigned_transaction(&transaction).await?;
    print_result(response, &origins, settlement.config.compute_unit_limit);
    Ok(())
}

/// The last transaction's message, the v1 settlement whose config the merged
/// transaction takes.
fn settlement_message(
    transactions: &[(VersionedTransaction, Vec<Pubkey>)],
) -> Result<&v1::Message> {
    let Some((settlement, _)) = transactions.last() else {
        bail!("no transaction to simulate");
    };
    let VersionedMessage::V1(message) = &settlement.message else {
        bail!("the last transaction is not a v1 settlement");
    };
    Ok(message)
}

/// The transaction a merged instruction comes from and its position there.
type Origin = (usize, usize);

/// The instructions of `transactions` in order, each with its [`Origin`].
fn merge(
    transactions: &[(VersionedTransaction, Vec<Pubkey>)],
    settlement_program: Pubkey,
) -> Result<(Vec<Instruction>, Vec<Origin>)> {
    let mut instructions = Vec::new();
    let mut origins = Vec::new();
    for (n, (transaction, keys)) in transactions.iter().enumerate() {
        let offset = u16::try_from(instructions.len())?;
        for (index, compiled) in transaction.message.instructions().iter().enumerate() {
            let mut instruction = decompile(&transaction.message, keys, compiled);
            if instruction.program_id == settlement_program {
                shift_counterpart(&mut instruction.data, offset)?;
            }
            instructions.push(instruction);
            origins.push((n, index));
        }
    }
    Ok((instructions, origins))
}

/// `compiled` with its accounts resolved through `keys`, the account keys of
/// `message`.
fn decompile(
    message: &VersionedMessage,
    keys: &[Pubkey],
    compiled: &CompiledInstruction,
) -> Instruction {
    let accounts = compiled
        .accounts
        .iter()
        .map(|&account| {
            let account = usize::from(account);
            AccountMeta {
                pubkey: keys[account],
                is_signer: message.is_signer(account),
                is_writable: message.is_maybe_writable(account, None),
            }
        })
        .collect();
    Instruction {
        program_id: keys[usize::from(compiled.program_id_index)],
        accounts,
        data: compiled.data.clone(),
    }
}

/// Prints the compute units the simulation used against the settlement's
/// `declared_limit`, where it fails and its logs.
fn print_result(
    response: Response<RpcSimulateTransactionResult>,
    origins: &[Origin],
    declared_limit: Option<u32>,
) {
    let result = response.value;
    println!(
        "simulated at slot {}, {} compute units used, the settlement declares {declared_limit:?}",
        response.context.slot,
        result.units_consumed.unwrap_or_default(),
    );
    let err = result.err.map(TransactionError::from);
    if let Some(TransactionError::InstructionError(at, failure)) = &err
        && let Some((n, index)) = origins.get(usize::from(*at))
    {
        println!("  fails at transaction {n} instruction {index}: {failure:?}");
    } else if let Some(err) = err {
        println!("  fails: {err:?}");
    }
    for line in result.logs.unwrap_or_default() {
        println!("  {line}");
    }
}

/// Adds `offset` to the position of the counterpart a `BeginSettle` or a
/// `FinalizeSettle` names, a u16 LE right after the discriminator. Other
/// instructions keep their data.
fn shift_counterpart(data: &mut [u8], offset: u16) -> Result<()> {
    let kind = recover_discriminator(data).map(|(kind, _)| kind);
    if !matches!(
        kind,
        Ok(SettlementInstruction::BeginSettle | SettlementInstruction::FinalizeSettle)
    ) {
        return Ok(());
    }
    let position = data
        .get_mut(1..3)
        .context("settle instruction without its counterpart's position")?;
    let shifted = u16::from_le_bytes([position[0], position[1]])
        .checked_add(offset)
        .context("counterpart position overflows")?;
    position.copy_from_slice(&shifted.to_le_bytes());
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
