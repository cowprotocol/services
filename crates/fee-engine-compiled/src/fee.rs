//! The fee functions: `volume`, `surplus`, `price_improvement`, `no_fee`.

use {
    crate::{EvalError, node::Val, number::Num},
    fee_engine::{FactorError, Fee, FeeFactor, FeeKind},
    rust_decimal::Decimal,
};

/// Steps of 1e-6 of the volume per 1, per bps and per percent.
const GRID: u64 = 1_000_000;
const BPS: u64 = GRID / 10_000;
const PERCENT: u64 = GRID / 100;

/// `number` in `grid_units` as a factor; off-grid values are refused, as in
/// `fee_engine`.
fn factor(
    kind: FeeKind,
    argument: &'static str,
    number: Decimal,
    grid_units: u64,
) -> Result<FeeFactor, EvalError> {
    let error = |error| EvalError::Fee {
        kind,
        argument,
        value: number,
        error,
    };
    let scaled = number
        .checked_mul(Decimal::from(grid_units))
        .ok_or(error(FactorError::OutOfRange))?;
    if !scaled.is_integer() {
        return Err(error(FactorError::OffGrid));
    }
    FeeFactor::try_from(scaled / Decimal::from(GRID)).map_err(error)
}

/// Compiles a call to a fee function, or `None` if `name` is not one.
pub(crate) fn call(name: &str, args: Vec<Num>) -> Result<Option<Val<Fee>>, String> {
    let kind = match name {
        "volume" => FeeKind::Volume,
        "surplus" => FeeKind::Surplus,
        "price_improvement" => FeeKind::PriceImprovement,
        _ => return Ok(None),
    };
    let arity = if kind == FeeKind::Volume { 1 } else { 2 };
    if args.len() != arity {
        return Err(format!(
            "`{name}` takes {arity} arguments, not {}",
            args.len()
        ));
    }
    let mut args = args
        .into_iter()
        .map(Num::into_decimal)
        .collect::<Result<Vec<_>, _>>()?;
    let fee = if kind == FeeKind::Volume {
        args.pop().unwrap().map(|bps| {
            Ok(Fee::Volume {
                factor: factor(FeeKind::Volume, "bps", bps, BPS)?,
            })
        })?
    } else {
        let max = args.pop().unwrap();
        let share = args.pop().unwrap();
        crate::node::map2(share, max, move |share, max| {
            let share = factor(kind, "share", share, PERCENT)?;
            let max_volume_factor = factor(kind, "max_volume_bps", max, BPS)?;
            Ok(if kind == FeeKind::Surplus {
                Fee::Surplus {
                    factor: share,
                    max_volume_factor,
                }
            } else {
                Fee::PriceImprovement {
                    factor: share,
                    max_volume_factor,
                }
            })
        })?
    };
    Ok(Some(fee))
}
