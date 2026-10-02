//! The priority fee the driver attaches to a transaction.
//!
//! As with the EVM driver's gas price, the driver owns the fee and solvers
//! never set it: the compute unit price is the configured percentile of the
//! fees recently paid over the transaction's writable accounts, clamped to a
//! price band, and a transaction whose total priority fee is over the
//! configured budget is refused.

use {
    cow_solana_rpc::RpcPrioritizationFee,
    itertools::Itertools,
    serde::Deserialize,
    std::cmp::Reverse,
};

/// The runtime's per-transaction compute unit ceiling, which prices a
/// transaction that declares no limit.
const MAX_COMPUTE_UNIT_LIMIT: u32 = 1_400_000;

/// The slots `getRecentPrioritizationFees` serves.
const MAX_RECENT_SLOTS: usize = 150;

/// The priority fee policy.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct PriorityFeePolicy {
    /// The percentile of the recent per-slot fees to use as the compute unit
    /// price.
    pub percentile: Percentile,
    /// How many of the most recent slots the percentile ranges over.
    pub recent_slots: RecentSlots,
    /// The lowest compute unit price, in micro-lamports per compute unit.
    pub min_compute_unit_price: u64,
    /// The highest compute unit price, in micro-lamports per compute unit.
    pub max_compute_unit_price: u64,
    /// The most a transaction pays in priority fee, in lamports. A transaction
    /// over it is refused.
    pub max_priority_fee_lamports: u64,
}

/// A percentile, `0..=100`.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(try_from = "u8")]
pub struct Percentile(u8);

#[derive(Debug, thiserror::Error)]
#[error("percentile must be at most 100, got {0}")]
pub struct PercentileOutOfRange(u8);

/// How many of the node's recent slots a percentile ranges over,
/// `1..=150`.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(try_from = "usize")]
pub struct RecentSlots(usize);

#[derive(Debug, thiserror::Error)]
#[error("recent slots must be 1..={MAX_RECENT_SLOTS}, got {0}")]
pub struct RecentSlotsOutOfRange(usize);

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

    /// The configured percentile of the most recent slots' fees, within the
    /// price band. No fees at all give the floor.
    fn compute_unit_price(&self, fees: &[RpcPrioritizationFee]) -> u64 {
        let prices: Vec<u64> = fees
            .iter()
            .sorted_unstable_by_key(|fee| Reverse(fee.slot))
            .take(self.recent_slots.0)
            .map(|fee| fee.prioritization_fee)
            .sorted_unstable()
            .collect();
        self.percentile
            .of(&prices)
            .unwrap_or(0)
            .clamp(self.min_compute_unit_price, self.max_compute_unit_price)
    }
}

impl Percentile {
    /// The nearest-rank percentile of an ascending sample: the smallest value
    /// with at least this share of the sample at or below it, so 0 is the
    /// minimum and 100 the maximum. `None` for an empty sample.
    fn of(self, sorted: &[u64]) -> Option<u64> {
        let rank = (sorted.len() * usize::from(self.0)).div_ceil(100);
        sorted.get(rank.saturating_sub(1)).copied()
    }
}

impl TryFrom<u8> for Percentile {
    type Error = PercentileOutOfRange;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        (value <= 100)
            .then_some(Self(value))
            .ok_or(PercentileOutOfRange(value))
    }
}

impl TryFrom<usize> for RecentSlots {
    type Error = RecentSlotsOutOfRange;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        (1..=MAX_RECENT_SLOTS)
            .contains(&value)
            .then_some(Self(value))
            .ok_or(RecentSlotsOutOfRange(value))
    }
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
            percentile: Percentile::try_from(percentile).unwrap(),
            recent_slots: RecentSlots(150),
            min_compute_unit_price: 0,
            max_compute_unit_price: u64::MAX,
            max_priority_fee_lamports: u64::MAX,
        }
    }

    /// Fees 10, 20, 30, 40 in slot order: 0 is the minimum, 100 the maximum,
    /// 50 the second of four (nearest rank), 51 the third.
    #[test]
    fn nearest_rank_percentile() {
        let fees = [fee(1, 10), fee(2, 20), fee(3, 30), fee(4, 40)];
        let price = |p: u8| policy(p).compute_unit_price(&fees);
        assert_eq!(price(0), 10);
        assert_eq!(price(50), 20);
        assert_eq!(price(51), 30);
        assert_eq!(price(100), 40);
    }

    /// Only the most recent slots count, whatever order the node lists them
    /// in.
    #[test]
    fn only_the_most_recent_slots_count() {
        let fees = [fee(1, 1_000), fee(3, 30), fee(2, 1_000), fee(4, 40)];
        let policy = PriorityFeePolicy {
            recent_slots: RecentSlots(2),
            ..policy(100)
        };
        assert_eq!(policy.compute_unit_price(&fees), 40);
        let policy = PriorityFeePolicy {
            percentile: Percentile::try_from(0).unwrap(),
            ..policy
        };
        assert_eq!(policy.compute_unit_price(&fees), 30);
    }

    #[test]
    fn price_is_clamped_to_the_band() {
        let policy = PriorityFeePolicy {
            min_compute_unit_price: 100,
            max_compute_unit_price: 200,
            ..policy(50)
        };
        assert_eq!(policy.compute_unit_price(&[]), 100);
        assert_eq!(policy.compute_unit_price(&[fee(1, 5)]), 100);
        assert_eq!(policy.compute_unit_price(&[fee(1, 150)]), 150);
        assert_eq!(policy.compute_unit_price(&[fee(1, 500)]), 200);
    }

    /// 1_500 micro-lamports over 200_000 units is 300 lamports, and 1
    /// micro-lamport over 1 unit rounds up to 1 lamport.
    #[test]
    fn lamports_round_up() {
        let estimate = policy(50)
            .estimate(&[fee(1, 1_500)], Some(200_000))
            .unwrap();
        assert_eq!(estimate.lamports, 300);
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
    fn percentile_over_100_is_rejected() {
        assert!(Percentile::try_from(101).is_err());
        assert!(serde_json::from_str::<Percentile>("101").is_err());
        assert_eq!(
            serde_json::from_str::<Percentile>("100").unwrap(),
            Percentile(100)
        );
    }

    #[test]
    fn recent_slots_outside_one_to_150_are_rejected() {
        assert!(RecentSlots::try_from(0).is_err());
        assert!(RecentSlots::try_from(151).is_err());
        assert_eq!(RecentSlots::try_from(1).unwrap(), RecentSlots(1));
        assert_eq!(RecentSlots::try_from(150).unwrap(), RecentSlots(150));
        assert!(serde_json::from_str::<RecentSlots>("0").is_err());
        assert!(serde_json::from_str::<RecentSlots>("151").is_err());
        assert_eq!(
            serde_json::from_str::<RecentSlots>("150").unwrap(),
            RecentSlots(150)
        );
    }

    #[test]
    fn percentile_of_an_empty_sample_is_none() {
        for p in 0..=100 {
            assert_eq!(Percentile(p).of(&[]), None, "percentile {p}");
        }
    }

    #[test]
    fn percentile_of_a_singleton_is_the_element() {
        for p in 0..=100 {
            assert_eq!(Percentile(p).of(&[7]), Some(7), "percentile {p}");
        }
    }

    /// In the sample 1..=100 the p-th percentile is p itself; 0 is the
    /// minimum.
    #[test]
    fn percentile_of_one_to_a_hundred_is_itself() {
        let sample: Vec<u64> = (1..=100).collect();
        assert_eq!(Percentile(0).of(&sample), Some(1));
        for p in 1..=100 {
            assert_eq!(
                Percentile(p).of(&sample),
                Some(u64::from(p)),
                "percentile {p}"
            );
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
            assert_eq!(Percentile(p).of(&sample), Some(expected), "percentile {p}");
        }
    }
}
