//! The partner fees the engine returns apart from the rule's fees.

use {crate::Fee, configs::fee_factor::FeeFactor, rust_decimal::Decimal};

const MAX_BPS: u64 = 10_000;

/// A partner fee as the order's app data asks for it, in basis points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartnerFee {
    Volume { bps: u64 },
    Surplus { bps: u64, max_volume_bps: u64 },
    PriceImprovement { bps: u64, max_volume_bps: u64 },
}

/// The order's partner fees in app-data order, capped as the autopilot's
/// `get_partner_fee` caps them. A fee past the cap stays, with a zero volume
/// share or volume cap.
pub(crate) fn partner_fees(fees: &[PartnerFee], cap: FeeFactor) -> Vec<Fee> {
    let Ok(cap) = Decimal::try_from(cap.get()) else {
        return vec![];
    };
    let mut accumulated = Decimal::ZERO;
    let mut volume_share = |bps: u64| {
        capped(
            Decimal::from(bps) / Decimal::from(MAX_BPS),
            cap,
            &mut accumulated,
        )
    };
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

/// Mirrors `shared::fee::capped_fee_factor`, quirks included: with three or
/// more fees the compounded total can pass `cap` slightly.
fn capped(requested: Decimal, cap: Decimal, accumulated: &mut Decimal) -> FeeFactor {
    let remaining = (Decimal::ONE + cap) / (Decimal::ONE + *accumulated) - Decimal::ONE;
    *accumulated += requested.min(cap - *accumulated);
    let factor = requested.max(Decimal::ZERO).min(remaining);
    FeeFactor::new(f64::try_from(factor).expect("a decimal in [0, 1) is a double"))
}

/// A share of the surplus or improvement, just under 100% at most.
fn share(bps: u64) -> FeeFactor {
    FeeFactor::new(bps.min(MAX_BPS - 1) as f64 / MAX_BPS as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn volume(factor: f64) -> Fee {
        Fee::Volume {
            factor: FeeFactor::new(factor),
        }
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
                partner_fees(&fees, FeeFactor::new(cap)),
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
            FeeFactor::new(0.01),
        );
        assert_eq!(
            fees,
            [
                Fee::Surplus {
                    factor: FeeFactor::new(0.9999),
                    max_volume_factor: FeeFactor::new(0.01),
                },
                volume(0.0),
                Fee::PriceImprovement {
                    factor: FeeFactor::new(0.3),
                    max_volume_factor: FeeFactor::new(0.0),
                },
            ]
        );
    }
}
