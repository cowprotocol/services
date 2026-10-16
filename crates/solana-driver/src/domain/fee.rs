//! The execution costs the driver charges the fills it reports.
//!
//! A settlement costs its payer the network fee of its transaction and the
//! rent of every buy token account it creates. A solver whose `fee-handler`
//! is the driver has those lamports cut from every fill its engine reports,
//! converted into the order's limit-leg token at the auction's native prices:
//! a sell order delivers less buy token and a buy order pulls more sell token
//! than the route achieved. The difference is retained in the settlement
//! program's buy-mint buffer PDA (sell orders) or the solver's own sell ATA
//! (buy orders). A sell order buying native SOL leaves it in the solver's own
//! lamports: the settlement unwraps the whole swap output to the solver and
//! moves only the payouts to the state PDA.
//!
//! The cut is applied before the driver reports solutions, so the autopilot
//! ranks the same post-cut figure the user receives. A solution whose trades
//! cannot absorb the cut within the order's signed limit is dropped whole:
//! its interactions were built for those trades, so dropping a single trade
//! would leave them inconsistent.
//!
//! On top of the costs the driver takes a margin, `solver-fee-bps`, off every
//! fill it prices, quotes included. An engine reports the leg its route
//! quotes, and a route delivering less leaves the buffer short of the push,
//! which reverts `FinalizeSettle`. The margin keeps the push under the quote
//! by that share.
//!
//! As in the EVM driver, the cut is enforced at two points: the limit leg
//! sent to the engine is tightened by the cut and the margin, so an engine
//! that checks its fills against the limit only returns fills that survive
//! them, and [`ExecutionCost::apply`] re-validates every fill so an engine
//! that ignores the limit still cannot undercut the user.

use {
    super::{
        Order,
        Side,
        order_uid::OrderUid,
        settlement::respects_limit,
        solution::{Solution, Trade},
    },
    cow_settlement_interface::data::intent::ENCODED_NATIVE_SOL_TRANSFER,
    solana_sdk::pubkey::Pubkey,
    std::{
        collections::{HashMap, HashSet},
        num::NonZero,
    },
};

/// Lamports per 10^9 atoms of native SOL: an atom is a lamport.
const NATIVE_PRICE: u64 = 1_000_000_000;

pub(super) const BPS_DENOMINATOR: u16 = 10_000;

/// The share of every fill the driver takes on top of the execution costs,
/// in basis points below 100%.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(try_from = "u16")]
pub struct Margin(u16);

#[derive(Debug, thiserror::Error)]
#[error("solver fee must be below {} bps, got {0}", BPS_DENOMINATOR)]
pub struct OutOfRange(u16);

impl TryFrom<u16> for Margin {
    type Error = OutOfRange;

    fn try_from(bps: u16) -> Result<Self, Self::Error> {
        // A margin of 100% or more leaves a sell order nothing to deliver and
        // makes `tighten_limit` divide by zero.
        (bps < BPS_DENOMINATOR)
            .then_some(Self(bps))
            .ok_or(OutOfRange(bps))
    }
}

impl Margin {
    /// The limit leg the engine has to beat for its fill to still respect
    /// `limit` once the margin is taken: a sell order's minimum buy rises to
    /// `limit / (1 - m)`, a buy order's maximum sell falls to `limit / (1 +
    /// m)`. Rounds against the engine. A minimum buy past `u64::MAX`
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

    /// The margin's share of one executed leg, rounded up against the trader.
    fn of(self, leg: u64) -> u64 {
        let scaled = u128::from(leg) * u128::from(self.0);
        u64::try_from(scaled.div_ceil(u128::from(BPS_DENOMINATOR)))
            .expect("a share below 100% of a u64 leg fits in u64")
    }
}

/// Who prices the execution costs into the fills, and what the engine is
/// sent accordingly.
#[derive(Clone, Debug)]
pub enum Charge {
    /// The engine: it is sent the signed limits and the orders whose buy
    /// token account the settlement creates.
    Engine {
        missing_buy_token_accounts: HashSet<OrderUid>,
    },
    /// The driver: the engine is sent each order's limit leg tightened by the
    /// atoms the driver cuts from its fill, by uid, and by its margin. An
    /// order without a cut is not sent.
    Driver {
        cuts: HashMap<OrderUid, u64>,
        margin: Margin,
    },
}

impl Default for Charge {
    fn default() -> Self {
        Self::Engine {
            missing_buy_token_accounts: HashSet::new(),
        }
    }
}

/// What a settlement of the auction's orders costs its payer beyond the
/// network fee, the prices that turn lamports into order tokens, and the
/// margin taken on top.
#[derive(Clone, Copy, Debug)]
pub struct ExecutionCost<'a> {
    /// The lamports the settlement pays to create each order's buy token
    /// account, by uid. An order not listed needs no account created.
    pub rent: &'a HashMap<OrderUid, u64>,
    /// Lamports per 10^9 atoms of each auction token.
    pub native_prices: &'a HashMap<Pubkey, u64>,
    /// The share of every fill taken on top of the costs.
    pub margin: Margin,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Rejected {
    #[error("trade {0} references no auction order")]
    UnknownOrder(OrderUid),
    #[error("trade {0} fills an order whose limit-leg token has no native price")]
    MissingPrice(OrderUid),
    #[error("trade {0} delivers nothing after the cut")]
    ZeroPayout(OrderUid),
    #[error("trade {0} sell leg overflows u64 after the cut")]
    Overflow(OrderUid),
    #[error("trade {0} undercuts its limit price after the cut")]
    LimitPrice(OrderUid),
}

impl ExecutionCost<'_> {
    /// The cut of each order a fill can be charged for: the atoms of its
    /// limit-leg token, the buy token of a sell order and the sell token of
    /// a buy order, worth its rent plus `network_fee` lamports. An order
    /// whose limit-leg token has no positive price is left out.
    pub fn cuts(&self, orders: &[Order], network_fee: u64) -> HashMap<OrderUid, u64> {
        orders
            .iter()
            .filter_map(|order| {
                let cut = self.cut(order, self.rent(order) + u128::from(network_fee));
                if cut.is_none() {
                    tracing::debug!(
                        order = %order.uid,
                        "not sending order, its limit-leg token has no native price"
                    );
                }
                Some((order.uid, cut?))
            })
            .collect()
    }

    /// Cuts every trade of `solution` by the margin's share of its leg, its
    /// order's rent and its share of `network_fee`, the settlement's network
    /// fee split evenly over the trades and rounded up. An order filled by
    /// several trades pays its rent on the first. One failing trade rejects
    /// the whole solution, left untouched.
    pub fn apply(
        &self,
        solution: &mut Solution,
        orders: &HashMap<OrderUid, &Order>,
        network_fee: u64,
    ) -> Result<(), Rejected> {
        let Some(trades) = NonZero::new(solution.trades.len() as u64) else {
            return Ok(());
        };
        let fee_share = network_fee.div_ceil(trades.get());
        let mut rent_charged = HashSet::new();
        let trades = solution
            .trades
            .iter()
            .map(|trade| {
                let order = orders
                    .get(&trade.order_uid)
                    .copied()
                    .ok_or(Rejected::UnknownOrder(trade.order_uid))?;
                let rent = if rent_charged.insert(order.uid) {
                    self.rent(order)
                } else {
                    0
                };
                let cut = self
                    .cut(order, rent + u128::from(fee_share))
                    .ok_or(Rejected::MissingPrice(order.uid))?;
                cut_trade(trade, order, self.margin, cut)
            })
            .collect::<Result<Vec<_>, _>>()?;
        solution.trades = trades;
        Ok(())
    }

    fn rent(&self, order: &Order) -> u128 {
        u128::from(self.rent.get(&order.uid).copied().unwrap_or(0))
    }

    /// The atoms of `order`'s limit-leg token worth `lamports`, rounded up.
    /// `None` without a positive price for the token, unless there is
    /// nothing to convert. A cut past `u64::MAX` saturates: no fill can
    /// absorb it.
    fn cut(&self, order: &Order, lamports: u128) -> Option<u64> {
        if lamports == 0 {
            return Some(0);
        }
        let token = match order.side {
            Side::Sell => order.buy_token,
            Side::Buy => order.sell_token,
        };
        let price = if token == ENCODED_NATIVE_SOL_TRANSFER {
            NATIVE_PRICE
        } else {
            self.native_prices.get(&token).copied()?
        };
        let price = NonZero::new(u128::from(price))?;
        let atoms = (lamports * u128::from(NATIVE_PRICE)).div_ceil(price.get());
        Some(u64::try_from(atoms).unwrap_or(u64::MAX))
    }
}

/// `trade` with the margin's share of its limit leg and `cut` taken off it:
/// a sell order delivers that much less buy token, a buy order pulls that
/// much more sell token.
fn cut_trade(trade: &Trade, order: &Order, margin: Margin, cut: u64) -> Result<Trade, Rejected> {
    let mut trade = trade.clone();
    let leg = match order.side {
        Side::Sell => trade.executed_buy,
        Side::Buy => trade.executed_sell,
    };
    let fee = margin
        .of(leg)
        .checked_add(cut)
        .ok_or(Rejected::Overflow(trade.order_uid))?;
    match order.side {
        Side::Sell => {
            trade.executed_buy = leg
                .checked_sub(fee)
                .filter(|delivered| *delivered > 0)
                .ok_or(Rejected::ZeroPayout(trade.order_uid))?;
        }
        Side::Buy => {
            trade.executed_sell = leg
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

#[cfg(test)]
mod tests {
    use {super::*, std::num::NonZero};

    fn pubkey(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    fn nz(value: u64) -> NonZero<u64> {
        NonZero::new(value).unwrap()
    }

    fn order(uid: u8, side: Side, sell_amount: u64, buy_amount: u64) -> Order {
        Order {
            uid: OrderUid([uid; 32]),
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
            executed: 0,
            sell_balance: None,
        }
    }

    fn trade(uid: u8, executed_sell: u64, executed_buy: u64) -> Trade {
        Trade {
            order_uid: OrderUid([uid; 32]),
            executed_sell,
            executed_buy,
            solver_fee: 0,
        }
    }

    fn solution(trades: Vec<Trade>) -> Solution {
        Solution {
            id: 1,
            solver: pubkey(0x99),
            prices: HashMap::from([(pubkey(0x33), nz(2_000)), (pubkey(0x44), nz(1_000))]),
            trades,
            interactions: vec![],
            address_lookup_tables: vec![],
            cu_estimate: None,
        }
    }

    fn orders(orders: &[Order]) -> HashMap<OrderUid, &Order> {
        orders.iter().map(|order| (order.uid, order)).collect()
    }

    /// The sell token is worth 2 lamports an atom and the buy token 0.5, so
    /// 1000 lamports are 500 sell atoms or 2000 buy atoms.
    fn prices() -> HashMap<Pubkey, u64> {
        HashMap::from([
            (pubkey(0x33), 2 * NATIVE_PRICE),
            (pubkey(0x44), NATIVE_PRICE / 2),
        ])
    }

    fn cost<'a>(
        rent: &'a HashMap<OrderUid, u64>,
        prices: &'a HashMap<Pubkey, u64>,
    ) -> ExecutionCost<'a> {
        ExecutionCost {
            rent,
            native_prices: prices,
            margin: Margin::default(),
        }
    }

    fn margin(bps: u16) -> Margin {
        Margin::try_from(bps).unwrap()
    }

    #[test]
    fn a_margin_is_below_100_percent_and_tightens_against_the_engine() {
        assert!(Margin::try_from(BPS_DENOMINATOR).is_err());
        assert!(Margin::try_from(BPS_DENOMINATOR - 1).is_ok());

        let margin = margin(500);
        assert_eq!(margin.tighten_limit(Side::Sell, 1_000), 1_053);
        assert_eq!(margin.tighten_limit(Side::Buy, 1_000), 952);
        assert_eq!(margin.tighten_limit(Side::Sell, 0), 0);
        assert_eq!(margin.tighten_limit(Side::Sell, u64::MAX), u64::MAX);
        assert_eq!(
            margin.tighten_limit(Side::Buy, u64::MAX),
            u64::try_from(u128::from(u64::MAX) * 10_000 / 10_500).unwrap()
        );
        assert_eq!(Margin::default().tighten_limit(Side::Sell, 1_000), 1_000);
    }

    /// 1000 lamports are 2000 buy atoms for a sell order and 500 sell atoms
    /// for a buy order, rent adds to the fee, a lone lamport rounds up to an
    /// atom, and a native SOL buy is cut in lamports with no price looked up.
    #[test]
    fn the_cut_is_the_rent_and_the_fee_in_the_limit_leg_token() {
        let rent = HashMap::from([(OrderUid([3; 32]), 500)]);
        let prices = prices();
        let cost = cost(&rent, &prices);
        let orders = [
            order(1, Side::Sell, 1_000, 1_000),
            order(2, Side::Buy, 1_000, 1_000),
            order(3, Side::Sell, 1_000, 1_000),
            Order {
                buy_token: ENCODED_NATIVE_SOL_TRANSFER,
                ..order(4, Side::Sell, 1_000, 1_000)
            },
        ];
        let cuts = cost.cuts(&orders, 1_000);
        assert_eq!(cuts[&orders[0].uid], 2_000);
        assert_eq!(cuts[&orders[1].uid], 500);
        assert_eq!(cuts[&orders[2].uid], 3_000);
        assert_eq!(cuts[&orders[3].uid], 1_000);
        assert_eq!(cost.cuts(&orders[1..2], 1)[&orders[1].uid], 1);
    }

    /// An order whose limit-leg token has no positive price gets no cut and
    /// is not listed, unless there is nothing to convert. A cut past
    /// `u64::MAX` saturates.
    #[test]
    fn a_cut_needs_a_price_unless_there_is_nothing_to_convert() {
        let rent = HashMap::new();
        let unpriced = HashMap::from([(pubkey(0x44), 0)]);
        let orders = [
            order(1, Side::Sell, 1_000, 1_000),
            order(2, Side::Buy, 1_000, 1_000),
        ];
        let cost = cost(&rent, &unpriced);
        assert!(cost.cuts(&orders, 1_000).is_empty());
        assert_eq!(
            cost.cuts(&orders, 0),
            HashMap::from([(orders[0].uid, 0), (orders[1].uid, 0)])
        );

        let tiny = HashMap::from([(pubkey(0x44), 1)]);
        let cuts = self::cost(&rent, &tiny).cuts(&orders[..1], u64::MAX);
        assert_eq!(cuts[&orders[0].uid], u64::MAX);
    }

    /// 5% of the 3000 buy leg is 150, on top of the 2000 atom cut. 5% of the
    /// 1000 sell leg is 50, on top of the 500 atom cut. Nothing to take
    /// leaves the fill as the engine reported it.
    #[test]
    fn the_margin_and_the_cut_come_off_the_limit_leg() {
        let (rent, prices) = (HashMap::new(), prices());
        let cost = ExecutionCost {
            margin: margin(500),
            ..cost(&rent, &prices)
        };

        let orders = [order(1, Side::Sell, 1_000, 500)];
        let mut sol = solution(vec![trade(1, 1_000, 3_000)]);
        cost.apply(&mut sol, &self::orders(&orders), 1_000).unwrap();
        assert_eq!(sol.trades[0].executed_sell, 1_000);
        assert_eq!(sol.trades[0].executed_buy, 850);
        assert_eq!(sol.trades[0].solver_fee, 2_150);

        let orders = [order(1, Side::Buy, 2_000, 1_000)];
        let mut sol = solution(vec![trade(1, 1_000, 1_000)]);
        cost.apply(&mut sol, &self::orders(&orders), 1_000).unwrap();
        assert_eq!(sol.trades[0].executed_sell, 1_550);
        assert_eq!(sol.trades[0].executed_buy, 1_000);
        assert_eq!(sol.trades[0].solver_fee, 550);

        let orders = [order(1, Side::Sell, 1_000, 1_000)];
        let mut sol = solution(vec![trade(1, 1_000, 1_000)]);
        self::cost(&rent, &prices)
            .apply(&mut sol, &self::orders(&orders), 0)
            .unwrap();
        assert_eq!(sol.trades[0].executed_buy, 1_000);
        assert_eq!(sol.trades[0].solver_fee, 0);
    }

    /// Two trades share the 1001 lamport fee as 501 lamports each, and only
    /// the first trade of an order pays its rent.
    #[test]
    fn the_network_fee_is_split_over_the_trades_and_the_rent_paid_once() {
        let rent = HashMap::from([(OrderUid([1; 32]), 499)]);
        let prices = prices();
        let orders = [order(1, Side::Sell, 1_000, 1_000)];
        let mut sol = solution(vec![trade(1, 500, 3_000), trade(1, 500, 3_000)]);
        cost(&rent, &prices)
            .apply(&mut sol, &self::orders(&orders), 1_001)
            .unwrap();
        assert_eq!(sol.trades[0].solver_fee, 2_000);
        assert_eq!(sol.trades[1].solver_fee, 1_002);
    }

    /// The limit leg sent to the engine, the signed limit moved by the cut
    /// and the margin, is the exact boundary: a fill on it survives, one atom
    /// past it is rejected.
    #[test]
    fn the_tightened_limit_is_the_exact_boundary() {
        let rent = HashMap::from([(OrderUid([1; 32]), 123)]);
        let prices = prices();
        let cost = ExecutionCost {
            margin: margin(500),
            ..cost(&rent, &prices)
        };
        for side in [Side::Sell, Side::Buy] {
            let orders = [order(1, side, 1_000, 1_000)];
            let cut = cost.cuts(&orders, 1_000)[&orders[0].uid];
            let (at_limit, beyond) = match side {
                Side::Sell => {
                    let min_buy = cost.margin.tighten_limit(side, 1_000 + cut);
                    (trade(1, 1_000, min_buy), trade(1, 1_000, min_buy - 1))
                }
                Side::Buy => {
                    let max_sell = cost.margin.tighten_limit(side, 1_000 - cut);
                    (trade(1, max_sell, 1_000), trade(1, max_sell + 1, 1_000))
                }
            };
            cost.apply(&mut solution(vec![at_limit]), &self::orders(&orders), 1_000)
                .unwrap();
            assert_eq!(
                cost.apply(&mut solution(vec![beyond]), &self::orders(&orders), 1_000),
                Err(Rejected::LimitPrice(orders[0].uid))
            );
        }
    }

    /// Each failing trade names its reason, and the passing trade before it
    /// is left as the engine reported it.
    #[test]
    fn a_bad_trade_rejects_the_whole_solution_untouched() {
        let (rent, prices) = (HashMap::new(), prices());
        let cost = cost(&rent, &prices);
        let fine = order(1, Side::Sell, 1_000, 1_000);
        let uid = |byte| OrderUid([byte; 32]);
        let cases = [
            (
                order(8, Side::Sell, 1_000, 1_000),
                trade(9, 1_000, 3_000),
                Rejected::UnknownOrder(uid(9)),
            ),
            (
                Order {
                    buy_token: pubkey(0x45),
                    ..order(2, Side::Sell, 1_000, 1_000)
                },
                trade(2, 1_000, 3_000),
                Rejected::MissingPrice(uid(2)),
            ),
            (
                order(3, Side::Sell, 1_000, 1),
                trade(3, 1_000, 1_000),
                Rejected::ZeroPayout(uid(3)),
            ),
            (
                order(4, Side::Buy, u64::MAX, 1_000),
                trade(4, u64::MAX, 1_000),
                Rejected::Overflow(uid(4)),
            ),
            (
                order(5, Side::Sell, 1_000, 1_000),
                trade(5, 1_000, 1_500),
                Rejected::LimitPrice(uid(5)),
            ),
        ];
        for (order, bad, reason) in cases {
            let orders = [fine.clone(), order];
            let mut sol = solution(vec![trade(1, 1_000, 3_000), bad]);
            assert_eq!(
                cost.apply(&mut sol, &self::orders(&orders), 1_000),
                Err(reason)
            );
            assert_eq!(
                (sol.trades[0].executed_buy, sol.trades[0].solver_fee),
                (3_000, 0)
            );
        }
    }
}
