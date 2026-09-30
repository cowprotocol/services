mod account;
mod create_order;
mod healthz;
mod mint;
mod order;
mod quote;
mod status;
mod trades;

use solana_sdk::rent::Rent;
pub use {
    account::account_orders,
    create_order::create_order,
    healthz::healthz,
    order::order,
    quote::quote,
    status::order_status,
    trades::trades,
};

/// The smallest native SOL payout a settlement can make: the rent-exempt
/// minimum of an empty account. A smaller payout into a missing wallet
/// reverts the whole settlement.
///
/// TODO: use the cluster's rent, refreshed periodically. The SDK default is
/// above it since SIMD-0437, so this floor also rejects small payouts that
/// would settle.
fn min_native_payout() -> u64 {
    Rent::default().minimum_balance(0)
}
