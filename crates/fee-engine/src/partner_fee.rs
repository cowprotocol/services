//! The partner fees the engine returns apart from the rule's fees.
//!
//! The cap is the autopilot's: `get_partner_fee` in `autopilot::domain::fee`
//! and the orderbook's fast-path check (`capped_partner_volume_factors` in
//! `shared::fee`) use [`PartnerFeeBudget`] from here. The fast-path check
//! spends no cap budget on surplus fees; [`partner_fees`] does, as the
//! autopilot does.

use {
    crate::{Fee, FeeFactor},
    rust_decimal::Decimal,
};

const MAX_BPS: u64 = 10_000;

/// A partner fee as the order's app data asks for it, in basis points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartnerFee {
    Volume { bps: u64 },
    Surplus { bps: u64, max_volume_bps: u64 },
    PriceImprovement { bps: u64, max_volume_bps: u64 },
}

/// The order's partner fees in app-data order, capped together at `cap` of
/// the volume. A fee past the cap stays, with a zero volume share or volume
/// cap.
pub(crate) fn partner_fees(fees: &[PartnerFee], cap: FeeFactor) -> Vec<Fee> {
    let mut budget = PartnerFeeBudget::new(cap);
    let mut volume_share =
        |bps: u64| budget.capped_fee_factor(Decimal::from(bps) / Decimal::from(MAX_BPS));
    fees.iter()
        .map(|fee| match *fee {
            PartnerFee::Volume { bps } => Fee::Volume {
                factor: volume_share(bps),
            },
            PartnerFee::Surplus {
                bps,
                max_volume_bps,
            } => Fee::Surplus {
                factor: share(bps),
                max_volume_factor: volume_share(max_volume_bps),
            },
            PartnerFee::PriceImprovement {
                bps,
                max_volume_bps,
            } => Fee::PriceImprovement {
                factor: share(bps),
                max_volume_factor: volume_share(max_volume_bps),
            },
        })
        .collect()
}

/// The share of the volume an order's partner fees may still take, spent fee
/// by fee in app-data order.
#[derive(Debug)]
pub struct PartnerFeeBudget {
    cap: FeeFactor,
    /// In [0, `cap`].
    accumulated: Decimal,
}

impl PartnerFeeBudget {
    pub fn new(cap: FeeFactor) -> Self {
        Self {
            cap,
            accumulated: Decimal::ZERO,
        }
    }

    /// Caps `value` to the headroom `(1 + cap) / (1 + accumulated) - 1` the
    /// fees before it left and spends it. With three or more fees the
    /// compounded total can pass `cap` slightly. A negative `value` is zero.
    pub fn capped_fee_factor(&mut self, value: Decimal) -> FeeFactor {
        let cap = self.cap.get();
        // Clamped before it is spent too, so a negative value cannot lower
        // `accumulated` and raise the headroom past the cap.
        let value = value.max(Decimal::ZERO);
        let remaining = (Decimal::ONE + cap) / (Decimal::ONE + self.accumulated) - Decimal::ONE;
        self.accumulated += value.min(cap - self.accumulated);
        FeeFactor::try_from(value.min(remaining))
            .expect("in [0, cap] because accumulated is in [0, cap]")
    }
}

/// A share of the surplus or improvement, just under 100% at most.
fn share(bps: u64) -> FeeFactor {
    FeeFactor::try_from(Decimal::from(bps.min(MAX_BPS - 1)) / Decimal::from(MAX_BPS))
        .expect("under 100%")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn factor(value: f64) -> FeeFactor {
        FeeFactor::try_from(value).unwrap()
    }

    fn volume(factor: f64) -> Fee {
        Fee::Volume {
            factor: self::factor(factor),
        }
    }

    /// The fees with their factors as doubles, as the autopilot computes them.
    fn as_f64(fees: &[Fee]) -> Vec<Fee> {
        let round = |factor: FeeFactor| self::factor(factor.to_f64());
        fees.iter()
            .map(|fee| match *fee {
                Fee::Volume { factor } => Fee::Volume {
                    factor: round(factor),
                },
                Fee::Surplus {
                    factor,
                    max_volume_factor,
                } => Fee::Surplus {
                    factor: round(factor),
                    max_volume_factor: round(max_volume_factor),
                },
                Fee::PriceImprovement {
                    factor,
                    max_volume_factor,
                } => Fee::PriceImprovement {
                    factor: round(factor),
                    max_volume_factor: round(max_volume_factor),
                },
            })
            .collect()
    }

    #[test]
    fn matches_the_autopilot_volume_fee_tests() {
        let cases: [(&[u64], f64, &[f64]); 8] = [
            (&[500, 2000], 0.3, &[0.05, 0.2]),
            (&[1000, 2500], 0.3, &[0.1, 0.18181818181818182]),
            (&[1000, 2000, 1500], 0.3, &[0.1, 0.18181818181818182, 0.0]),
            (&[5000], 0.3, &[0.3]),
            (&[20_000], 0.3, &[0.3]),
            (&[0], 0.3, &[0.0]),
            (&[1000, 2000], 0.0, &[0.0, 0.0]),
            // The compounded total passes the cap slightly.
            (&[40, 40, 40], 0.01, &[0.004, 0.004, 0.001984126984126984]),
        ];
        for (bps, cap, expected) in cases {
            let fees: Vec<_> = bps.iter().map(|&bps| PartnerFee::Volume { bps }).collect();
            let expected: Vec<_> = expected.iter().copied().map(volume).collect();
            assert_eq!(
                as_f64(&partner_fees(&fees, factor(cap))),
                expected,
                "{bps:?} at {cap}"
            );
        }
    }

    #[test]
    fn surplus_and_price_improvement_take_from_the_budget() {
        let fees = partner_fees(
            &[
                PartnerFee::Surplus {
                    bps: 20_000,
                    max_volume_bps: 100,
                },
                PartnerFee::Volume { bps: 50 },
                PartnerFee::PriceImprovement {
                    bps: 3000,
                    max_volume_bps: 10,
                },
            ],
            factor(0.01),
        );
        assert_eq!(
            fees,
            [
                Fee::Surplus {
                    factor: factor(0.9999),
                    max_volume_factor: factor(0.01),
                },
                volume(0.0),
                Fee::PriceImprovement {
                    factor: factor(0.3),
                    max_volume_factor: factor(0.0),
                },
            ]
        );
    }

    #[test]
    fn a_negative_value_spends_nothing() {
        let mut budget = PartnerFeeBudget::new(factor(0.01));
        assert_eq!(budget.capped_fee_factor(Decimal::from(-1)), FeeFactor::ZERO);
        assert_eq!(budget.capped_fee_factor(Decimal::from(2)), factor(0.01));
        assert_eq!(budget.capped_fee_factor(Decimal::from(2)), FeeFactor::ZERO);
    }
}
