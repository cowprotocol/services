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
//! As in the EVM driver, the cut is enforced at two points: the limit leg
//! sent to the engine is tightened by the cut, so an engine that checks its
//! fills against the limit only returns fills that survive it, and
//! [`ExecutionCost::apply`] re-validates every fill so an engine that ignores
//! the limit still cannot undercut the user.

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
    /// atoms the driver cuts from its fill, by uid. An order without a cut is
    /// not sent.
    Driver { cuts: HashMap<OrderUid, u64> },
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
    /// rent on the first. One failing trade rejects the whole solution,
    /// left untouched.
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
                cut_trade(trade, order, cut)
            })
            .collect::<Result<Vec<_>, _>>()?;
        solution.trades = trades;
        Ok(())
    }

    fn rent(&self, order: &Order) -> u128 {
        u128::from(self.rent.get(&order.uid).copied().unwrap_or(0))
    }

    /// The atoms of `order`'s limit-leg token worth `lamports`, rounded up.
    /// `None` without a positive price for the token. A cut past `u64::MAX`
    /// saturates: no fill can absorb it.
    fn cut(&self, order: &Order, lamports: u128) -> Option<u64> {
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

/// `trade` with `cut` taken off its limit leg: a sell order delivers that
/// much less buy token, a buy order pulls that much more sell token.
fn cut_trade(trade: &Trade, order: &Order, cut: u64) -> Result<Trade, Rejected> {
    let mut trade = trade.clone();
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
    trade.solver_fee = cut;
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
        }
    }

    #[test]
    fn the_cut_is_in_the_limit_leg_token() {
        let (rent, prices) = (HashMap::new(), prices());
        let cost = cost(&rent, &prices);
        let orders = [
            order(1, Side::Sell, 1_000, 1_000),
            order(2, Side::Buy, 1_000, 1_000),
        ];
        let cuts = cost.cuts(&orders, 1_000);
        assert_eq!(cuts[&orders[0].uid], 2_000);
        assert_eq!(cuts[&orders[1].uid], 500);
    }

    #[test]
    fn the_rent_adds_to_the_network_fee() {
        let rent = HashMap::from([(OrderUid([1; 32]), 500)]);
        let prices = prices();
        let orders = [order(1, Side::Sell, 1_000, 1_000)];
        let cuts = cost(&rent, &prices).cuts(&orders, 1_000);
        assert_eq!(cuts[&orders[0].uid], 3_000);
    }

    #[test]
    fn the_cut_rounds_up() {
        let (rent, prices) = (HashMap::new(), prices());
        let orders = [order(1, Side::Buy, 1_000, 1_000)];
        let cuts = cost(&rent, &prices).cuts(&orders, 1);
        assert_eq!(cuts[&orders[0].uid], 1);
    }

    /// A lamport is an atom of native SOL, whether or not the auction prices
    /// it.
    #[test]
    fn a_native_sol_buy_is_cut_in_lamports() {
        let (rent, prices) = (HashMap::new(), HashMap::new());
        let orders = [Order {
            buy_token: ENCODED_NATIVE_SOL_TRANSFER,
            ..order(1, Side::Sell, 1_000, 1_000)
        }];
        let cuts = cost(&rent, &prices).cuts(&orders, 1_000);
        assert_eq!(cuts[&orders[0].uid], 1_000);
    }

    #[test]
    fn an_order_without_a_price_has_no_cut() {
        let rent = HashMap::new();
        let unpriced = HashMap::from([(pubkey(0x44), 0)]);
        let orders = [
            order(1, Side::Sell, 1_000, 1_000),
            order(2, Side::Buy, 1_000, 1_000),
        ];
        assert!(cost(&rent, &unpriced).cuts(&orders, 1_000).is_empty());
    }

    #[test]
    fn a_cut_past_u64_saturates() {
        let (rent, prices) = (HashMap::new(), HashMap::from([(pubkey(0x44), 1)]));
        let orders = [order(1, Side::Sell, 1_000, 1_000)];
        let cuts = cost(&rent, &prices).cuts(&orders, u64::MAX);
        assert_eq!(cuts[&orders[0].uid], u64::MAX);
    }

    #[test]
    fn a_sell_order_delivers_less() {
        let (rent, prices) = (HashMap::new(), prices());
        let orders = [order(1, Side::Sell, 1_000, 1_000)];
        let mut sol = solution(vec![trade(1, 1_000, 3_000)]);
        cost(&rent, &prices)
            .apply(&mut sol, &self::orders(&orders), 1_000)
            .unwrap();
        assert_eq!(sol.trades[0].executed_sell, 1_000);
        assert_eq!(sol.trades[0].executed_buy, 1_000);
        assert_eq!(sol.trades[0].solver_fee, 2_000);
    }

    #[test]
    fn a_buy_order_pulls_more() {
        let (rent, prices) = (HashMap::new(), prices());
        let orders = [order(1, Side::Buy, 2_000, 1_000)];
        let mut sol = solution(vec![trade(1, 1_000, 1_000)]);
        cost(&rent, &prices)
            .apply(&mut sol, &self::orders(&orders), 1_000)
            .unwrap();
        assert_eq!(sol.trades[0].executed_sell, 1_500);
        assert_eq!(sol.trades[0].executed_buy, 1_000);
        assert_eq!(sol.trades[0].solver_fee, 500);
    }

    #[test]
    fn nothing_to_cut_leaves_the_fill_as_is() {
        let (rent, prices) = (HashMap::new(), prices());
        let orders = [order(1, Side::Sell, 1_000, 1_000)];
        let mut sol = solution(vec![trade(1, 1_000, 1_000)]);
        cost(&rent, &prices)
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

    #[test]
    fn a_fill_at_the_tightened_limit_survives_the_cut() {
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

    #[test]
    fn a_trade_for_no_order_is_rejected() {
        let (rent, prices) = (HashMap::new(), prices());
        let mut sol = solution(vec![trade(9, 1_000, 3_000)]);
        assert_eq!(
            cost(&rent, &prices).apply(&mut sol, &HashMap::new(), 1_000),
            Err(Rejected::UnknownOrder(OrderUid([9; 32])))
        );
    }

    #[test]
    fn a_trade_without_a_price_is_rejected() {
        let (rent, prices) = (HashMap::new(), HashMap::new());
        let orders = [order(1, Side::Sell, 1_000, 1_000)];
        let mut sol = solution(vec![trade(1, 1_000, 3_000)]);
        assert_eq!(
            cost(&rent, &prices).apply(&mut sol, &self::orders(&orders), 1_000),
            Err(Rejected::MissingPrice(orders[0].uid))
        );
    }

    #[test]
    fn a_cut_eating_the_whole_payout_is_rejected() {
        let (rent, prices) = (HashMap::new(), prices());
        let orders = [order(1, Side::Sell, 1_000, 1)];
        let cost = cost(&rent, &prices);
        for executed_buy in [2_000, 1_000] {
            let mut sol = solution(vec![trade(1, 1_000, executed_buy)]);
            assert_eq!(
                cost.apply(&mut sol, &self::orders(&orders), 1_000),
                Err(Rejected::ZeroPayout(orders[0].uid))
            );
        }
    }

    #[test]
    fn a_sell_leg_overflowing_u64_is_rejected() {
        let (rent, prices) = (HashMap::new(), prices());
        let orders = [order(1, Side::Buy, u64::MAX, 1_000)];
        let mut sol = solution(vec![trade(1, u64::MAX, 1_000)]);
        assert_eq!(
            cost(&rent, &prices).apply(&mut sol, &self::orders(&orders), 1_000),
            Err(Rejected::Overflow(orders[0].uid))
        );
    }

    /// The first trade passes on its own; the rejection leaves it untouched.
    #[test]
    fn one_bad_trade_rejects_the_whole_solution() {
        let (rent, prices) = (HashMap::new(), prices());
        let orders = [
            order(1, Side::Sell, 1_000, 1_000),
            order(2, Side::Sell, 1_000, 1_000),
        ];
        let mut sol = solution(vec![trade(1, 1_000, 3_000), trade(2, 1_000, 1_500)]);
        assert_eq!(
            cost(&rent, &prices).apply(&mut sol, &self::orders(&orders), 1_000),
            Err(Rejected::LimitPrice(orders[1].uid))
        );
        assert_eq!(sol.trades[0].executed_buy, 3_000);
        assert_eq!(sol.trades[0].solver_fee, 0);
    }
}
