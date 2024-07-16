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
        sell_amount: big_decimal_to_u256(&quote.sell_amount)
            .ok_or(AmountOverflow)?
            .into(),
        buy_amount: big_decimal_to_u256(&quote.buy_amount)
            .ok_or(AmountOverflow)?
            .into(),
        fee: U256::from_f64_lossy(quote.gas_amount * quote.gas_price / quote.sell_token_price)
            .into(),
    })
}

#[derive(Debug, thiserror::Error)]
#[error("amount overflow")]
pub struct AmountOverflow;
