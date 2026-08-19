//! Minimal settlement transaction assembly.
//!
//! Builds `create_buffers + begin_settle + finalize_settle` from a stored
//! solution, signs with the solver keypair, and submits. The solver
//! pre-funds the canonical buffers out of band (the pure-CoW custody model).
//!
//! TODO: predicted demo-grade shape. The real settlement encoding adds the
//! solution's interactions between begin and finalize (routing the swap
//! output into the buffers), address lookup tables, compute budget, and a
//! simulation gate before submission.

use {
    crate::{domain, infra::api::dto, util},
    cow_solana_rpc::SolanaRPC,
    settlement_interface::{
        instruction::{
            create_buffer::CreateBuffers,
            settle::{BeginSettle, FinalizeSettle, Pull},
        },
        pda::{buffer::find_buffer_pda, order::find_order_pda, state::find_state_pda},
    },
    solana_sdk::{
        hash::Hash,
        pubkey::Pubkey,
        signature::{Keypair, Signature},
        signer::Signer,
        transaction::Transaction,
    },
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum Error {
    /// A traded order is missing from the stored auction.
    #[error("trade references order {0} outside the stored auction")]
    UnknownOrder(domain::order_uid::OrderUid),
    /// The wire order PDA does not match the canonical derivation.
    #[error("order {0} carries a non-canonical order PDA")]
    NonCanonicalOrderPda(domain::order_uid::OrderUid),
    /// TODO: buy orders need the sell-side amount, which the engine wire
    /// does not carry.
    #[error("order {0} is a buy order, unsupported")]
    BuyOrdersUnsupported(domain::order_uid::OrderUid),
    /// The RPC rejected or failed the submission.
    #[error(transparent)]
    Rpc(#[from] cow_solana_rpc::Error),
}

/// One order's settlement inputs, kept from the solve request for the later
/// `/settle`.
pub struct SettleOrder {
    pub uid: domain::order_uid::OrderUid,
    pub kind: dto::Kind,
    pub sell_mint: Pubkey,
    pub sell_token_account: Pubkey,
    pub buy_token_account: Pubkey,
    pub buy_mint: Pubkey,
    /// The order's buy limit. The demo settles sells exactly at the limit,
    /// so this is also the pushed amount.
    pub buy_amount: u64,
    pub order_pda: Pubkey,
}

impl From<&dto::Order> for SettleOrder {
    fn from(order: &dto::Order) -> Self {
        Self {
            uid: order.uid,
            kind: order.kind,
            sell_mint: order.sell_token,
            sell_token_account: order.sell_token_account,
            buy_token_account: order.buy_token_account,
            buy_mint: order.buy_token,
            buy_amount: order.buy_amount,
            order_pda: order.order_pda,
        }
    }
}

/// Build, sign, and submit the settlement for the solution's trades.
pub async fn submit(
    rpc: &SolanaRPC,
    keypair: &Keypair,
    program_id: &Pubkey,
    auction_id: i64,
    orders: &[SettleOrder],
    solution: &domain::Solution,
) -> Result<Signature, Error> {
    let payer = keypair.pubkey();
    let (state_pda, _) = find_state_pda(program_id);

    // The program requires the settled orders strictly increasing by their
    // order PDA, and every pull lands in the solver's sell token account per
    // the custody model.
    let mut trades = solution
        .trades
        .iter()
        .map(|trade| {
            let order = orders
                .iter()
                .find(|order| order.uid == trade.order_uid)
                .ok_or(Error::UnknownOrder(trade.order_uid))?;
            if order.kind == dto::Kind::Buy {
                return Err(Error::BuyOrdersUnsupported(trade.order_uid));
            }
            let (order_pda, bump) = find_order_pda(program_id, &Hash::new_from_array(order.uid.0));
            if order_pda != order.order_pda {
                return Err(Error::NonCanonicalOrderPda(trade.order_uid));
            }
            Ok((order, trade, order_pda, bump))
        })
        .collect::<Result<Vec<_>, Error>>()?;
    trades.sort_by_key(|(_, _, order_pda, _)| *order_pda);

    let buffers: Vec<(Pubkey, Pubkey)> = trades
        .iter()
        .map(|(order, ..)| {
            (
                find_buffer_pda(program_id, &order.buy_mint).0,
                order.buy_mint,
            )
        })
        .collect();
    let order_pdas: Vec<Pubkey> = trades.iter().map(|(.., pda, _)| *pda).collect();
    let order_pda_bumps: Vec<u8> = trades.iter().map(|(.., bump)| *bump).collect();
    let sell_token_accounts: Vec<Pubkey> = trades
        .iter()
        .map(|(order, ..)| order.sell_token_account)
        .collect();
    // Pulled sell tokens land in the solver's own token account, the
    // custody model the engines quote against.
    let pulls: Vec<Vec<Pull>> = trades
        .iter()
        .map(|(order, trade, ..)| {
            vec![Pull {
                destination: util::associated_token_address(&payer, &order.sell_mint),
                amount: trade.executed_amount,
            }]
        })
        .collect();
    let pull_refs: Vec<&[Pull]> = pulls.iter().map(Vec::as_slice).collect();
    let (source_buffers, bumps): (Vec<Pubkey>, Vec<u8>) = trades
        .iter()
        .map(|(order, ..)| find_buffer_pda(program_id, &order.buy_mint))
        .unzip();
    let destinations: Vec<Pubkey> = trades
        .iter()
        .map(|(order, ..)| order.buy_token_account)
        .collect();
    // TODO: the demo settles sells exactly at the order's buy limit, the
    // engine wire carries no executed buy amount. Real encoding derives
    // pushes from the solution.
    let amounts: Vec<u64> = trades.iter().map(|(order, ..)| order.buy_amount).collect();

    // Instruction layout: [create_buffers, begin, finalize], the settle pair
    // cross-references by top-level index.
    let instructions = vec![
        CreateBuffers {
            program_id: *program_id,
            payer,
            buffers: &buffers,
        }
        .into(),
        BeginSettle {
            program_id: *program_id,
            state_pda,
            finalize_ix_index: 2,
            auction_id,
            order_pdas: &order_pdas,
            order_pda_bumps: &order_pda_bumps,
            sell_token_accounts: &sell_token_accounts,
            pulls: &pull_refs,
        }
        .into(),
        FinalizeSettle {
            program_id: *program_id,
            state_pda,
            begin_ix_index: 1,
            source_buffers: &source_buffers,
            destinations: &destinations,
            bumps: &bumps,
            amounts: &amounts,
        }
        .into(),
    ];

    let blockhash = rpc.latest_blockhash().await?;
    let transaction =
        Transaction::new_signed_with_payer(&instructions, Some(&payer), &[keypair], blockhash);
    Ok(rpc.send_transaction(&transaction).await?)
}
