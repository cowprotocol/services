use alloy::primitives::{Address, U256};

/// Note that get_amount_out and get_amount_in are not always symmetrical. That
/// is for some AMMs it is possible that get_amount_out returns an amount for
/// which get_amount_in returns None when trying to go the reverse direction. Or
/// that the resulting amount is different from the original. This situation is
/// rare and resulting amounts should usually be identical or very close but it
/// can occur.
pub trait BaselineSolvable {
    /// Given the desired output token, the amount and token input, return the
    /// expected amount of output token.
    fn get_amount_out(
        &self,
        out_token: Address,
        input: (U256, Address),
    ) -> impl Future<Output = Option<U256>> + Send;

    /// Given the input token, the amount and token we want output, return the
    /// required amount of input token that needs to be provided.
    fn get_amount_in(
        &self,
        in_token: Address,
        out: (U256, Address),
    ) -> impl Future<Output = Option<U256>> + Send;

    /// Returns the approximate amount of gas that using this piece of liquidity
    /// would incur
    fn gas_cost(&self) -> impl Future<Output = usize> + Send;
}
