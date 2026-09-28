mod account;
mod create_order;
mod healthz;
mod order;
mod quote;
mod status;
mod trades;

pub use {
    account::account_orders,
    create_order::create_order,
    healthz::healthz,
    order::order,
    quote::quote,
    status::order_status,
    trades::trades,
};
use {
    cow_settlement_interface::data::intent::ENCODED_NATIVE_SOL_TRANSFER,
    solana_sdk::pubkey::Pubkey,
    spl_token_interface::native_mint,
};

/// Whether an order trades a token for itself. Native SOL counts as wSOL, so
/// selling wSOL for native SOL is a same-token trade.
fn same_token(sell: &Pubkey, buy: &Pubkey) -> bool {
    sell == buy || (*sell == native_mint::ID && *buy == ENCODED_NATIVE_SOL_TRANSFER)
}
