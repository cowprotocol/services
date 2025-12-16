//! Balance, allowance, and transfer simulation checks
//!
//! This module handles balance and allowance verification.
//! All functions are **asynchronous** - they require blockchain state queries.

use {
    crate::order_validation::types::*,
    account_balances::{BalanceFetching, Query, TransferSimulationError},
    ethcontract::H160,
    ethrpc::alloy::conversions::{IntoAlloy, IntoLegacy},
    model::order::{OrderCreation, Signature},
    number::nonzero::U256,
    std::sync::Arc,
};

/// Ensures the order has sufficient balance and allowance for the sell token
///
/// # Async
/// This function is asynchronous and queries blockchain state for:
/// - Token balance
/// - Token allowance
/// - Transfer simulation
///
/// # Arguments
/// - `order`: The order to validate
/// - `owner`: The order owner address
/// - `app_data`: Validated app data (contains flashloan info)
/// - `balance_fetcher`: The balance fetcher trait implementation
///
/// # Returns
/// - `Ok(())` if token is transferable
/// - `Err(ValidationError::InsufficientBalance)` if balance is insufficient
/// - `Err(ValidationError::InsufficientAllowance)` if allowance is insufficient
/// - `Err(ValidationError::TransferSimulationFailed)` if simulation fails
pub async fn ensure_transferable(
    order: &OrderCreation,
    owner: H160,
    app_data: &OrderAppData,
    balance_fetcher: &Arc<dyn BalanceFetching>,
) -> Result<(), ValidationError> {
    let mut last_error = Err(ValidationError::TransferSimulationFailed);

    // Simulate transferring a small token balance into the settlement contract.
    // As a spam protection we require that an account must have at least 1 atom
    // of the sell_token. However, some tokens (e.g. rebasing tokens) actually run
    // into numerical issues with such small amounts. But there are also tokens
    // where a single atom is already quite expensive (tokenized stocks).
    // To cover both cases we simulate multiple small transfers. As soon as one
    // passes we consider the token transferable. If all transfers fail we return
    // the last error.
    for transfer_amount in [1, 10, 100].into_iter().map(U256::from) {
        let order_data = order.data();

        match balance_fetcher
            .can_transfer(
                &Query {
                    token: order_data.sell_token,
                    owner: owner.into_alloy(),
                    source: order_data.sell_token_balance,
                    interactions: app_data.interactions.pre.clone(),
                    balance_override: app_data.inner.protocol.flashloan.as_ref().map(|loan| {
                        price_estimation::trade_verifier::balance_overrides::BalanceOverrideRequest {
                            token: loan.token.into_legacy(),
                            holder: loan.receiver.into_legacy(),
                            amount: loan.amount.into_legacy(),
                        }
                    }),
                },
                transfer_amount,
            )
            .await
        {
            Ok(_) => {
                // Transfer succeeded, token is transferable
                return Ok(());
            }
            Err(
                TransferSimulationError::InsufficientAllowance
                | TransferSimulationError::InsufficientBalance
                | TransferSimulationError::TransferFailed,
            ) if order.signature == Signature::PreSign => {
                // Pre-sign orders do not require sufficient balance or allowance.
                // The idea is that this allows smart contracts to place orders bundled with
                // other transactions that either produce the required balance or set the
                // allowance. This would, for example, allow a Gnosis Safe to bundle the
                // pre-signature transaction with a WETH wrap and WETH approval to the vault
                // relayer contract.
                return Ok(());
            }
            Err(err) => {
                last_error = Err(match err {
                    TransferSimulationError::InsufficientAllowance => {
                        // This error will be triggered regardless of the amount
                        return Err(ValidationError::InsufficientAllowance);
                    }
                    TransferSimulationError::InsufficientBalance => {
                        // Since the amount starts at 1 atom, if this error is triggered then it
                        // will be triggered for the other amounts too
                        return Err(ValidationError::InsufficientBalance);
                    }
                    TransferSimulationError::TransferFailed => {
                        ValidationError::TransferSimulationFailed
                    }
                    TransferSimulationError::Other(err) => {
                        tracing::warn!("TransferSimulation failed: {:?}", err);
                        ValidationError::TransferSimulationFailed
                    }
                });
            }
        }
    }

    // All transfer simulations failed
    last_error
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balance_check_stub() {
        // Tests will be implemented when full integration is ready
    }
}
