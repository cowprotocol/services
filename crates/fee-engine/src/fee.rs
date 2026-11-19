//! Fees a rule returns and the functions that build them.

use {
    crate::{
        FeeFactor,
        RuleError,
        compile::{
            node::{Val, map2},
            number::Num,
        },
    },
    rust_decimal::Decimal,
    std::fmt,
};

/// A protocol fee a rule returns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Fee {
    /// A share of the order's volume.
    Volume { factor: FeeFactor },
    /// A share of the surplus over the limit price, capped at a share of the
    /// volume.
    Surplus {
        factor: FeeFactor,
        max_volume_factor: FeeFactor,
    },
    /// A share of the improvement over the quote, capped at a share of the
    /// volume.
    PriceImprovement {
        factor: FeeFactor,
        max_volume_factor: FeeFactor,
    },
}

/// The kinds of [`Fee`], by the function that builds each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeeKind {
    Volume,
    Surplus,
    PriceImprovement,
}

impl FeeKind {
    /// The name of the function that builds it.
    pub fn name(self) -> &'static str {
        match self {
            FeeKind::Volume => "volume",
            FeeKind::Surplus => "surplus",
            FeeKind::PriceImprovement => "price_improvement",
        }
    }
}

impl fmt::Display for FeeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A fee argument is a whole number of steps of 1e-6 of the volume (the
/// "grid"): this many steps per 1.
const GRID: u64 = 1_000_000;
/// Steps per basis point.
pub(crate) const BPS: u64 = GRID / 10_000;
/// Steps per percent.
pub(crate) const PERCENT: u64 = GRID / 100;

/// `number` in the unit `grid_units` sizes ([`PERCENT`], [`BPS`]) as a fee
/// factor: `0.3` with `BPS` is 30 steps of 1e-6, i.e. 0.00003. A value that
/// is not a whole number of steps is refused, not rounded.
pub(crate) fn factor_on_grid(number: Decimal, grid_units: u64) -> Result<FeeFactor, FactorError> {
    let scaled = number
        .checked_mul(Decimal::from(grid_units))
        .ok_or(FactorError::OutOfRange)?;
    if !scaled.is_integer() {
        return Err(FactorError::OffGrid);
    }
    FeeFactor::try_from(scaled / Decimal::from(GRID))
}

/// Why a number is not a fee factor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FactorError {
    /// Not a whole number of steps of 1e-6 of the volume.
    #[error("is finer than the fee precision (1e-6 of the volume)")]
    OffGrid,
    /// Negative, 100% of the volume or more, or not a finite number.
    #[error("is out of range")]
    OutOfRange,
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
        map2(share, max, move |share, max| {
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

/// A fee argument as a factor on the grid; the error names the argument.
fn factor(
    kind: FeeKind,
    argument: &'static str,
    value: Decimal,
    grid_units: u64,
) -> Result<FeeFactor, RuleError> {
    factor_on_grid(value, grid_units).map_err(|error| RuleError::Fee {
        kind,
        argument,
        value,
        error,
    })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{AppData, OrderView, RuleDefinition, RuleSet, RuleSetDefinition},
        chrono::Utc,
        model::order::Order,
    };

    /// The fees `source` gives a default order, out of market. A constant
    /// fee fails when the rules compile, one that reads the order when it
    /// runs; the error is the message either way.
    fn evaluate(source: &str) -> Result<Vec<Fee>, String> {
        let rules = RuleSet::new(RuleSetDefinition {
            rules: vec![RuleDefinition {
                name: "x".to_owned(),
                when: "true".to_owned(),
                fee: source.to_owned(),
            }],
            ..Default::default()
        })
        .map_err(|err| err.to_string())?;
        let order = Order::default();
        let mut evaluation = rules.evaluator(Utc::now()).evaluate(OrderView {
            order: &order,
            app_data: &AppData::default(),
            partner_fees: &[],
            in_market: false,
        });
        match evaluation.skipped.pop() {
            Some(skipped) => Err(skipped.error.to_string()),
            None => Ok(evaluation.matched.remove(0).fees),
        }
    }

    fn factor(value: f64) -> FeeFactor {
        FeeFactor::try_from(value).unwrap()
    }

    fn steps(factor: FeeFactor) -> Decimal {
        factor.get() * Decimal::from(GRID)
    }

    #[test]
    fn builds_each_fee_kind() {
        assert_eq!(
            evaluate("volume(10)").unwrap(),
            vec![Fee::Volume {
                factor: factor(0.001)
            }]
        );
        assert_eq!(
            evaluate("surplus(50, 100)").unwrap(),
            vec![Fee::Surplus {
                factor: factor(0.5),
                max_volume_factor: factor(0.01)
            }]
        );
        assert_eq!(
            evaluate("price_improvement(30, 50)").unwrap(),
            vec![Fee::PriceImprovement {
                factor: factor(0.3),
                max_volume_factor: factor(0.005)
            }]
        );
        assert_eq!(evaluate("no_fee()").unwrap(), vec![]);
    }

    #[test]
    fn stacks_a_list_and_computes_arguments() {
        assert_eq!(
            evaluate("[surplus(30, 100), volume(1 + 1)]").unwrap(),
            vec![
                Fee::Surplus {
                    factor: factor(0.3),
                    max_volume_factor: factor(0.01)
                },
                Fee::Volume {
                    factor: factor(0.0002)
                }
            ]
        );
    }

    #[test]
    fn reads_arguments_on_the_high_precision_grid() {
        let volume = |source: &str| match evaluate(source).unwrap()[..] {
            [Fee::Volume { factor }] => factor,
            _ => panic!("expected one volume fee"),
        };
        assert_ne!(0.3 / 10_000., 0.00003);
        assert_eq!(volume("volume(0.3)"), factor(0.00003));
        assert_eq!(steps(volume("volume(0.3)")), Decimal::from(30));
        assert_eq!(steps(volume("volume(0.33)")), Decimal::from(33));
        assert_eq!(
            evaluate("surplus(50, 98)").unwrap(),
            vec![Fee::Surplus {
                factor: factor(0.5),
                max_volume_factor: factor(0.0098)
            }]
        );
    }

    #[test]
    fn accepts_every_argument_down_to_the_grid() {
        assert_eq!(
            evaluate("price_improvement(50.5, 98.75)").unwrap(),
            vec![Fee::PriceImprovement {
                factor: factor(0.505),
                max_volume_factor: factor(0.009875)
            }]
        );
        assert_eq!(
            evaluate("surplus(0.0001, 0.01)").unwrap(),
            vec![Fee::Surplus {
                factor: factor(0.000001),
                max_volume_factor: factor(0.000001)
            }]
        );
    }

    #[test]
    fn rejects_arguments_finer_than_the_grid() {
        for source in [
            "volume(0.333)",
            "surplus(50.00001, 100)",
            "surplus(50, 98.765)",
            "price_improvement(0.00005, 100)",
            "volume(order.inMarket ? 1 : 0.333)",
        ] {
            let error = evaluate(source).unwrap_err();
            assert!(
                error.ends_with("is finer than the fee precision (1e-6 of the volume)"),
                "{source}: {error}"
            );
        }
    }

    #[test]
    fn rejects_values_that_are_not_fees() {
        for (source, message) in [
            ("volume('ten')", "expected a number, found a string"),
            ("volume(-1)", "is out of range"),
            ("volume(10000)", "is out of range"),
            ("volume(1e30)", "decimal"),
            ("42", "expected fees, found a number"),
            (
                "{'kind': 'volume'}",
                "map and struct literals are not supported",
            ),
            ("tip(1)", "unknown function `tip`"),
        ] {
            let error = evaluate(source).unwrap_err();
            assert!(error.contains(message), "{source}: {error}");
        }
        // A constant fee fails when the rules compile.
        assert_eq!(
            evaluate("volume(0.333)").unwrap_err(),
            "rule `x`: `fee` does not compile: `volume` argument `bps` = 0.333 is finer than the \
             fee precision (1e-6 of the volume)"
        );
        // One that reads the order fails on the order.
        assert_eq!(
            evaluate("volume(order.inMarket ? 1 : 0.333)").unwrap_err(),
            "`volume` argument `bps` = 0.333 is finer than the fee precision (1e-6 of the volume)"
        );
    }
}
