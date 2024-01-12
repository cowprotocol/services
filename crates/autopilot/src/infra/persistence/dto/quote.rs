use {
    crate::{boundary, domain},
    number::conversions::big_decimal_to_u256,
    primitive_types::U256,
};

pub fn into_domain(
    quote: boundary::database::orders::Quote,
) -> Result<domain::Quote, AmountOverflow> {
    Ok(domain::Quote {
        order_uid: domain::OrderUid(quote.order_uid.0),
        sell_amount: big_decimal_to_u256(&quote.sell_amount).ok_or(AmountOverflow)?,
        buy_amount: big_decimal_to_u256(&quote.buy_amount).ok_or(AmountOverflow)?,
        fee: {
            let gas_amount = U256::from_f64_lossy(quote.gas_amount);
            let gas_price = U256::from_f64_lossy(quote.gas_price);
            let sell_token_price = U256::from_f64_lossy(quote.sell_token_price);
            gas_amount
                .checked_mul(gas_price)
                .ok_or(AmountOverflow)?
                .checked_div(sell_token_price)
                .ok_or(AmountOverflow)?
        },
    })
}

#[derive(Debug, thiserror::Error)]
#[error("invalid conversion from database to domain")]
pub struct AmountOverflow;
