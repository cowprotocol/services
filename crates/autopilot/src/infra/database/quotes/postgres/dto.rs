use {
    crate::{boundary, domain},
    number::conversions::big_decimal_to_u256,
    primitive_types::H160,
};

pub fn into_domain(quote: database::orders::Quote) -> Result<domain::Quote, InvalidConversion> {
    Ok(domain::Quote {
        order_uid: boundary::OrderUid(quote.order_uid.0),
        gas_amount: quote.gas_amount,
        gas_price: quote.gas_price,
        sell_token_price: quote.sell_token_price,
        sell_amount: big_decimal_to_u256(&quote.sell_amount).ok_or(InvalidConversion)?,
        buy_amount: big_decimal_to_u256(&quote.buy_amount).ok_or(InvalidConversion)?,
        solver: H160(quote.solver.0),
    })
}

#[derive(Debug, thiserror::Error)]
#[error("invalid conversion from database to domain")]
pub struct InvalidConversion;
