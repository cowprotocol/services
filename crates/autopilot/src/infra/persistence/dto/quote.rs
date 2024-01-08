use {
    crate::{boundary, domain},
    number::conversions::big_decimal_to_u256,
};

pub fn into_domain(quote: boundary::DatabaseQuote) -> Result<domain::Quote, AmountOverflow> {
    Ok(domain::Quote {
        order_uid: domain::OrderUid(quote.order_uid.0),
        sell_amount: big_decimal_to_u256(&quote.sell_amount).ok_or(AmountOverflow)?,
        buy_amount: big_decimal_to_u256(&quote.buy_amount).ok_or(AmountOverflow)?,
    })
}

#[derive(Debug, thiserror::Error)]
#[error("invalid conversion from database to domain")]
pub struct AmountOverflow;
