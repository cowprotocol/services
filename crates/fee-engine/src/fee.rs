//! Fees a rule returns and the CEL functions that build them.

use {
    cel::{
        Context,
        Value,
        objects::{KeyRef, Map, ValueType},
    },
    configs::fee_factor::FeeFactor,
    rust_decimal::{
        Decimal,
        prelude::{FromPrimitive, ToPrimitive},
    },
    std::{collections::HashMap, fmt, str::FromStr},
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

/// The kinds of [`Fee`], by the CEL function that builds each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeeKind {
    Volume,
    Surplus,
    PriceImprovement,
}

impl FeeKind {
    const ALL: [FeeKind; 3] = [FeeKind::Volume, FeeKind::Surplus, FeeKind::PriceImprovement];

    /// The name the function is bound to in CEL.
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

/// Parses the CEL function name; the error is the unknown name.
impl FromStr for FeeKind {
    type Err = String;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.name() == name)
            .ok_or_else(|| name.to_owned())
    }
}

/// A fee factor is a whole number of steps of 1e-6 of the volume (the "grid",
/// [`FeeFactor::HIGH_PRECISION_SCALE`] steps per 1). Steps per basis point.
pub(crate) const BPS: u64 = FeeFactor::HIGH_PRECISION_SCALE / 10_000;
/// Steps per percent.
pub(crate) const PERCENT: u64 = FeeFactor::HIGH_PRECISION_SCALE / 100;

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
    scaled
        .to_u64()
        .and_then(|units| FeeFactor::from_high_precision(units).ok())
        .ok_or(FactorError::OutOfRange)
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

/// An argument of a fee function: its name and the unit the rule writes it
/// in.
#[derive(Debug, Clone, Copy)]
struct Argument {
    name: &'static str,
    grid_units: u64,
}

impl Argument {
    const BPS: Self = Self {
        name: "bps",
        grid_units: BPS,
    };
    const MAX_VOLUME: Self = Self {
        name: "max_volume",
        grid_units: PERCENT,
    };
    const SHARE: Self = Self {
        name: "share",
        grid_units: PERCENT,
    };
}

/// The map key a fee function tags its return value with.
const KIND: &str = "kind";

/// Binds the fee functions. Each returns a tagged map that
/// [`fees_from_value`] turns into a [`Fee`] after the rule has run.
pub(crate) fn register_fee_functions(context: &mut Context) {
    context
        .add_function(FeeKind::Volume.name(), |bps: Value| {
            fee_value(FeeKind::Volume, [(Argument::BPS, bps)])
        })
        .expect("fee function names are free");
    context
        .add_function(
            FeeKind::Surplus.name(),
            |share: Value, max_volume: Value| {
                fee_value(
                    FeeKind::Surplus,
                    [(Argument::SHARE, share), (Argument::MAX_VOLUME, max_volume)],
                )
            },
        )
        .expect("fee function names are free");
    context
        .add_function(
            FeeKind::PriceImprovement.name(),
            |share: Value, max_volume: Value| {
                fee_value(
                    FeeKind::PriceImprovement,
                    [(Argument::SHARE, share), (Argument::MAX_VOLUME, max_volume)],
                )
            },
        )
        .expect("fee function names are free");
    context
        .add_function("no_fee", || Value::List(Default::default()))
        .expect("fee function names are free");
}

/// The CEL value a fee function returns: a map tagged with the fee `kind`
/// plus one entry per argument. Fee functions return plain CEL data; typing
/// and range checks happen in [`Arguments::factor`] once the rule has run.
fn fee_value<const N: usize>(kind: FeeKind, args: [(Argument, Value); N]) -> Value {
    let mut map: HashMap<String, Value> = HashMap::with_capacity(N + 1);
    map.insert(KIND.to_owned(), Value::from(kind.name()));
    for (argument, value) in args {
        map.insert(argument.name.to_owned(), value);
    }
    Value::from(map)
}

/// The fees a `fee` expression evaluated to: one, or a list to stack.
pub(crate) fn fees_from_value(value: &Value) -> Result<Vec<Fee>, FeeError> {
    match value {
        Value::List(items) => items.iter().map(fee_from_value).collect(),
        single => Ok(vec![fee_from_value(single)?]),
    }
}

/// Turns the tagged map a fee function returned (see [`fee_value`]) into a
/// [`Fee`]: reads `kind`, then each argument as a decimal scaled by its unit
/// (bps or percent) onto the high precision grid.
fn fee_from_value(value: &Value) -> Result<Fee, FeeError> {
    let Value::Map(map) = value else {
        return Err(FeeError::NotAFee(value.type_of()));
    };
    let Some(Value::String(kind)) = map.get(&KeyRef::String(KIND)) else {
        return Err(FeeError::NotAFee(ValueType::Map));
    };
    let kind = kind.parse().map_err(FeeError::UnknownKind)?;
    let args = Arguments { kind, map };
    match kind {
        FeeKind::Volume => Fee::volume(args),
        FeeKind::Surplus => Fee::surplus(args),
        FeeKind::PriceImprovement => Fee::price_improvement(args),
    }
}

impl Fee {
    fn volume(args: Arguments<'_>) -> Result<Self, FeeError> {
        Ok(Fee::Volume {
            factor: args.factor(Argument::BPS)?,
        })
    }

    fn surplus(args: Arguments<'_>) -> Result<Self, FeeError> {
        Ok(Fee::Surplus {
            factor: args.factor(Argument::SHARE)?,
            max_volume_factor: args.factor(Argument::MAX_VOLUME)?,
        })
    }

    fn price_improvement(args: Arguments<'_>) -> Result<Self, FeeError> {
        Ok(Fee::PriceImprovement {
            factor: args.factor(Argument::SHARE)?,
            max_volume_factor: args.factor(Argument::MAX_VOLUME)?,
        })
    }
}

/// The arguments of one fee function, read off the map it returned.
struct Arguments<'a> {
    kind: FeeKind,
    map: &'a Map,
}

impl Arguments<'_> {
    /// Reads `argument` as a fee factor.
    fn factor(&self, argument: Argument) -> Result<FeeFactor, FeeError> {
        let kind = self.kind;
        let Argument {
            name: argument,
            grid_units,
        } = argument;
        let value = self
            .map
            .get(&KeyRef::String(argument))
            .ok_or(FeeError::MissingArgument { kind, argument })?;
        // `from_f64` recovers the decimal the rule spelled out (0.3, not the
        // nearest binary fraction), so the grid check below is exact.
        let number = match value {
            Value::Int(int) => Decimal::from(*int),
            Value::UInt(uint) => Decimal::from(*uint),
            Value::Float(float) => Decimal::from_f64(*float).ok_or(FeeError::NotFinite {
                kind,
                argument,
                value: *float,
            })?,
            other => {
                return Err(FeeError::NotANumber {
                    kind,
                    argument,
                    found: other.type_of(),
                });
            }
        };
        factor_on_grid(number, grid_units).map_err(|error| FeeError::Factor {
            kind,
            argument,
            value: number,
            error,
        })
    }
}

/// Why the value a `fee` expression returned is not a fee.
#[derive(Debug, Clone, thiserror::Error)]
pub enum FeeError {
    #[error("returned {0} instead of a fee such as volume(10)")]
    NotAFee(ValueType),
    #[error("unknown fee kind `{0}`")]
    UnknownKind(String),
    #[error("`{kind}` is missing `{argument}`")]
    MissingArgument {
        kind: FeeKind,
        argument: &'static str,
    },
    #[error("`{kind}` argument `{argument}` is {found} instead of a number")]
    NotANumber {
        kind: FeeKind,
        argument: &'static str,
        found: ValueType,
    },
    #[error("`{kind}` argument `{argument}` = {value} is not a finite number")]
    NotFinite {
        kind: FeeKind,
        argument: &'static str,
        value: f64,
    },
    #[error("`{kind}` argument `{argument}` = {value} {error}")]
    Factor {
        kind: FeeKind,
        argument: &'static str,
        value: Decimal,
        error: FactorError,
    },
}

#[cfg(test)]
mod tests {
    use {super::*, cel::Env, std::sync::Arc};

    fn evaluate(source: &str) -> Result<Vec<Fee>, FeeError> {
        let env = Arc::new(Env::stdlib());
        let program = env.compile(source).unwrap();
        let mut context = Context::with_env(env);
        register_fee_functions(&mut context);
        fees_from_value(&program.execute(&context).unwrap())
    }

    fn factor(value: f64) -> FeeFactor {
        FeeFactor::try_from(value).unwrap()
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
            evaluate("surplus(50, 1)").unwrap(),
            vec![Fee::Surplus {
                factor: factor(0.5),
                max_volume_factor: factor(0.01)
            }]
        );
        assert_eq!(
            evaluate("price_improvement(30, 0.5)").unwrap(),
            vec![Fee::PriceImprovement {
                factor: factor(0.3),
                max_volume_factor: factor(0.005)
            }]
        );
        assert_eq!(evaluate("no_fee()").unwrap(), vec![]);
    }

    #[test]
    fn stacks_a_list_and_accepts_cel_arguments() {
        assert_eq!(
            evaluate("[surplus(30, 1), volume(1 + 1)]").unwrap(),
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
        assert_eq!(volume("volume(0.3)").get(), 0.00003);
        assert_eq!(volume("volume(0.3)").to_high_precision(), 30);
        assert_eq!(volume("volume(0.33)").to_high_precision(), 33);
        assert_eq!(
            evaluate("surplus(50, 0.98)").unwrap(),
            vec![Fee::Surplus {
                factor: factor(0.5),
                max_volume_factor: factor(0.0098)
            }]
        );
    }

    #[test]
    fn accepts_every_argument_down_to_the_grid() {
        assert_eq!(
            evaluate("price_improvement(50.5, 0.9875)").unwrap(),
            vec![Fee::PriceImprovement {
                factor: factor(0.505),
                max_volume_factor: factor(0.009875)
            }]
        );
        assert_eq!(
            evaluate("surplus(0.0001, 0.0001)").unwrap(),
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
            "surplus(50.00001, 1)",
            "surplus(50, 0.98765)",
            "price_improvement(0.00005, 1)",
        ] {
            assert!(
                matches!(
                    evaluate(source).unwrap_err(),
                    FeeError::Factor {
                        error: FactorError::OffGrid,
                        ..
                    }
                ),
                "{source}"
            );
        }
    }

    #[test]
    fn rejects_values_that_are_not_fees() {
        assert!(matches!(
            evaluate("volume('ten')").unwrap_err(),
            FeeError::NotANumber {
                kind: FeeKind::Volume,
                argument: "bps",
                found: ValueType::String,
            }
        ));
        for source in ["volume(-1)", "volume(10000)"] {
            assert!(
                matches!(
                    evaluate(source).unwrap_err(),
                    FeeError::Factor {
                        error: FactorError::OutOfRange,
                        ..
                    }
                ),
                "{source}"
            );
        }
        assert!(matches!(
            evaluate("42").unwrap_err(),
            FeeError::NotAFee(ValueType::Int)
        ));
        assert!(matches!(
            evaluate("{'kind': 'volume'}").unwrap_err(),
            FeeError::MissingArgument {
                kind: FeeKind::Volume,
                argument: "bps",
            }
        ));
        assert!(matches!(
            evaluate("{'kind': 'tip'}").unwrap_err(),
            FeeError::UnknownKind(kind) if kind == "tip"
        ));
        assert_eq!(
            evaluate("volume(0.333)").unwrap_err().to_string(),
            "`volume` argument `bps` = 0.333 is finer than the fee precision (1e-6 of the volume)"
        );
    }
}
