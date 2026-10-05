mod account;
mod create_order;
mod healthz;
mod mint;
mod order;
mod quote;
mod status;
mod trades;

use solana_sdk::{account::Account, rent::Rent};
pub use {
    account::account_orders,
    create_order::create_order,
    healthz::healthz,
    order::order,
    quote::quote,
    status::order_status,
    trades::trades,
};

/// Whether `wallet` can take a native SOL payout of at least `buy_amount`. A
/// credit that leaves a wallet under its rent-exempt minimum reverts the
/// settlement, so a wallet under it takes only a fill-or-kill payout that
/// lifts it over. A missing wallet counts as empty.
///
/// TODO: use the cluster's rent, refreshed periodically. The SDK default is
/// above it since SIMD-0437, so this floor also rejects small payouts that
/// would settle.
fn receivable_native_payout(
    wallet: Option<&Account>,
    buy_amount: u64,
    partially_fillable: bool,
) -> bool {
    let (lamports, len) = wallet.map_or((0, 0), |wallet| (wallet.lamports, wallet.data.len()));
    let floor = Rent::default().minimum_balance(len);
    lamports >= floor || (!partially_fillable && lamports.saturating_add(buy_amount) >= floor)
}

#[cfg(test)]
mod tests {
    use {super::*, solana_sdk::pubkey::Pubkey};

    /// A wallet at the rent-exempt minimum takes any payout, partial fills
    /// included. A wallet under it, or a missing one, takes only a
    /// fill-or-kill payout that lifts it over. The minimum grows with the
    /// wallet's data.
    #[test]
    fn native_payouts_leave_wallets_rent_exempt() {
        let floor = Rent::default().minimum_balance(0);
        let wallet = |lamports, len| Some(Account::new(lamports, len, &Pubkey::default()));
        for (account, buy_amount, partially_fillable, receivable) in [
            (None, floor, false, true),
            (None, floor - 1, false, false),
            (None, u64::MAX, true, false),
            (wallet(floor, 0), 1, true, true),
            (wallet(floor - 1, 0), u64::MAX, false, true),
            (wallet(floor - 1_000, 0), 1_000, false, true),
            (wallet(floor - 1_000, 0), 999, false, false),
            (wallet(floor, 8), 1, false, false),
        ] {
            assert_eq!(
                receivable_native_payout(account.as_ref(), buy_amount, partially_fillable),
                receivable,
                "{account:?} {buy_amount} {partially_fillable}"
            );
        }
    }
}
