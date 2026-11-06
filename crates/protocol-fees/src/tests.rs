//! Tests of the forward fee math against the backward math it inverts.

use {
    crate::{
        FeePolicy,
        Order,
        Quote,
        Side,
        apply_protocol_fees,
        calculate_custom_prices_from_executed,
        protocol_fees,
    },
    chain_types::{
        MathError,
        solana::{IntentHash, Pubkey, Solana},
    },
    rand::{Rng, SeedableRng, rngs::StdRng},
};

fn sol_uid(n: u8) -> IntentHash {
    IntentHash([n; 32])
}

/// A Solana fill: the signed limit and the legs it executed at.
fn fill(
    side: Side,
    sell_amount: u64,
    buy_amount: u64,
    executed_sell: u64,
    executed_buy: u64,
) -> Order<Solana> {
    Order {
        uid: sol_uid(1),
        sell_token: Pubkey([1; 32]),
        buy_token: Pubkey([2; 32]),
        sell_amount,
        buy_amount,
        executed_sell,
        executed_buy,
        side,
    }
}

#[test]
fn volume_fee_moves_a_sell_fill_down() {
    let mut fill = fill(Side::Sell, 1_000, 1_800, 1_000, 2_000);
    let fee = apply_protocol_fees(&mut fill, &[FeePolicy::Volume { factor: 0.1 }]).unwrap();
    assert_eq!(fee, 200);
    assert_eq!((fill.executed_sell, fill.executed_buy), (1_000, 1_800));
}

#[test]
fn volume_fee_moves_a_buy_fill_up() {
    let mut fill = fill(Side::Buy, 1_200, 1_000, 1_000, 1_000);
    let fee = apply_protocol_fees(&mut fill, &[FeePolicy::Volume { factor: 0.1 }]).unwrap();
    assert_eq!(fee, 100);
    assert_eq!((fill.executed_sell, fill.executed_buy), (1_100, 1_000));
}

/// Surplus 200 at a factor of one half is 100, the volume cap at 1% of the
/// 2000 bought is 20.
#[test]
fn surplus_fee_is_capped_by_volume() {
    let mut capped = fill(Side::Sell, 1_000, 1_800, 1_000, 2_000);
    let fee = apply_protocol_fees(
        &mut capped,
        &[FeePolicy::Surplus {
            factor: 0.5,
            max_volume_factor: 0.01,
        }],
    )
    .unwrap();
    assert_eq!(fee, 20);
    assert_eq!(capped.executed_buy, 1_980);

    let mut uncapped = fill(Side::Sell, 1_000, 1_800, 1_000, 2_000);
    let fee = apply_protocol_fees(
        &mut uncapped,
        &[FeePolicy::Surplus {
            factor: 0.5,
            max_volume_factor: 0.5,
        }],
    )
    .unwrap();
    assert_eq!(fee, 100);
    assert_eq!(uncapped.executed_buy, 1_900);
}

/// The quote buys 1900 for the order's 1000, better than the 1800 limit, so
/// the improvement is 100 over the quote.
#[test]
fn price_improvement_fee_is_over_the_better_of_quote_and_limit() {
    let quote = Quote {
        sell_amount: 1_000,
        buy_amount: 1_900,
        fee: 0,
        solver: Pubkey([9; 32]),
    };
    let mut fill = fill(Side::Sell, 1_000, 1_800, 1_000, 2_000);
    let fee = apply_protocol_fees(
        &mut fill,
        &[FeePolicy::PriceImprovement {
            factor: 0.5,
            max_volume_factor: 0.5,
            quote,
        }],
    )
    .unwrap();
    assert_eq!(fee, 50);
    assert_eq!(fill.executed_buy, 1_950);
}

/// The volume fee lands the fill on its limit, so the surplus policy after it
/// takes nothing.
#[test]
fn policies_apply_in_list_order() {
    let mut fill = fill(Side::Sell, 1_000, 1_800, 1_000, 2_000);
    let fee = apply_protocol_fees(
        &mut fill,
        &[
            FeePolicy::Volume { factor: 0.1 },
            FeePolicy::Surplus {
                factor: 0.5,
                max_volume_factor: 1.0,
            },
        ],
    )
    .unwrap();
    assert_eq!(fee, 200);
    assert_eq!(fill.executed_buy, 1_800);
}

#[test]
fn a_fill_under_its_limit_fails_a_surplus_policy() {
    let mut fill = fill(Side::Sell, 1_000, 1_800, 1_000, 1_700);
    let err = apply_protocol_fees(
        &mut fill,
        &[FeePolicy::Surplus {
            factor: 0.5,
            max_volume_factor: 0.5,
        }],
    )
    .unwrap_err();
    assert_eq!(err, MathError::Negative);
}

/// The volume fee pushes the fill under its limit, so the surplus policy after
/// it fails, and the fill keeps the legs it came with.
#[test]
fn a_failed_policy_leaves_the_fill_untouched() {
    let mut fill = fill(Side::Sell, 1_000, 1_800, 1_000, 2_000);
    let err = apply_protocol_fees(
        &mut fill,
        &[
            FeePolicy::Volume { factor: 0.5 },
            FeePolicy::Surplus {
                factor: 0.5,
                max_volume_factor: 0.5,
            },
        ],
    )
    .unwrap_err();
    assert_eq!(err, MathError::Negative);
    assert_eq!((fill.executed_sell, fill.executed_buy), (1_000, 2_000));
}

fn random_policy(rng: &mut StdRng) -> FeePolicy<Solana> {
    let factor = rng.random_range(0.0..=0.5);
    let max_volume_factor = rng.random_range(0.0..=0.5);
    match rng.random_range(0..3) {
        0 => FeePolicy::Volume { factor },
        1 => FeePolicy::Surplus {
            factor,
            max_volume_factor,
        },
        _ => {
            let sell_amount = rng.random_range(1..=1u64 << 48);
            FeePolicy::PriceImprovement {
                factor,
                max_volume_factor,
                quote: Quote {
                    sell_amount,
                    buy_amount: rng.random_range(1..=1u64 << 48),
                    fee: rng.random_range(0..=sell_amount / 2),
                    solver: Pubkey([9; 32]),
                },
            }
        }
    }
}

/// The driver's forward math and the autopilot's backward math agree on a
/// fill: the fees taken before settlement are what the settled legs give
/// back. Factors stay at or below one half. Above it `f / (1 - f)` passes
/// one, and a fee rounded down forward comes back more than an atom off.
/// Each policy rounds within an atom, and an earlier policy's legs are
/// rebuilt from the later one's fee, so `n` policies agree within `2^n - 1`
/// atoms.
#[test]
fn backward_math_recovers_the_forward_fees() {
    let mut rng = StdRng::seed_from_u64(0x5eed);
    for _ in 0..5_000 {
        let side = if rng.random_bool(0.5) {
            Side::Sell
        } else {
            Side::Buy
        };
        let sell_amount = rng.random_range(1..=1u64 << 48);
        let buy_amount = rng.random_range(1..=1u64 << 48);
        let surplus = rng.random_range(0..=1u64 << 47);
        let (executed_sell, executed_buy) = match side {
            Side::Sell => {
                let executed_sell = rng.random_range(1..=sell_amount);
                let at_limit = (u128::from(executed_sell) * u128::from(buy_amount))
                    .div_ceil(u128::from(sell_amount)) as u64;
                (executed_sell, at_limit + surplus)
            }
            Side::Buy => {
                let executed_buy = rng.random_range(1..=buy_amount);
                let at_limit = (u128::from(executed_buy) * u128::from(sell_amount)
                    / u128::from(buy_amount)) as u64;
                (at_limit.saturating_sub(surplus).max(1), executed_buy)
            }
        };
        let engine = fill(side, sell_amount, buy_amount, executed_sell, executed_buy);
        let policies: Vec<FeePolicy<Solana>> = (0..rng.random_range(1..=3))
            .map(|_| random_policy(&mut rng))
            .collect();

        let mut settled = engine.clone();
        let Ok(forward) = apply_protocol_fees(&mut settled, &policies) else {
            continue;
        };
        // The driver drops a fill the fees push under its limit.
        if u128::from(settled.executed_buy) * u128::from(sell_amount)
            < u128::from(settled.executed_sell) * u128::from(buy_amount)
        {
            continue;
        }

        let backward = protocol_fees(
            &settled,
            &policies,
            &calculate_custom_prices_from_executed(&settled),
        )
        .unwrap();
        let tolerance = (1u64 << policies.len()) - 1;
        assert!(
            forward.abs_diff(backward) <= tolerance,
            "{engine:?} {policies:?} forward {forward} backward {backward}"
        );
    }
}
