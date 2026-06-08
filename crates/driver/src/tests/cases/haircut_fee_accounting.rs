//! Tests pinning down how the haircut relates to volume protocol fees.
//!
//! A volume fee and a haircut withhold value from the user through the *same*
//! make-room (limit tightening) and the same post-hoc adjustment of the
//! solver's bid, but they are accounted differently:
//!
//! - a volume fee is captured as protocol revenue and therefore enters the
//!   solution `score` (score = user surplus + protocol fees), while
//! - a haircut is pure conservative bidding: it lowers the reported execution
//!   but is *not* a fee, so it must never enter the score.
//!
//! These tests lock that asymmetry, which is the load-bearing invariant now
//! that the haircut is implemented as a non-scoring volume fee (it reuses the
//! volume-fee make-room and post-processing but carries
//! `contributes_to_score = false`). See the
//! `volume_protocol_fee_*_at_limit_price` cases in [`super::protocol_fees`] and
//! the make-room cases in [`super::haircut_pre_processing`] for the
//! single-mechanism precedents.

use {
    crate::{
        domain::competition::order,
        infra::config::file::FeeHandler,
        tests::{
            self,
            cases::EtherExt,
            setup::{
                ExpectedOrderAmounts,
                Test,
                ab_adjusted_pool,
                ab_liquidity_quote,
                ab_order,
                ab_solution,
                fee::Policy,
                test_solver,
            },
        },
    },
    eth_domain_types as eth,
    number::testing::ApproxEq,
};

/// Tiny constant network fee: keeps the order off the StaticFee path while
/// staying small enough not to disturb the at-limit math (see the note in
/// [`super::haircut_pre_processing`]).
const SOLVER_FEE: u64 = 100;

/// Score noise tolerance (wei) when the expected score is zero, where
/// `is_approx_eq` cannot be used (it divides by the expected value).
const ZERO_SCORE_TOLERANCE: u64 = 1_000_000;

/// Drives the identical solver execution (sell 50 -> buy 50, landing the user
/// on their signed buy limit of 40) through two configurations that withhold
/// the same 20% of the buy volume:
///   1. a 20% **volume protocol fee**, and
///   2. a 2000 bps (20%) **haircut**.
///
/// The user-facing amounts are identical in both (sell 50, buy 40), but the
/// volume fee is booked as protocol revenue and shows up in the score (~10
/// ETH), whereas the haircut is not revenue and the score is ~0. This is the
/// accounting guardrail: a haircut must never be counted as captured fee.
#[tokio::test]
#[ignore]
async fn haircut_is_not_booked_as_protocol_revenue() {
    // Both configurations tighten the signed buy limit (40) to the same value
    // the solver sees: 40 / (1 - 0.2) = 50. The solver clears there and the
    // driver brings the reported buy back down to the signed 40.
    let solver = Amounts {
        sell: 50u64.ether().into_wei(),
        buy: 50u64.ether().into_wei(),
    };
    let driver = Amounts {
        sell: 50u64.ether().into_wei(),
        buy: 40u64.ether().into_wei(),
    };

    let volume_score = run_sell_order(SellOrderCase {
        name: "accounting: 20% volume fee",
        fee_policy: vec![Policy::Volume { factor: 0.2 }],
        haircut_bps: 0,
        solver: &solver,
        driver: &driver,
    })
    .await;

    let haircut_score = run_sell_order(SellOrderCase {
        name: "accounting: 2000 bps haircut",
        fee_policy: vec![],
        haircut_bps: 2000,
        solver: &solver,
        driver: &driver,
    })
    .await;

    // The volume fee is captured as protocol revenue: 20% of the 50 ETH buy
    // volume = 10 ETH, all of which lands in the score (the user is at their
    // limit, so there is no surplus component).
    assert!(
        volume_score.is_approx_eq(&10u64.ether().into_wei(), None),
        "volume-fee score {volume_score} should be ~10 ETH (the captured fee)",
    );

    // The haircut withholds the same 10 ETH of value from the user, but it is
    // not a fee, so the score must be ~0.
    assert!(
        haircut_score < eth::U256::from(ZERO_SCORE_TOLERANCE),
        "haircut score {haircut_score} should be ~0 (a haircut is not protocol revenue and must \
         not enter the score)",
    );
}

/// A single sell order that carries BOTH a 20% volume protocol fee AND a 2000
/// bps haircut. This proves the two make-room steps compose (the solver bids at
/// the doubly-tightened limit `40 / ((1 - 0.2) * (1 - 0.2)) = 62.5` and the
/// driver still lands the user on the signed limit of 40 after applying both),
/// and that only the volume fee — not the haircut — is counted in the score.
///
/// Without the haircut make-room added in #4417 the solver would bid against
/// the volume-only limit (50) and the post-hoc haircut would push the reported
/// buy below the signed limit, reverting on-chain. Landing exactly on 40 here
/// is the regression guard for that.
///
/// Of the 22.5 ETH withheld from the gross solver buy (62.5 → 40), the protocol
/// volume fee takes its full 20% of the gross (12.5 ETH, booked into the score)
/// and the haircut is the 10 ETH residual (excluded from the score).
#[tokio::test]
#[ignore]
async fn volume_fee_and_haircut_compose_on_sell_order() {
    let score = run_sell_order(SellOrderCase {
        name: "combined: 20% volume fee + 2000 bps haircut",
        fee_policy: vec![Policy::Volume { factor: 0.2 }],
        haircut_bps: 2000,
        // Solver clears at the doubly-tightened limit: 40 / (0.8 * 0.8) = 62.5.
        solver: &Amounts {
            sell: 50u64.ether().into_wei(),
            buy: 62.5f64.ether().into_wei(),
        },
        // After the volume fee and the haircut the user lands on the signed
        // limit (sell 50, buy 40).
        driver: &Amounts {
            sell: 50u64.ether().into_wei(),
            buy: 40u64.ether().into_wei(),
        },
    })
    .await;

    // Only the volume fee enters the score: 20% of the gross solver buy volume
    // (0.2 * 62.5 = 12.5 ETH). The haircut residual (10 ETH) is excluded — if it
    // were wrongly booked, the score would be ~22.5 ETH instead.
    assert!(
        score.is_approx_eq(&12.5f64.ether().into_wei(), None),
        "combined score {score} should be ~12.5 ETH (only the volume fee is booked; the haircut \
         must be excluded)",
    );
}

struct Amounts {
    sell: eth::U256,
    buy: eth::U256,
}

struct SellOrderCase<'a> {
    name: &'a str,
    fee_policy: Vec<Policy>,
    haircut_bps: u32,
    solver: &'a Amounts,
    driver: &'a Amounts,
}

/// Sets up a fill-or-kill sell order whose pool clears at the solver's bid,
/// runs the solve and asserts the driver-reported amounts match `driver`.
/// Returns the solution score for the caller to assert on.
async fn run_sell_order(case: SellOrderCase<'_>) -> eth::U256 {
    let solver_fee = eth::U256::from(SOLVER_FEE);
    let quote = ab_liquidity_quote()
        .sell_amount(case.solver.sell)
        .buy_amount(case.solver.buy);
    let pool = ab_adjusted_pool(quote);

    let order = ab_order()
        .kind(order::Kind::Limit)
        .side(order::Side::Sell)
        .sell_amount(50u64.ether().into_wei())
        .buy_amount(40u64.ether().into_wei())
        .solver_fee(Some(solver_fee))
        .fee_policy(case.fee_policy)
        .executed(Some(case.solver.sell - solver_fee))
        .no_surplus()
        .expected_amounts(ExpectedOrderAmounts {
            sell: case.driver.sell,
            buy: case.driver.buy,
        });

    let test: Test = tests::setup()
        .name(case.name.to_owned())
        .pool(pool)
        .order(order.clone())
        .solution(ab_solution())
        .solvers(vec![
            test_solver()
                .fee_handler(FeeHandler::Driver)
                .haircut_bps(case.haircut_bps),
        ])
        .done()
        .await;

    let result = test.solve().await.ok();
    let score = result.score();
    result.orders(&[order]);
    score
}
