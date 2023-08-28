pub mod u256;

pub fn rational_to_big_decimal<T>(value: &num::rational::Ratio<T>) -> bigdecimal::BigDecimal
where
    T: Clone,
    num::BigInt: From<T>,
{
    let numer = num::BigInt::from(value.numer().clone());
    let denom = num::BigInt::from(value.denom().clone());
    bigdecimal::BigDecimal::new(numer, 0) / bigdecimal::BigDecimal::new(denom, 0)
}
