use num::{BigRational, ToPrimitive as _};

pub fn big_rational_to_float(ratio: BigRational) -> Option<f64> {
    Some(ratio.numer().to_f64()? / ratio.denom().to_f64()?)
}
