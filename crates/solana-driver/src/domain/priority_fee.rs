//! The priority fee the driver attaches to a transaction.
//!
//! As with the EVM driver's gas price, the driver owns the fee and solvers
//! never set it: the compute unit price is the configured percentile of the
//! fees recently paid over the transaction's writable accounts, raised to a
//! floor, and a transaction whose total priority fee is over the configured
//! budget is refused.

use {
    cow_solana_rpc::RpcPrioritizationFee,
    itertools::Itertools,
    serde::Deserialize,
    std::cmp::Reverse,
};

/// The runtime's per-transaction compute unit ceiling, which prices a
/// transaction that declares no limit. The runtime's own default for such a
/// transaction is lower (200k units per non-builtin instruction and 3k per
/// builtin, up to this ceiling), so this is the conservative pick.
const MAX_COMPUTE_UNIT_LIMIT: u32 = 1_400_000;

/// The slots `getRecentPrioritizationFees` serves.
pub(crate) const MAX_RECENT_SLOTS: usize = 150;

/// Headroom over the simulated compute units, in percent, for the state that
/// moves between simulation and inclusion.
const SIMULATION_MARGIN_PERCENT: u128 = 30;

/// The compute unit limit for a transaction whose simulation consumed
/// `units`: the usage plus margin, at most the runtime's ceiling. Zero units
/// measured nothing, so no limit.
pub(crate) fn limit_from_simulation(units: u64) -> Option<u32> {
    if units == 0 {
        return None;
    }
    let limit = u128::from(units) * (100 + SIMULATION_MARGIN_PERCENT) / 100;
    Some(limit.min(u128::from(MAX_COMPUTE_UNIT_LIMIT)) as u32)
}

/// The priority fee policy. Fields left out of the config take their
/// [`Default`] value.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields, default)]
pub struct PriorityFeePolicy {
    /// The percentile of the recent per-slot fees to use as the compute unit
    /// price, `0..=100`.
    pub percentile: u8,
    /// How many of the most recent slots the percentile ranges over,
    /// `1..=150`.
    pub recent_slots: usize,
    /// The lowest compute unit price, in micro-lamports per compute unit.
    pub min_compute_unit_price: u64,
    /// The most a transaction pays in priority fee, in lamports. A transaction
    /// over it is refused.
    pub max_priority_fee_lamports: u64,
}

impl Default for PriorityFeePolicy {
    fn default() -> Self {
        Self {
            percentile: 75,
            recent_slots: 50,
            min_compute_unit_price: 10_000,
            max_priority_fee_lamports: 1_000_000,
        }
    }
}

/// A transaction's estimated priority fee.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Estimate {
    /// Micro-lamports per compute unit.
    pub(crate) compute_unit_price: u64,
    /// The priority fee the transaction pays at that price, in lamports.
    pub(crate) lamports: u128,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error(
    "priority fee of {lamports} lamports is over the {cap} lamport budget (network is too \
     congested)"
)]
pub(crate) struct OverBudget {
    lamports: u128,
    cap: u64,
}

impl PriorityFeePolicy {
    /// The priority fee for a transaction given the fees recently paid over its
    /// writable accounts and its compute unit limit, or the fee it would pay
    /// when that is over the budget. An undeclared limit is priced at the
    /// runtime's ceiling.
    pub(crate) fn estimate(
        &self,
        fees: &[RpcPrioritizationFee],
        compute_unit_limit: Option<u32>,
    ) -> Result<Estimate, OverBudget> {
        let compute_unit_price = self.compute_unit_price(fees);
        let limit = compute_unit_limit.unwrap_or(MAX_COMPUTE_UNIT_LIMIT);
        // Micro-lamports per unit times units, rounded up to whole lamports.
        let lamports = (u128::from(compute_unit_price) * u128::from(limit)).div_ceil(1_000_000);
        if lamports > u128::from(self.max_priority_fee_lamports) {
            return Err(OverBudget {
                lamports,
                cap: self.max_priority_fee_lamports,
            });
        }
        Ok(Estimate {
            compute_unit_price,
            lamports,
        })
    }

    /// The configured percentile of the most recent slots' fees, raised to the
    /// floor. No fees at all give the floor.
    fn compute_unit_price(&self, fees: &[RpcPrioritizationFee]) -> u64 {
        let prices: Vec<u64> = fees
            .iter()
            .sorted_unstable_by_key(|fee| Reverse(fee.slot))
            .take(self.recent_slots)
            .map(|fee| fee.prioritization_fee)
            .sorted_unstable()
            .collect();
        percentile(&prices, self.percentile)
            .unwrap_or(0)
            .max(self.min_compute_unit_price)
    }
}

/// The nearest-rank `p`-th percentile of an ascending sample: the smallest
/// value with at least `p` percent of the sample at or below it, so 0 is the
/// minimum and 100 the maximum. `None` for an empty sample.
fn percentile(sorted: &[u64], p: u8) -> Option<u64> {
    let rank = (sorted.len() * usize::from(p)).div_ceil(100);
    sorted.get(rank.saturating_sub(1)).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fee(slot: u64, prioritization_fee: u64) -> RpcPrioritizationFee {
        RpcPrioritizationFee {
            slot,
            prioritization_fee,
        }
    }

    fn policy(percentile: u8) -> PriorityFeePolicy {
        PriorityFeePolicy {
            percentile,
            recent_slots: MAX_RECENT_SLOTS,
            min_compute_unit_price: 0,
            max_priority_fee_lamports: u64::MAX,
        }
    }

    /// Only the most recent slots count, whatever order the node lists them
    /// in.
    #[test]
    fn only_the_most_recent_slots_count() {
        let fees = [fee(1, 1_000), fee(3, 30), fee(2, 1_000), fee(4, 40)];
        let policy = PriorityFeePolicy {
            recent_slots: 2,
            ..policy(100)
        };
        assert_eq!(policy.compute_unit_price(&fees), 40);
        let policy = PriorityFeePolicy {
            percentile: 0,
            ..policy
        };
        assert_eq!(policy.compute_unit_price(&fees), 30);
    }

    #[test]
    fn price_is_raised_to_the_floor() {
        let policy = PriorityFeePolicy {
            min_compute_unit_price: 100,
            ..policy(50)
        };
        assert_eq!(policy.compute_unit_price(&[]), 100);
        assert_eq!(policy.compute_unit_price(&[fee(1, 5)]), 100);
        assert_eq!(policy.compute_unit_price(&[fee(1, 150)]), 150);
    }

    /// 1_500 micro-lamports over 200_001 units is 300.0015 lamports, and 1
    /// micro-lamport over 1 unit is 0.000001: both round up.
    #[test]
    fn lamports_round_up() {
        let estimate = policy(50)
            .estimate(&[fee(1, 1_500)], Some(200_001))
            .unwrap();
        assert_eq!(estimate.lamports, 301);
        let estimate = policy(50).estimate(&[fee(1, 1)], Some(1)).unwrap();
        assert_eq!(estimate.lamports, 1);
    }

    /// An undeclared limit is priced at the 1.4M ceiling.
    #[test]
    fn undeclared_limit_is_priced_at_the_ceiling() {
        let estimate = policy(50).estimate(&[fee(1, 1_000)], None).unwrap();
        assert_eq!(estimate.lamports, 1_400);
    }

    #[test]
    fn a_fee_over_budget_is_refused() {
        let policy = PriorityFeePolicy {
            max_priority_fee_lamports: 299,
            ..policy(50)
        };
        assert_eq!(
            policy.estimate(&[fee(1, 1_500)], Some(200_000)),
            Err(OverBudget {
                lamports: 300,
                cap: 299
            })
        );
        let policy = PriorityFeePolicy {
            max_priority_fee_lamports: 300,
            ..policy
        };
        assert!(policy.estimate(&[fee(1, 1_500)], Some(200_000)).is_ok());
    }

    #[test]
    fn simulated_units_get_a_margin_up_to_the_ceiling() {
        assert_eq!(limit_from_simulation(100_000), Some(130_000));
        assert_eq!(limit_from_simulation(1_200_000), Some(1_400_000));
        assert_eq!(limit_from_simulation(u64::MAX), Some(1_400_000));
        assert_eq!(limit_from_simulation(0), None);
    }

    #[test]
    fn percentile_of_an_empty_sample_is_none() {
        for p in 0..=100 {
            assert_eq!(percentile(&[], p), None, "percentile {p}");
        }
    }

    #[test]
    fn percentile_of_a_singleton_is_the_element() {
        for p in 0..=100 {
            assert_eq!(percentile(&[7], p), Some(7), "percentile {p}");
        }
    }

    /// In the sample 1..=100 the p-th percentile is p itself; 0 is the
    /// minimum.
    #[test]
    fn percentile_of_one_to_a_hundred_is_itself() {
        let sample: Vec<u64> = (1..=100).collect();
        assert_eq!(percentile(&sample, 0), Some(1));
        for p in 1..=100 {
            assert_eq!(percentile(&sample, p), Some(u64::from(p)), "percentile {p}");
        }
    }

    /// Four values step at the quarter marks. The rank rounds up, so 26
    /// already reaches the second value while 50 still does not reach the
    /// third.
    #[test]
    fn percentile_of_four_values_steps_at_the_quarters() {
        let sample = [10, 20, 30, 40];
        for p in 0..=100 {
            let expected = match p {
                0..=25 => 10,
                26..=50 => 20,
                51..=75 => 30,
                76..=100 => 40,
                _ => unreachable!(),
            };
            assert_eq!(percentile(&sample, p), Some(expected), "percentile {p}");
        }
    }
}
