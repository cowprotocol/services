//! The execution costs the driver cuts from the fills it reports.
//!
//! A settlement costs its payer the network fee of its transaction and the
//! rent of every buy token account it creates. A solver whose `fee-handler`
//! is the driver has those lamports cut from every fill its engine reports,
//! converted into the order's limit-leg token at the auction's native prices:
//! a sell order delivers less buy token and a buy order pulls more sell token
//! than the route achieved. The difference stays where the solver fee's
//! does, see [`super::solver_fee`].
//!
//! The cut is taken at two points of solve time. Before any route is known,
//! the limit leg sent to the engine is moved by the cut priced at the
//! policy's default compute units, before the solver fee tightens it. Once
//! the settlement is simulated, [`ExecutionCost::apply`] cuts every fill at
//! the compute units the settlement will declare, or else the policy's
//! default. A solution whose trades cannot absorb the cut within the order's
//! signed limit is dropped whole, like one that cannot absorb the fee.

use {
    super::{
        Order,
        Side,
        order_uid::OrderUid,
        settlement::respects_limit,
        solution::{Solution, Trade},
        solver_fee::SolverFee,
    },
    chain_types::{ChainTypes, solana::Solana},
    cow_settlement_interface::data::intent::ENCODED_NATIVE_SOL_TRANSFER,
    solana_sdk::pubkey::Pubkey,
    std::{
        collections::{HashMap, HashSet},
        num::NonZero,
    },
};

/// Who prices the execution costs into the fills, and what the engine is
/// sent accordingly.
#[derive(Clone, Debug)]
pub enum Charge {
    /// The engine: it is sent the signed limits and the orders whose buy
    /// token account the settlement creates.
    Engine {
        missing_buy_token_accounts: HashSet<OrderUid>,
    },
    /// The driver: the engine is sent each order's limit leg moved by the
    /// atoms the driver cuts from its fill, by uid, and tightened by the
    /// solver fee. An order without a cut is not sent.
    Driver {
        cuts: HashMap<OrderUid, u64>,
        fee: Option<SolverFee>,
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
/// network fee, and the prices that turn lamports into order tokens.
#[derive(Clone, Copy, Debug)]
pub struct ExecutionCost<'a> {
    /// The lamports the settlement pays to create each order's buy token
    /// account, by uid. An order not listed needs no account created.
    pub rent: &'a HashMap<OrderUid, u64>,
    /// Lamports per 10^9 atoms of each auction token.
    pub native_prices: &'a HashMap<Pubkey, u64>,
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

    /// Cuts every trade of `solution` by its order's rent and its share of
    /// `network_fee`, the settlement's network fee split evenly over the
    /// trades and rounded up. An order filled by several trades pays its
    /// rent on the first. One failing trade rejects the whole solution, left
    /// untouched. `Trade::solver_fee` carries the solver fee only, not the
    /// cut.
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
        let solution_id = solution.id;
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
                let lamports = rent + u128::from(fee_share);
                let atoms = self
                    .cut(order, lamports)
                    .ok_or(Rejected::MissingPrice(order.uid))?;
                tracing::debug!(
                    solution_id,
                    order = %order.uid,
                    lamports,
                    atoms,
                    "cutting the execution cost from the fill"
                );
                cut_trade(trade, order, atoms)
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
            Solana::NATIVE_PRICE_DENOMINATOR
        } else {
            self.native_prices.get(&token).copied()?
        };
        let price = NonZero::new(u128::from(price))?;
        let atoms = (lamports * u128::from(Solana::NATIVE_PRICE_DENOMINATOR)).div_ceil(price.get());
        Some(u64::try_from(atoms).unwrap_or(u64::MAX))
    }
}

/// `trade` with `cut` taken off its limit leg: a sell order delivers that
/// much less buy token, a buy order pulls that much more sell token. Nothing
/// to cut leaves the trade as the engine reported it.
fn cut_trade(trade: &Trade, order: &Order, cut: u64) -> Result<Trade, Rejected> {
    let mut trade = trade.clone();
    if cut == 0 {
        return Ok(trade);
    }
    match order.side {
        Side::Sell => {
            trade.executed_buy = trade
                .executed_buy
                .checked_sub(cut)
                .filter(|delivered| *delivered > 0)
                .ok_or(Rejected::ZeroPayout(trade.order_uid))?;
        }
        Side::Buy => {
            trade.executed_sell = trade
                .executed_sell
                .checked_add(cut)
                .ok_or(Rejected::Overflow(trade.order_uid))?;
        }
    }
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
            solver_fee: 7,
        }
    }

    fn solution(trades: Vec<Trade>) -> Solution {
        Solution {
            id: 1,
            solver: pubkey(0x99),
            prices: HashMap::from([(pubkey(0x33), nz(2_000)), (pubkey(0x44), nz(1_000))]),
            trades,
            interactions: vec![],
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
            (pubkey(0x33), 2 * Solana::NATIVE_PRICE_DENOMINATOR),
            (pubkey(0x44), Solana::NATIVE_PRICE_DENOMINATOR / 2),
        ])
    }

    fn cost<'a>(
        rent: &'a HashMap<OrderUid, u64>,
        prices: &'a HashMap<Pubkey, u64>,
    ) -> ExecutionCost<'a> {
        ExecutionCost {
            rent,
            native_prices: prices,
        }
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

    /// 1000 lamports come off the 3000 buy leg as 2000 atoms and go onto the
    /// 1000 sell leg as 500. The trade's solver fee is left alone. Nothing to
    /// cut leaves the fill as the engine reported it.
    #[test]
    fn the_cut_comes_off_the_limit_leg() {
        let (rent, prices) = (HashMap::new(), prices());
        let cost = cost(&rent, &prices);

        let orders = [order(1, Side::Sell, 1_000, 500)];
        let mut sol = solution(vec![trade(1, 1_000, 3_000)]);
        cost.apply(&mut sol, &self::orders(&orders), 1_000).unwrap();
        assert_eq!(sol.trades[0].executed_sell, 1_000);
        assert_eq!(sol.trades[0].executed_buy, 1_000);
        assert_eq!(sol.trades[0].solver_fee, 7);

        let orders = [order(1, Side::Buy, 2_000, 1_000)];
        let mut sol = solution(vec![trade(1, 1_000, 1_000)]);
        cost.apply(&mut sol, &self::orders(&orders), 1_000).unwrap();
        assert_eq!(sol.trades[0].executed_sell, 1_500);
        assert_eq!(sol.trades[0].executed_buy, 1_000);

        let orders = [order(1, Side::Sell, 1_000, 1_000)];
        let mut sol = solution(vec![trade(1, 1_000, 1_000)]);
        cost.apply(&mut sol, &self::orders(&orders), 0).unwrap();
        assert_eq!(sol.trades[0].executed_buy, 1_000);
    }

    /// Two trades share the 1001 lamport fee as 501 lamports each, and only
    /// the first trade of an order pays its rent: 1000 lamports are 2000
    /// atoms off the first buy leg, 501 are 1002 off the second.
    #[test]
    fn the_network_fee_is_split_over_the_trades_and_the_rent_paid_once() {
        let rent = HashMap::from([(OrderUid([1; 32]), 499)]);
        let prices = prices();
        let orders = [order(1, Side::Sell, 1_000, 1_000)];
        let mut sol = solution(vec![trade(1, 500, 3_000), trade(1, 500, 3_000)]);
        cost(&rent, &prices)
            .apply(&mut sol, &self::orders(&orders), 1_001)
            .unwrap();
        assert_eq!(sol.trades[0].executed_buy, 1_000);
        assert_eq!(sol.trades[1].executed_buy, 1_998);
    }

    /// The limit leg sent to the engine, the signed limit moved by the cut,
    /// is the exact boundary: a fill on it survives, one atom past it is
    /// rejected.
    #[test]
    fn the_moved_limit_is_the_exact_boundary() {
        let rent = HashMap::from([(OrderUid([1; 32]), 123)]);
        let prices = prices();
        let cost = cost(&rent, &prices);
        for side in [Side::Sell, Side::Buy] {
            let orders = [order(1, side, 1_000, 1_000)];
            let cut = cost.cuts(&orders, 1_000)[&orders[0].uid];
            let (at_limit, beyond) = match side {
                Side::Sell => (trade(1, 1_000, 1_000 + cut), trade(1, 1_000, 999 + cut)),
                Side::Buy => (trade(1, 1_000 - cut, 1_000), trade(1, 1_001 - cut, 1_000)),
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
            assert_eq!(sol.trades[0].executed_buy, 3_000);
        }
    }
}
