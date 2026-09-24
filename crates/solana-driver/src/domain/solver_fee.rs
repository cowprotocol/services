//! Volume-based solver fee.
//!
//! The engine builds its interactions to fill each order at the trade legs it
//! reported. The solver fee then moves the executed legs into the conservative
//! direction: a sell order delivers less buy token and a buy order pulls more
//! sell token than the route achieved. The difference is retained in the
//! settlement program's buy-mint buffer PDA (sell orders) or the solver's own
//! sell ATA (buy orders).
//!
//! The fee is applied to every fill before the driver reports solutions, so the
//! autopilot ranks the same post-fee figure the user receives. A solution whose
//! trades cannot absorb the fee within the order's signed limit is dropped
//! whole: its interactions were built for those trades, so dropping a single
//! trade would leave them inconsistent.
//!
//! As in the EVM driver, the fee is enforced at two points: the limit leg sent
//! to the engine is tightened by the fee, so an engine that checks its fills
//! against both legs only returns fills that survive it, and the post-fee check
//! in [`SolverFee::apply`] re-validates every fill so an engine that ignores
//! the limit leg still cannot undercut the user.

use {
    super::{
        Order,
        Side,
        order_uid::OrderUid,
        settlement::respects_limit,
        solution::{Solution, Trade},
    },
    std::collections::HashMap,
};

const BPS_DENOMINATOR: u16 = 10_000;

/// A fee in basis points, below 100%.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(try_from = "u16")]
pub struct SolverFee(u16);

#[derive(Debug, thiserror::Error)]
#[error("solver fee must be below {} bps, got {0}", BPS_DENOMINATOR)]
pub struct OutOfRange(u16);

impl TryFrom<u16> for SolverFee {
    type Error = OutOfRange;

    fn try_from(bps: u16) -> Result<Self, Self::Error> {
        // A fee of 100% or more leaves a sell order nothing to deliver and
        // makes `tighten_limit` divide by zero.
        (bps < BPS_DENOMINATOR)
            .then_some(Self(bps))
            .ok_or(OutOfRange(bps))
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Rejected {
    #[error("trade {0} references no auction order")]
    UnknownOrder(OrderUid),
    #[error("trade {0} pays nothing after the solver fee")]
    ZeroPayout(OrderUid),
    #[error("trade {0} sell leg overflows u64 after the solver fee")]
    Overflow(OrderUid),
    #[error("trade {0} undercuts its limit price after the solver fee")]
    LimitPrice(OrderUid),
}

impl SolverFee {
    /// The limit leg the engine has to beat for its fill to still respect the
    /// signed limit once the fee is applied: a sell order's minimum buy rises
    /// to `buy / (1 - f)`, a buy order's maximum sell falls to `sell / (1 +
    /// f)`. Rounds against the engine. A minimum buy past `u64::MAX`
    /// saturates: no fill can meet it.
    pub fn tighten_limit(self, side: Side, limit: u64) -> u64 {
        let limit = u128::from(limit);
        let base = u128::from(BPS_DENOMINATOR);
        let bps = u128::from(self.0);
        match side {
            Side::Sell => u64::try_from((limit * base).div_ceil(base - bps)).unwrap_or(u64::MAX),
            Side::Buy => u64::try_from(limit * base / (base + bps))
                .expect("dividing by more than the multiplier never exceeds the input"),
        }
    }

    /// The fee on one executed leg, `executed * f` rounded up in the fee's
    /// favour. The leg is the engine's pre-fee figure, so the same factor
    /// applies on both sides, like the EVM driver's `fee_from_volume`.
    fn fee_from_volume(self, executed: u64) -> u64 {
        let scaled = u128::from(executed)
            .checked_mul(u128::from(self.0))
            .expect("the product of a 64-bit and a 16-bit number always fits in 128 bits");
        u64::try_from(scaled.div_ceil(u128::from(BPS_DENOMINATOR)))
            .expect("a fee below 100% of a u64 leg fits in u64")
    }

    /// Applies the solver fee to every trade of `solution`.
    ///
    /// If one trade fails a step, the whole solution is rejected and left
    /// untouched.
    pub fn apply(
        self,
        solution: &mut Solution,
        orders: &HashMap<OrderUid, &Order>,
    ) -> Result<(), Rejected> {
        let trades = solution
            .trades
            .iter()
            .map(|trade| {
                let order = orders
                    .get(&trade.order_uid)
                    .copied()
                    .ok_or(Rejected::UnknownOrder(trade.order_uid))?;
                self.apply_to_trade(trade, order)
            })
            .collect::<Result<Vec<_>, _>>()?;
        solution.trades = trades;
        Ok(())
    }

    fn apply_to_trade(self, trade: &Trade, order: &Order) -> Result<Trade, Rejected> {
        let mut trade = trade.clone();
        let fee_leg = match order.side {
            Side::Sell => trade.executed_buy,
            Side::Buy => trade.executed_sell,
        };
        let fee = self.fee_from_volume(fee_leg);
        match order.side {
            Side::Sell => {
                let delivered = fee_leg
                    .checked_sub(fee)
                    .expect("a fee below 100% never exceeds the amount it is taken from");
                if delivered == 0 {
                    return Err(Rejected::ZeroPayout(trade.order_uid));
                }
                trade.executed_buy = delivered;
            }
            Side::Buy => {
                trade.executed_sell = fee_leg
                    .checked_add(fee)
                    .ok_or(Rejected::Overflow(trade.order_uid))?;
            }
        }
        trade.solver_fee = fee;
        if !respects_limit(order, trade.executed_sell, trade.executed_buy) {
            return Err(Rejected::LimitPrice(trade.order_uid));
        }
        Ok(trade)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::{super::auction::Order, *},
        solana_sdk::pubkey::Pubkey,
        std::num::NonZero,
    };

    fn pubkey(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    fn nz(value: u64) -> NonZero<u64> {
        NonZero::new(value).unwrap()
    }

    fn order(side: Side, sell_amount: u64, buy_amount: u64) -> Order {
        Order {
            uid: OrderUid([8; 32]),
            owner: pubkey(0x22),
            sell_token: pubkey(0x33),
            buy_token: pubkey(0x44),
            sell_token_account: pubkey(0x55),
            buy_token_account: pubkey(0x66),
            sell_amount,
            buy_amount,
            valid_to: u32::MAX,
            side,
            partially_fillable: false,
            order_pda: pubkey(0x67),
            app_data: [0x77; 32],
        }
    }

    fn solution(executed_sell: u64, executed_buy: u64) -> Solution {
        Solution {
            id: 1,
            solver: pubkey(0x99),
            prices: HashMap::from([(pubkey(0x33), nz(2_000)), (pubkey(0x44), nz(1_000))]),
            trades: vec![Trade {
                order_uid: OrderUid([8; 32]),
                executed_sell,
                executed_buy,
                solver_fee: 0,
            }],
            interactions: vec![],
            address_lookup_tables: vec![],
            cu_estimate: None,
        }
    }

    fn orders(order: &Order) -> HashMap<OrderUid, &Order> {
        HashMap::from([(order.uid, order)])
    }

    #[test]
    fn zero_fee_is_identity() {
        let order = order(Side::Sell, 1_000, 2_000);
        let mut sol = solution(1_000, 2_000);
        SolverFee::try_from(0)
            .unwrap()
            .apply(&mut sol, &orders(&order))
            .unwrap();
        let trade = &sol.trades[0];
        assert_eq!(trade.executed_sell, 1_000);
        assert_eq!(trade.executed_buy, 2_000);
        assert_eq!(trade.solver_fee, 0);
    }

    #[test]
    fn sell_order_fee_reduces_buy_amount() {
        let order = order(Side::Sell, 1_000, 1_000);
        let mut sol = solution(1_000, 2_000);
        SolverFee::try_from(500)
            .unwrap()
            .apply(&mut sol, &orders(&order))
            .unwrap();
        let trade = &sol.trades[0];
        assert_eq!(trade.executed_sell, 1_000);
        assert_eq!(trade.executed_buy, 1_900);
        assert_eq!(trade.solver_fee, 100);
    }

    #[test]
    fn buy_order_fee_increases_sell_amount() {
        let order = order(Side::Buy, 2_000, 2_000);
        let mut sol = solution(1_000, 2_000);
        SolverFee::try_from(500)
            .unwrap()
            .apply(&mut sol, &orders(&order))
            .unwrap();
        let trade = &sol.trades[0];
        assert_eq!(trade.executed_sell, 1_050);
        assert_eq!(trade.executed_buy, 2_000);
        assert_eq!(trade.solver_fee, 50);
    }

    #[test]
    fn fee_from_volume_rounds_up() {
        let fee = SolverFee::try_from(500).unwrap();
        assert_eq!(fee.fee_from_volume(1), 1);
        assert_eq!(fee.fee_from_volume(999), 50);
        assert_eq!(fee.fee_from_volume(1_000), 50);
    }

    #[test]
    fn tighten_limit_rounds_against_the_engine() {
        let fee = SolverFee::try_from(500).unwrap();
        // 1000 / 0.95 = 1052.6 and 1000 / 1.05 = 952.4.
        assert_eq!(fee.tighten_limit(Side::Sell, 1_000), 1_053);
        assert_eq!(fee.tighten_limit(Side::Buy, 1_000), 952);
        assert_eq!(fee.tighten_limit(Side::Sell, 0), 0);
    }

    #[test]
    fn buy_quote_placeholder_maximum_sell_fits_u64() {
        let fee = SolverFee::try_from(500).unwrap();
        assert_eq!(
            fee.tighten_limit(Side::Buy, u64::MAX),
            17_568_327_689_247_192_014
        );
    }

    #[test]
    fn zero_fee_leaves_the_limit_unchanged() {
        let fee = SolverFee::try_from(0).unwrap();
        assert_eq!(fee.tighten_limit(Side::Sell, 1_000), 1_000);
        assert_eq!(fee.tighten_limit(Side::Buy, 1_000), 1_000);
    }

    #[test]
    fn unrepresentable_minimum_buy_saturates() {
        let fee = SolverFee::try_from(1).unwrap();
        assert_eq!(fee.tighten_limit(Side::Sell, u64::MAX), u64::MAX);
    }

    #[test]
    fn fill_at_tightened_limit_survives_the_fee() {
        let fee = SolverFee::try_from(500).unwrap();

        let sell = order(Side::Sell, 1_000, 1_000);
        let mut sol = solution(1_000, fee.tighten_limit(Side::Sell, sell.buy_amount));
        fee.apply(&mut sol, &orders(&sell)).unwrap();
        assert_eq!(sol.trades[0].executed_buy, 1_000);

        let buy = order(Side::Buy, 1_000, 1_000);
        let mut sol = solution(fee.tighten_limit(Side::Buy, buy.sell_amount), 1_000);
        fee.apply(&mut sol, &orders(&buy)).unwrap();
        assert_eq!(sol.trades[0].executed_sell, 1_000);
    }

    /// `tighten_limit` and `apply` round independently. A fill at exactly the
    /// tightened limit survives the fee and one unit worse does not, on both
    /// sides, across fee sizes and limit magnitudes.
    #[test]
    fn tightened_limit_is_the_exact_boundary_of_apply() {
        for bps in [1u16, 7, 500, 2_500, 9_999] {
            let fee = SolverFee::try_from(bps).unwrap();
            for limit in [1u64, 3, 1_000, 12_345, 1_000_000_000_000] {
                let sell = order(Side::Sell, 1_000, limit);
                let min_buy = fee.tighten_limit(Side::Sell, limit);
                assert!(
                    fee.apply(&mut solution(1_000, min_buy), &orders(&sell))
                        .is_ok(),
                    "sell bps={bps} limit={limit}"
                );
                assert!(
                    fee.apply(&mut solution(1_000, min_buy - 1), &orders(&sell))
                        .is_err(),
                    "sell bps={bps} limit={limit}"
                );

                let buy = order(Side::Buy, limit, 1_000);
                let max_sell = fee.tighten_limit(Side::Buy, limit);
                assert!(
                    fee.apply(&mut solution(max_sell, 1_000), &orders(&buy))
                        .is_ok(),
                    "buy bps={bps} limit={limit}"
                );
                assert!(
                    fee.apply(&mut solution(max_sell + 1, 1_000), &orders(&buy))
                        .is_err(),
                    "buy bps={bps} limit={limit}"
                );
            }
        }
    }

    #[test]
    fn buy_order_fee_overflow_rejects_solution() {
        let order = order(Side::Buy, u64::MAX, 2_000);
        let mut sol = solution(u64::MAX, 2_000);
        let err = SolverFee::try_from(1)
            .unwrap()
            .apply(&mut sol, &orders(&order))
            .unwrap_err();
        assert_eq!(err, Rejected::Overflow(OrderUid([8; 32])));
    }

    #[test]
    fn sell_order_fee_zeroing_payout_rejects_solution() {
        let order = order(Side::Sell, 1_000, 1);
        let mut sol = solution(1_000, 1);
        let err = SolverFee::try_from(500)
            .unwrap()
            .apply(&mut sol, &orders(&order))
            .unwrap_err();
        assert_eq!(err, Rejected::ZeroPayout(OrderUid([8; 32])));
    }

    #[test]
    fn filters_fill_at_limit() {
        let order = order(Side::Sell, 1_000, 1_000);
        let mut sol = solution(1_000, 1_000);
        let err = SolverFee::try_from(500)
            .unwrap()
            .apply(&mut sol, &orders(&order))
            .unwrap_err();
        assert_eq!(err, Rejected::LimitPrice(OrderUid([8; 32])));
    }

    #[test]
    fn keeps_fill_that_respects_limit_after_fee() {
        let order = order(Side::Sell, 1_000, 1_000);
        let mut sol = solution(1_000, 2_000);
        SolverFee::try_from(500)
            .unwrap()
            .apply(&mut sol, &orders(&order))
            .unwrap();
        assert_eq!(sol.trades[0].executed_buy, 1_900);
    }

    #[test]
    fn rejects_whole_solution_on_one_bad_trade() {
        let order = order(Side::Sell, 1_000, 1_000);
        let mut sol = solution(1_000, 2_000);
        sol.trades.push(Trade {
            order_uid: OrderUid([9; 32]),
            executed_sell: 1_000,
            executed_buy: 1_000,
            solver_fee: 0,
        });
        let mut m = orders(&order);
        let mut order2 = order.clone();
        order2.uid = OrderUid([9; 32]);
        m.insert(order2.uid, &order2);
        let err = SolverFee::try_from(500)
            .unwrap()
            .apply(&mut sol, &m)
            .unwrap_err();
        assert_eq!(err, Rejected::LimitPrice(OrderUid([9; 32])));
        // The first trade passed on its own; the rejection leaves it untouched.
        assert_eq!(sol.trades[0].executed_buy, 2_000);
        assert_eq!(sol.trades[0].solver_fee, 0);
    }

    #[test]
    fn quote_buy_order_with_max_sell_limit_passes() {
        let order = order(Side::Buy, u64::MAX, 1_000);
        let mut sol = solution(1_000, 1_000);
        SolverFee::try_from(500)
            .unwrap()
            .apply(&mut sol, &orders(&order))
            .unwrap();
        assert_eq!(sol.trades[0].executed_sell, 1_050);
    }
}
