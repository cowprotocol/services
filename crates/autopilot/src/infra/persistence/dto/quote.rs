use {
    crate::{
        boundary,
        domain::{self, eth},
    },
    bigdecimal::{
        FromPrimitive,
        num_traits::{CheckedDiv, CheckedMul},
    },
    num::BigRational,
    number::conversions::{big_decimal_to_u256, big_rational_to_u256},
};

pub fn into_domain(quote: boundary::database::orders::Quote) -> Result<domain::Quote, QuoteError> {
    let gas_amount = BigRational::from_f64(quote.gas_amount).ok_or(QuoteError::InvalidInput)?;
    let gas_price = BigRational::from_f64(quote.gas_price).ok_or(QuoteError::InvalidInput)?;
    let sell_token_price =
        BigRational::from_f64(quote.sell_token_price).ok_or(QuoteError::InvalidInput)?;
    let fee = big_rational_to_u256(
        &gas_amount
            .checked_mul(&gas_price)
            .ok_or(QuoteError::BigRationalOverflow)?
            .checked_div(&sell_token_price)
            .ok_or(QuoteError::DivisionByZero)?,
    )
    .map_err(QuoteError::Error)?;
    Ok(domain::Quote {
        order_uid: domain::OrderUid(quote.order_uid.0),
        sell_amount: big_decimal_to_u256(&quote.sell_amount)
            .ok_or(QuoteError::U256Overflow)?
            .into(),
        buy_amount: big_decimal_to_u256(&quote.buy_amount)
            .ok_or(QuoteError::U256Overflow)?
            .into(),
        fee: fee.into(),
        solver: eth::H160::from(quote.solver.0).into(),
    })
}

pub fn competition_metadata_into_domain(
    metadata: boundary::database::solver_competition::Metadata,
) -> domain::competition::Metadata {
    domain::competition::Metadata {
        auction_id: metadata.auction_id,
        solver: eth::Address::from(eth::H160(metadata.solver.0)),
        settled: metadata.settled,
    }
}

#[derive(Debug, thiserror::Error)]
pub enum QuoteError {
    #[error("BigRational amount overflow")]
    BigRationalOverflow,
    #[error("U256 amount overflow")]
    U256Overflow,
    #[error("invalid BigRational input")]
    InvalidInput,
    #[error("division by zero")]
    DivisionByZero,
    #[error(transparent)]
    Error(#[from] anyhow::Error),
}
