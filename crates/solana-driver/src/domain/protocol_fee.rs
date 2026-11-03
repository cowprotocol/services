//! Protocol fees on fills.
//!
//! The autopilot attaches fee policies to an auction order, and the driver
//! takes them from every fill of it in the surplus token: a sell order
//! delivers less buy token, a buy order pulls more sell token. They apply
//! after the solver fee and the execution cost cut, in list order, so the
//! autopilot recovers each fee from the settled legs alone, see
//! [`apply_protocol_fees`]. A solution with a fill that cannot absorb its
//! fees within the order's signed limit is dropped whole, like one that
//! cannot absorb the solver fee.
//!
//! Only a volume policy moves the limit leg sent to the engine, see
//! [`tighten_limit`]: a surplus kind takes a share of the surplus and cannot
//! cross the limit.

use {
    super::{
        Order,
        Side,
        order_uid::OrderUid,
        settlement::respects_limit,
        solution::{Solution, Trade},
    },
    chain_types::{
        Amount,
        MathError,
        solana::{IntentHash, Pubkey as ChainPubkey, Solana},
    },
    protocol_fees::apply_protocol_fees,
    std::collections::HashMap,
};

pub type FeePolicy = protocol_fees::FeePolicy<Solana>;

/// Each auction order's policies by uid. An order without policies is absent.
pub type FeePolicies = HashMap<OrderUid, Vec<FeePolicy>>;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Rejected {
    #[error("trade {0} references no auction order")]
    UnknownOrder(OrderUid),
    #[error("trade {order} cannot take its protocol fees: {source}")]
    Math { order: OrderUid, source: MathError },
    #[error("trade {0} undercuts its limit price after the protocol fees")]
    LimitPrice(OrderUid),
}

/// Takes each order's policies from every trade of `solution`. One failing
/// trade rejects the whole solution, left untouched.
pub fn apply(
    policies: &FeePolicies,
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
            let policies = policies.get(&order.uid).map_or(&[][..], Vec::as_slice);
            apply_to_trade(policies, trade, order)
        })
        .collect::<Result<Vec<_>, _>>()?;
    solution.trades = trades;
    Ok(())
}

fn apply_to_trade(policies: &[FeePolicy], trade: &Trade, order: &Order) -> Result<Trade, Rejected> {
    let mut fill = fill(order, trade);
    let fee = apply_protocol_fees(&mut fill, policies).map_err(|source| Rejected::Math {
        order: order.uid,
        source,
    })?;
    let trade = Trade {
        executed_sell: fill.executed_sell,
        executed_buy: fill.executed_buy,
        protocol_fee: fee,
        ..trade.clone()
    };
    if !respects_limit(order, trade.executed_sell, trade.executed_buy) {
        return Err(Rejected::LimitPrice(order.uid));
    }
    Ok(trade)
}

/// The fill `trade` makes of `order`, in the chain-generic vocabulary.
fn fill(order: &Order, trade: &Trade) -> protocol_fees::Order<Solana> {
    protocol_fees::Order {
        uid: IntentHash(order.uid.0),
        sell_token: ChainPubkey(order.sell_token.to_bytes()),
        buy_token: ChainPubkey(order.buy_token.to_bytes()),
        sell_amount: order.sell_amount,
        buy_amount: order.buy_amount,
        executed_sell: trade.executed_sell,
        executed_buy: trade.executed_buy,
        side: match order.side {
            Side::Sell => protocol_fees::Side::Sell,
            Side::Buy => protocol_fees::Side::Buy,
        },
    }
}

/// The limit leg the engine has to beat for its fill to respect `limit` once
/// the volume fees among `policies` are taken: a sell order's minimum buy
/// rises, a buy order's maximum sell falls. The fill meets the last policy
/// last, so the last policy tightens first. A minimum buy past `u64::MAX`
/// saturates: no fill can meet it.
pub fn tighten_limit(policies: &[FeePolicy], side: Side, limit: u64) -> u64 {
    policies
        .iter()
        .rev()
        .filter_map(|policy| match policy {
            FeePolicy::Volume { factor } => Some(*factor),
            FeePolicy::Surplus { .. } | FeePolicy::PriceImprovement { .. } => None,
        })
        .fold(limit, |limit, factor| tighten(side, factor, limit))
}

/// The boundary leg under one volume `factor`: the least buy leg whose
/// remainder after the fee reaches `limit`, or the greatest sell leg that
/// stays within `limit` with the fee on top. The fee rounds down, so the
/// boundary is settled against the exact fee by stepping from an f64
/// estimate. Exact up to `2^53` atoms, past which the fee itself rounds the
/// leg and the boundary can be off by a few atoms.
fn tighten(side: Side, factor: f64, limit: u64) -> u64 {
    let fee = |leg: u64| leg.try_mul_f64(factor).unwrap_or(u64::MAX);
    match side {
        Side::Sell => {
            let fits = |leg: u64| leg.checked_sub(fee(leg)).is_some_and(|left| left >= limit);
            let mut leg = (limit as f64 / (1.0 - factor)).ceil() as u64;
            while leg > 0 && fits(leg - 1) {
                leg -= 1;
            }
            while leg < u64::MAX && !fits(leg) {
                leg += 1;
            }
            leg
        }
        Side::Buy => {
            let fits = |leg: u64| {
                leg.checked_add(fee(leg))
                    .is_some_and(|total| total <= limit)
            };
            let mut leg = (limit as f64 / (1.0 + factor)).floor() as u64;
            while leg < u64::MAX && fits(leg + 1) {
                leg += 1;
            }
            while leg > 0 && !fits(leg) {
                leg -= 1;
            }
            leg
        }
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
            receiver: None,
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
            protocol_fee: 0,
        }
    }

    fn solution(executed_sell: u64, executed_buy: u64) -> Solution {
        Solution {
            id: 1,
            solver: pubkey(0x99),
            prices: HashMap::from([(pubkey(0x33), nz(2_000)), (pubkey(0x44), nz(1_000))]),
            trades: vec![trade(8, executed_sell, executed_buy)],
            interactions: vec![],
            cu_estimate: None,
        }
    }

    fn orders(order: &Order) -> HashMap<OrderUid, &Order> {
        HashMap::from([(order.uid, order)])
    }

    fn policies(order: &Order, policies: &[FeePolicy]) -> FeePolicies {
        HashMap::from([(order.uid, policies.to_vec())])
    }

    fn volume(factor: f64) -> FeePolicy {
        FeePolicy::Volume { factor }
    }

    fn legs(solution: &Solution) -> (u64, u64, u64) {
        let trade = &solution.trades[0];
        (trade.executed_sell, trade.executed_buy, trade.protocol_fee)
    }

    #[test]
    fn no_policy_is_identity() {
        let order = order(Side::Sell, 1_000, 1_000);
        let mut sol = solution(1_000, 2_000);
        apply(&HashMap::new(), &mut sol, &orders(&order)).unwrap();
        assert_eq!(legs(&sol), (1_000, 2_000, 0));
    }

    #[test]
    fn sell_fill_pays_the_volume_fee_in_buy_token() {
        let order = order(Side::Sell, 1_000, 1_000);
        let mut sol = solution(1_000, 2_000);
        apply(
            &policies(&order, &[volume(0.001)]),
            &mut sol,
            &orders(&order),
        )
        .unwrap();
        assert_eq!(legs(&sol), (1_000, 1_998, 2));
    }

    /// Surplus 200 at a factor of one half is 100, the volume cap at 1% of
    /// the 1000 sold is 10.
    #[test]
    fn buy_fill_pays_the_surplus_fee_in_sell_token() {
        let order = order(Side::Buy, 1_200, 1_000);
        let mut sol = solution(1_000, 1_000);
        let surplus = FeePolicy::Surplus {
            factor: 0.5,
            max_volume_factor: 0.01,
        };
        apply(&policies(&order, &[surplus]), &mut sol, &orders(&order)).unwrap();
        assert_eq!(legs(&sol), (1_010, 1_000, 10));
    }

    #[test]
    fn buy_fill_overflowing_its_sell_leg_rejects_the_solution() {
        let order = order(Side::Buy, u64::MAX, 1_000);
        let mut sol = solution(u64::MAX, 1_000);
        let err = apply(&policies(&order, &[volume(0.5)]), &mut sol, &orders(&order)).unwrap_err();
        assert_eq!(
            err,
            Rejected::Math {
                order: order.uid,
                source: MathError::Overflow,
            }
        );
    }

    #[test]
    fn rejects_whole_solution_on_one_bad_trade() {
        let first = order(Side::Sell, 1_000, 1_000);
        let mut second = first.clone();
        second.uid = OrderUid([9; 32]);
        let mut sol = solution(1_000, 2_000);
        sol.trades.push(trade(9, 1_000, 1_100));
        let mut orders = orders(&first);
        orders.insert(second.uid, &second);
        let mut policies = policies(&first, &[volume(0.5)]);
        policies.insert(second.uid, vec![volume(0.5)]);

        let err = apply(&policies, &mut sol, &orders).unwrap_err();

        assert_eq!(err, Rejected::LimitPrice(second.uid));
        assert_eq!(legs(&sol), (1_000, 2_000, 0));
    }

    #[test]
    fn only_a_volume_policy_moves_the_limit() {
        let surplus = FeePolicy::Surplus {
            factor: 0.5,
            max_volume_factor: 0.5,
        };
        assert_eq!(tighten_limit(&[], Side::Sell, 1_000), 1_000);
        assert_eq!(tighten_limit(&[surplus], Side::Sell, 1_000), 1_000);
        assert_eq!(tighten_limit(&[surplus], Side::Buy, 1_000), 1_000);
        assert_eq!(tighten_limit(&[volume(0.0)], Side::Sell, 1_000), 1_000);
        // 1000 / 0.9 = 1111.1 and 1000 / 1.1 = 909.1, each settled against
        // the fee rounding down: 1111 - 111 reaches 1000, 909 + 90 stays
        // within it.
        assert_eq!(tighten_limit(&[volume(0.1)], Side::Sell, 1_000), 1_111);
        assert_eq!(tighten_limit(&[volume(0.1)], Side::Buy, 1_000), 909);
        assert_eq!(tighten_limit(&[volume(0.1)], Side::Sell, 0), 0);
    }

    /// The fill meets the last policy last, so it tightens the limit first:
    /// 1000 under 20% needs 1249, and 1249 under 10% needs 1387. The other
    /// order would need 1388.
    #[test]
    fn later_policies_tighten_first() {
        let policies = [volume(0.1), volume(0.2)];
        assert_eq!(tighten_limit(&policies, Side::Sell, 1_000), 1_387);
        assert_eq!(
            tighten_limit(&policies, Side::Sell, 1_000),
            tighten_limit(
                &policies[..1],
                Side::Sell,
                tighten_limit(&policies[1..], Side::Sell, 1_000)
            )
        );
    }

    #[test]
    fn unrepresentable_minimum_buy_saturates() {
        assert_eq!(
            tighten_limit(&[volume(0.5)], Side::Sell, u64::MAX),
            u64::MAX
        );
    }

    #[test]
    fn quote_placeholder_maximum_sell_fits_u64() {
        let max_sell = tighten_limit(&[volume(0.5)], Side::Buy, u64::MAX);
        let fee = max_sell.try_mul_f64(0.5).unwrap();
        assert!(max_sell.checked_add(fee).is_some());
    }

    /// A fill at exactly the tightened limit survives the fees and one unit
    /// worse does not, on both sides, across factors and limit magnitudes.
    #[test]
    fn tightened_limit_is_the_exact_boundary_of_apply() {
        for factor in [0.0001, 0.001, 0.05, 0.25, 0.5] {
            let volume = [volume(factor)];
            for limit in [1u64, 3, 1_000, 12_345, 1_000_000_000_000] {
                let sell = order(Side::Sell, 1_000, limit);
                let fees = policies(&sell, &volume);
                let min_buy = tighten_limit(&volume, Side::Sell, limit);
                assert!(
                    apply(&fees, &mut solution(1_000, min_buy), &orders(&sell)).is_ok(),
                    "sell factor={factor} limit={limit}"
                );
                assert!(
                    apply(&fees, &mut solution(1_000, min_buy - 1), &orders(&sell)).is_err(),
                    "sell factor={factor} limit={limit}"
                );

                let buy = order(Side::Buy, limit, 1_000);
                let fees = policies(&buy, &volume);
                let max_sell = tighten_limit(&volume, Side::Buy, limit);
                assert!(
                    apply(&fees, &mut solution(max_sell, 1_000), &orders(&buy)).is_ok(),
                    "buy factor={factor} limit={limit}"
                );
                assert!(
                    apply(&fees, &mut solution(max_sell + 1, 1_000), &orders(&buy)).is_err(),
                    "buy factor={factor} limit={limit}"
                );
            }
        }
    }
}
