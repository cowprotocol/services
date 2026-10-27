//! Fees a rule returns, the CEL functions that build them and the caps they
//! are clamped to.

use {
    cel::{Context, Value, objects::Key},
    configs::fee_factor::FeeFactor,
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

impl Fee {
    /// Lowers every component to its cap. Returns whether anything changed.
    pub fn clamp(&mut self, caps: &Caps) -> bool {
        match self {
            Fee::Volume { factor } => factor.cap(caps.volume_factor),
            Fee::Surplus {
                factor,
                max_volume_factor,
            }
            | Fee::PriceImprovement {
                factor,
                max_volume_factor,
            } => {
                // Evaluate both so neither clamp is skipped by
                // short-circuiting.
                let share = factor.cap(caps.share_factor);
                let volume = max_volume_factor.cap(caps.max_volume_factor);
                share || volume
            }
        }
    }
}

/// Upper bounds every fee is clamped to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Caps {
    /// Highest volume share a `volume()` fee charges.
    pub volume_factor: FeeFactor,
    /// Highest surplus or price-improvement share a fee takes.
    pub share_factor: FeeFactor,
    /// Highest volume share a surplus or price-improvement fee is capped at.
    pub max_volume_factor: FeeFactor,
}

const KIND: &str = "kind";
const BPS: f64 = 10_000.;
const PERCENT: f64 = 100.;

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
/// [`Fee`]: reads `kind`, then each argument as a number scaled by its unit
/// (bps or percent). Fails with a message naming what is wrong.
fn fee_from_value(value: &Value) -> Result<Fee, String> {
    let Value::Map(map) = value else {
        return Err(format!(
            "returned {} instead of a fee such as volume(10)",
            value.type_of()
        ));
    };
    let field = |name: &str| {
        map.map
            .iter()
            .find(|(key, _)| matches!(key, Key::String(key) if key.as_str() == name))
            .map(|(_, value)| value)
    };
    let Some(Value::String(kind)) = field(KIND) else {
        return Err("returned a map instead of a fee such as volume(10)".to_owned());
    };
    let factor = |name: &str, per_unit: f64| -> Result<FeeFactor, String> {
        let value = field(name).ok_or_else(|| format!("`{kind}` is missing `{name}`"))?;
        let number = match value {
            Value::Int(int) => *int as f64,
            Value::UInt(uint) => *uint as f64,
            Value::Float(float) => *float,
            other => {
                return Err(format!(
                    "`{kind}` argument `{name}` is {} instead of a number",
                    other.type_of()
                ));
            }
        };
        // Round to the grid `FeeFactor` compares on, so `volume(0.3)` is
        // exactly 3e-5 and not 2.9999999999999997e-05.
        let grid = FeeFactor::HIGH_PRECISION_SCALE as f64;
        let factor = (number / per_unit * grid).round() / grid;
        FeeFactor::try_from(factor)
            .map_err(|_| format!("`{kind}` argument `{name}` = {number} is out of range"))
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
    fn snaps_factors_to_the_high_precision_grid() {
        let [Fee::Volume { factor }] = evaluate("volume(0.3)").unwrap()[..] else {
            panic!("expected one volume fee");
        };
        assert_ne!(0.3 / 10_000., 0.00003);
        assert_eq!(factor.get(), 0.00003);
        assert_eq!(factor.to_high_precision(), 30);
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

    #[test]
    fn clamps_to_caps() {
        let caps = Caps {
            volume_factor: factor(0.01),
            share_factor: factor(0.9),
            max_volume_factor: factor(0.05),
        };
        let mut fee = Fee::Volume {
            factor: factor(0.5),
        };
        assert!(fee.clamp(&caps));
        assert_eq!(
            fee,
            Fee::Volume {
                factor: caps.volume_factor
            }
        );

        let mut fee = Fee::Surplus {
            factor: factor(0.5),
            max_volume_factor: factor(0.5),
        };
        assert!(fee.clamp(&caps));
        assert_eq!(
            fee,
            Fee::Surplus {
                factor: factor(0.5),
                max_volume_factor: caps.max_volume_factor
            }
        );
        assert!(!fee.clamp(&caps));
    }
}
