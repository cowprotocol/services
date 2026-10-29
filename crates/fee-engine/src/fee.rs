//! Fees a rule returns and the CEL functions that build them.

use {
    cel::{Context, Value, objects::KeyRef},
    configs::fee_factor::FeeFactor,
    rust_decimal::{
        Decimal,
        prelude::{FromPrimitive, ToPrimitive},
    },
    std::collections::HashMap,
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

const KIND: &str = "kind";

/// Grid units per basis point. Every argument must land on the
/// [`FeeFactor::HIGH_PRECISION_SCALE`] grid; finer values are rejected, not
/// rounded.
const BPS: u64 = FeeFactor::HIGH_PRECISION_SCALE / 10_000;
/// Grid units per percent.
pub(crate) const PERCENT: u64 = FeeFactor::HIGH_PRECISION_SCALE / 100;

/// Binds the fee functions. Each returns a tagged map that
/// [`fees_from_value`] turns into a [`Fee`] after the rule has run.
pub(crate) fn register_fee_functions(context: &mut Context) {
    context
        .add_function("volume", |bps: Value| fee_value("volume", [("bps", bps)]))
        .expect("fee function names are free");
    context
        .add_function("surplus", |share: Value, max_volume: Value| {
            fee_value("surplus", [("share", share), ("max_volume", max_volume)])
        })
        .expect("fee function names are free");
    context
        .add_function("price_improvement", |share: Value, max_volume: Value| {
            fee_value(
                "price_improvement",
                [("share", share), ("max_volume", max_volume)],
            )
        })
        .expect("fee function names are free");
    context
        .add_function("no_fee", || Value::List(Default::default()))
        .expect("fee function names are free");
}

/// The CEL value a fee function returns: a map tagged with the fee `kind`
/// plus one entry per argument. Fee functions return plain CEL data; typing
/// and range checks happen in [`fee_from_value`] once the rule has run.
fn fee_value<const N: usize>(kind: &str, args: [(&str, Value); N]) -> Value {
    let mut map: HashMap<String, Value> = HashMap::with_capacity(N + 1);
    map.insert(KIND.to_owned(), Value::from(kind));
    for (name, value) in args {
        map.insert(name.to_owned(), value);
    }
    Value::from(map)
}

/// The fees a `fee` expression evaluated to: one, or a list to stack.
pub(crate) fn fees_from_value(value: &Value) -> Result<Vec<Fee>, String> {
    match value {
        Value::List(items) => items.iter().map(fee_from_value).collect(),
        single => Ok(vec![fee_from_value(single)?]),
    }
}

/// Turns the tagged map a fee function returned (see [`fee_value`]) into a
/// [`Fee`]: reads `kind`, then each argument as a decimal scaled by its unit
/// (bps or percent) onto the high precision grid. Fails with a message naming
/// what is wrong.
fn fee_from_value(value: &Value) -> Result<Fee, String> {
    let Value::Map(map) = value else {
        return Err(format!(
            "returned {} instead of a fee such as volume(10)",
            value.type_of()
        ));
    };
    let field = |name: &str| map.get(&KeyRef::String(name));
    let Some(Value::String(kind)) = field(KIND) else {
        return Err("returned a map instead of a fee such as volume(10)".to_owned());
    };
    let factor = |name: &str, grid_units: u64| -> Result<FeeFactor, String> {
        let value = field(name).ok_or_else(|| format!("`{kind}` is missing `{name}`"))?;
        let out_of_range = || format!("`{kind}` argument `{name}` = {value:?} is out of range");
        // `from_f64` recovers the decimal the rule spelled out (0.3, not the
        // nearest binary fraction), so the grid check below is exact.
        let number = match value {
            Value::Int(int) => Decimal::from(*int),
            Value::UInt(uint) => Decimal::from(*uint),
            Value::Float(float) => Decimal::from_f64(*float).ok_or_else(out_of_range)?,
            other => {
                return Err(format!(
                    "`{kind}` argument `{name}` is {} instead of a number",
                    other.type_of()
                ));
            }
        };
        factor_on_grid(number, grid_units)
            .map_err(|error| format!("`{kind}` argument `{name}` = {number} {error}"))
    };
    match kind.as_str() {
        "volume" => Ok(Fee::Volume {
            factor: factor("bps", BPS)?,
        }),
        "surplus" => Ok(Fee::Surplus {
            factor: factor("share", PERCENT)?,
            max_volume_factor: factor("max_volume", PERCENT)?,
        }),
        "price_improvement" => Ok(Fee::PriceImprovement {
            factor: factor("share", PERCENT)?,
            max_volume_factor: factor("max_volume", PERCENT)?,
        }),
        other => Err(format!("unknown fee kind `{other}`")),
    }
}

/// `number` steps of `grid_units` each (e.g. [`PERCENT`]) as a fee factor.
pub(crate) fn factor_on_grid(number: Decimal, grid_units: u64) -> Result<FeeFactor, &'static str> {
    let scaled = number
        .checked_mul(Decimal::from(grid_units))
        .ok_or(OUT_OF_RANGE)?;
    if !scaled.is_integer() {
        return Err("is finer than the fee precision (1e-6 of the volume)");
    }
    scaled
        .to_u64()
        .and_then(|units| FeeFactor::from_high_precision(units).ok())
        .ok_or(OUT_OF_RANGE)
}

/// Negative, 100% of the volume or more, or not a finite number.
pub(crate) const OUT_OF_RANGE: &str = "is out of range";

#[cfg(test)]
mod tests {
    use {super::*, cel::Env, std::sync::Arc};

    fn evaluate(source: &str) -> Result<Vec<Fee>, String> {
        let env = Arc::new(Env::stdlib());
        let program = env.compile(source).unwrap();
        let mut context = Context::with_env(env);
        register_fee_functions(&mut context);
        let value = program.execute(&context).map_err(|err| err.to_string())?;
        fees_from_value(&value)
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
                evaluate(source).unwrap_err().contains("finer than the fee"),
                "{source}"
            );
        }
    }

    #[test]
    fn rejects_values_that_are_not_fees() {
        assert!(
            evaluate("volume('ten')")
                .unwrap_err()
                .contains("instead of a number")
        );
        assert!(evaluate("volume(-1)").unwrap_err().contains("out of range"));
        assert!(
            evaluate("volume(10000)")
                .unwrap_err()
                .contains("out of range")
        );
        assert!(evaluate("42").unwrap_err().contains("instead of a fee"));
        assert!(
            evaluate("{'kind': 'volume'}")
                .unwrap_err()
                .contains("missing `bps`")
        );
    }
}
