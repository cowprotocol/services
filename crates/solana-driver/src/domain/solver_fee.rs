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

use {
    super::{Order, Side, order_uid::OrderUid, settlement::respects_limit, solution::Solution},
    std::collections::HashMap,
};

pub const MAX_BASE_POINT: u16 = 10_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SolverFee(u16);

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
    pub fn new(bps: u16) -> Option<Self> {
        (bps < MAX_BASE_POINT).then_some(Self(bps))
    }

    pub fn bps(self) -> u16 {
        self.0
    }

    /// The fee on one executed leg, `executed * f` rounded up in the fee's
    /// favour. The same factor applies to both sides because the leg is the
    /// engine's pre-fee figure, as in the EVM driver's `fee_from_volume`. The
    /// autopilot's `f / (1 - f)` and `f / (1 + f)` back the fee out of
    /// post-fee amounts and do not apply here.
    fn fee_from_volume(self, executed: u64) -> u64 {
        let scaled = u128::from(executed)
            .checked_mul(u128::from(self.0))
            .expect("the product of a 64-bit and a 16-bit number always fits in 128 bits");
        u64::try_from(scaled.div_ceil(u128::from(MAX_BASE_POINT)))
            .expect("a fee below 100% of a u64 leg fits in u64")
    }

    /// Applies the solver fee to every trade of `solution`, in place.
    ///
    /// If one trade fails a step, the whole solution is rejected.
    pub fn apply(
        self,
        solution: &mut Solution,
        orders: &HashMap<OrderUid, &Order>,
    ) -> Result<(), Rejected> {
        for trade in &mut solution.trades {
            let order = orders
                .get(&trade.order_uid)
                .copied()
                .ok_or(Rejected::UnknownOrder(trade.order_uid))?;
            let fee_leg = match order.side {
                Side::Sell => trade.executed_buy,
                Side::Buy => trade.executed_sell,
            };
            let fee = self.fee_from_volume(fee_leg);
            match order.side {
                Side::Sell => {
                    // The user receives the leg minus the fee.
                    let delivered = fee_leg
                        .checked_sub(fee)
                        .expect("a fee below 100% never exceeds the amount it is taken from");
                    if delivered == 0 {
                        return Err(Rejected::ZeroPayout(trade.order_uid));
                    }
                    trade.executed_buy = delivered;
                }
                Side::Buy => {
                    // The user pays the leg plus the fee.
                    trade.executed_sell = fee_leg
                        .checked_add(fee)
                        .ok_or(Rejected::Overflow(trade.order_uid))?;
                }
            }
            // The legs alone no longer show the fee. The settle path logs it.
            trade.solver_fee = fee;
            if !respects_limit(order, trade.executed_sell, trade.executed_buy) {
                return Err(Rejected::LimitPrice(trade.order_uid));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use {
        super::{
            super::{auction::Order, solution::Trade},
            *,
        },
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
        SolverFee::new(0)
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
        SolverFee::new(500)
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
        SolverFee::new(500)
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
        let fee = SolverFee::new(500).unwrap();
        assert_eq!(fee.fee_from_volume(1), 1);
        assert_eq!(fee.fee_from_volume(999), 50);
        assert_eq!(fee.fee_from_volume(1_000), 50);
    }

    #[test]
    fn buy_order_fee_overflow_rejects_solution() {
        let order = order(Side::Buy, u64::MAX, 2_000);
        let mut sol = solution(u64::MAX, 2_000);
        let err = SolverFee::new(1)
            .unwrap()
            .apply(&mut sol, &orders(&order))
            .unwrap_err();
        assert_eq!(err, Rejected::Overflow(OrderUid([8; 32])));
    }

    #[test]
    fn sell_order_fee_zeroing_payout_rejects_solution() {
        let order = order(Side::Sell, 1_000, 1);
        let mut sol = solution(1_000, 1);
        let err = SolverFee::new(500)
            .unwrap()
            .apply(&mut sol, &orders(&order))
            .unwrap_err();
        assert_eq!(err, Rejected::ZeroPayout(OrderUid([8; 32])));
    }

    #[test]
    fn filters_fill_at_limit() {
        let order = order(Side::Sell, 1_000, 1_000);
        let mut sol = solution(1_000, 1_000);
        let err = SolverFee::new(500)
            .unwrap()
            .apply(&mut sol, &orders(&order))
            .unwrap_err();
        assert_eq!(err, Rejected::LimitPrice(OrderUid([8; 32])));
    }

    #[test]
    fn keeps_fill_that_respects_limit_after_fee() {
        let order = order(Side::Sell, 1_000, 1_000);
        let mut sol = solution(1_000, 2_000);
        SolverFee::new(500)
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
        let err = SolverFee::new(500)
            .unwrap()
            .apply(&mut sol, &m)
            .unwrap_err();
        assert_eq!(err, Rejected::LimitPrice(OrderUid([9; 32])));
    }

    #[test]
    fn quote_buy_order_with_max_sell_limit_passes() {
        let order = order(Side::Buy, u64::MAX, 1_000);
        let mut sol = solution(1_000, 1_000);
        SolverFee::new(500)
            .unwrap()
            .apply(&mut sol, &orders(&order))
            .unwrap();
        assert_eq!(sol.trades[0].executed_sell, 1_050);
    }
}
