//! Tests exercising the one generic algorithm through both chain
//! instantiations.

use {
    crate::{
        arbitrator::Arbitrator,
        auction::AuctionContext,
        chain::ChainTypes,
        evm::{Evm, OrderUid},
        primitives::{FeePolicy, Side},
        solana::{IntentHash, Pubkey, Solana},
        solution::{Order, Solution},
        state::RankedItem,
    },
    alloy_primitives::{Address, U256},
    std::collections::HashMap,
};

fn evm_uid(digest: u8, owner: u8) -> OrderUid {
    let mut bytes = [digest; 56];
    bytes[32..52].copy_from_slice(&[owner; 20]);
    OrderUid(bytes)
}

fn sol_uid(n: u8) -> IntentHash {
    IntentHash([n; 32])
}

/// 1.0 in the EVM native-price scale (18 decimals).
const EVM_UNIT_PRICE: u64 = 1_000_000_000_000_000_000;
/// 1.0 in the Solana native-price scale (9 decimals).
const SOL_UNIT_PRICE: u64 = 1_000_000_000;

/// A sell order executing at `executed_buy` against a `sell/buy` 100/90
/// limit. Surplus over limit = executed_buy - 90.
fn sell_order<C: ChainTypes>(
    uid: C::OrderUid,
    sell_token: C::TokenId,
    buy_token: C::TokenId,
    executed_buy: u64,
    to_amount: fn(u64) -> C::Amount,
) -> Order<C> {
    Order {
        uid,
        sell_token,
        buy_token,
        sell_amount: to_amount(100),
        buy_amount: to_amount(90),
        executed_sell: to_amount(100),
        executed_buy: to_amount(executed_buy),
        side: Side::Sell,
    }
}

/// Same auction on both chains: two solutions on the same token pair, the
/// higher-surplus one wins, the other loses to the uniform directional
/// clearing price. Scores and reference scores must agree across chains.
#[test]
fn same_scenario_ranks_identically_on_both_chains() {
    // EVM run.
    let uid = evm_uid(1, 0xaa);
    let (token_a, token_b) = (Address::repeat_byte(1), Address::repeat_byte(2));
    let (solver_x, solver_y) = (Address::repeat_byte(3), Address::repeat_byte(4));
    let context = AuctionContext::<Evm> {
        fee_policies: HashMap::from([(uid, vec![])]),
        native_prices: HashMap::from([(token_b, U256::from(EVM_UNIT_PRICE))]),
        ..Default::default()
    };
    let solutions = vec![
        Solution::new(
            1,
            solver_x,
            vec![sell_order::<Evm>(uid, token_a, token_b, 95, U256::from)],
        ),
        Solution::new(
            2,
            solver_y,
            vec![sell_order::<Evm>(uid, token_a, token_b, 92, U256::from)],
        ),
    ];
    let arbitrator = Arbitrator::<Evm> {
        max_winners: 5,
        wrapped_native: Address::repeat_byte(0xff),
    };
    let evm_ranking = arbitrator.arbitrate(solutions, &context);
    let evm_reference = arbitrator.compute_reference_scores(&evm_ranking);

    // Solana run, same numbers.
    let sol_id = sol_uid(1);
    let (mint_a, mint_b) = (Pubkey([1; 32]), Pubkey([2; 32]));
    let (sol_solver_x, sol_solver_y) = (Pubkey([3; 32]), Pubkey([4; 32]));
    let context = AuctionContext::<Solana> {
        fee_policies: HashMap::from([(sol_id, vec![])]),
        native_prices: HashMap::from([(mint_b, SOL_UNIT_PRICE)]),
        ..Default::default()
    };
    let solutions = vec![
        Solution::new(
            1,
            sol_solver_x,
            vec![sell_order::<Solana>(sol_id, mint_a, mint_b, 95, |n| n)],
        ),
        Solution::new(
            2,
            sol_solver_y,
            vec![sell_order::<Solana>(sol_id, mint_a, mint_b, 92, |n| n)],
        ),
    ];
    let arbitrator = Arbitrator::<Solana> {
        max_winners: 5,
        wrapped_native: Pubkey([0xff; 32]),
    };
    let sol_ranking = arbitrator.arbitrate(solutions, &context);
    let sol_reference = arbitrator.compute_reference_scores(&sol_ranking);

    // Both chains: solution 1 wins with surplus 5, solution 2 ranks second
    // with surplus 2 and does not win (same directed pair already cleared).
    for (winner_ids, scores) in [
        (
            evm_ranking.winners().map(|s| s.id()).collect::<Vec<_>>(),
            evm_ranking
                .ranked
                .iter()
                .map(|s| u64::try_from(s.score()).unwrap())
                .collect::<Vec<_>>(),
        ),
        (
            sol_ranking.winners().map(|s| s.id()).collect::<Vec<_>>(),
            sol_ranking.ranked.iter().map(|s| s.score()).collect(),
        ),
    ] {
        assert_eq!(winner_ids, vec![1]);
        assert_eq!(scores, vec![5, 2]);
    }
    // Reference score of the winning solver: rerun without them, the losing
    // solution wins with score 2. Identical on both chains.
    assert_eq!(evm_reference[&solver_x], U256::from(2u64));
    assert_eq!(sol_reference[&sol_solver_x], 2);
}

/// Amounts whose products overflow u64 still score correctly because the
/// u64 `Amount` impl always multiplies in u128.
#[test]
fn solana_amounts_survive_u64_product_overflow() {
    let uid = sol_uid(1);
    let (token_a, token_b) = (Pubkey([1; 32]), Pubkey([2; 32]));
    let big = 1u64 << 40;
    let context = AuctionContext::<Solana> {
        fee_policies: HashMap::from([(uid, vec![])]),
        native_prices: HashMap::from([(token_b, SOL_UNIT_PRICE)]),
        ..Default::default()
    };
    let order = Order::<Solana> {
        uid,
        sell_token: token_a,
        buy_token: token_b,
        sell_amount: big,
        buy_amount: big,
        executed_sell: big,
        executed_buy: big + 1000,
        side: Side::Sell,
    };
    let solutions = vec![Solution::new(1, Pubkey([3; 32]), vec![order])];
    let arbitrator = Arbitrator::<Solana> {
        max_winners: 1,
        wrapped_native: Pubkey([0xff; 32]),
    };

    let ranking = arbitrator.arbitrate(solutions, &context);

    // Surplus math computes 2^40 * (2^40 + 1000) intermediates (~2^80).
    // A u64-only implementation would discard the solution as overflow.
    assert_eq!(
        ranking.winners().map(|s| s.id()).collect::<Vec<_>>(),
        vec![1]
    );
    assert_eq!(ranking.ranked[0].score(), 1000);
}

/// The multi-pair fairness filter: a solution covering two pairs is dropped
/// when one of its per-pair scores is beaten by a single-pair baseline.
#[test]
fn solana_unfair_multi_pair_solution_is_filtered_out() {
    let (uid_ab, uid_cd) = (sol_uid(1), sol_uid(2));
    let (token_a, token_b) = (Pubkey([1; 32]), Pubkey([2; 32]));
    let (token_c, token_d) = (Pubkey([3; 32]), Pubkey([4; 32]));
    let context = AuctionContext::<Solana> {
        fee_policies: HashMap::from([(uid_ab, vec![]), (uid_cd, vec![])]),
        native_prices: HashMap::from([(token_b, SOL_UNIT_PRICE), (token_d, SOL_UNIT_PRICE)]),
        ..Default::default()
    };
    let solutions = vec![
        // Batches both pairs, surplus 3 each.
        Solution::new(
            1,
            Pubkey([5; 32]),
            vec![
                sell_order::<Solana>(uid_ab, token_a, token_b, 93, |n| n),
                sell_order::<Solana>(uid_cd, token_c, token_d, 93, |n| n),
            ],
        ),
        // Baseline on (A, B) alone with surplus 5: proves the batch
        // shortchanges that pair.
        Solution::new(
            2,
            Pubkey([6; 32]),
            vec![sell_order::<Solana>(uid_ab, token_a, token_b, 95, |n| n)],
        ),
    ];
    let arbitrator = Arbitrator::<Solana> {
        max_winners: 5,
        wrapped_native: Pubkey([0xff; 32]),
    };

    let ranking = arbitrator.arbitrate(solutions, &context);

    assert_eq!(
        ranking
            .filtered_out
            .iter()
            .map(|s| s.id())
            .collect::<Vec<_>>(),
        vec![1]
    );
    assert_eq!(
        ranking.winners().map(|s| s.id()).collect::<Vec<_>>(),
        vec![2]
    );
}

/// Protocol fees feed the score: a 50% surplus cut on an
/// already-fee-applied trade doubles the scored surplus.
#[test]
fn evm_surplus_fee_policy_doubles_score() {
    let uid = evm_uid(1, 0xaa);
    let (token_a, token_b) = (Address::repeat_byte(1), Address::repeat_byte(2));
    let context = AuctionContext::<Evm> {
        fee_policies: HashMap::from([(
            uid,
            vec![FeePolicy::Surplus {
                factor: 0.5,
                max_volume_factor: 0.9,
            }],
        )]),
        native_prices: HashMap::from([(token_b, U256::from(EVM_UNIT_PRICE))]),
        ..Default::default()
    };
    let solutions = vec![Solution::new(
        1,
        Address::repeat_byte(3),
        vec![sell_order::<Evm>(uid, token_a, token_b, 95, U256::from)],
    )];
    let arbitrator = Arbitrator::<Evm> {
        max_winners: 1,
        wrapped_native: Address::repeat_byte(0xff),
    };

    let ranking = arbitrator.arbitrate(solutions, &context);

    // User surplus 5, fee = 5 * 0.5 / (1 - 0.5) = 5, score = 10.
    assert_eq!(ranking.ranked[0].score(), U256::from(10u64));
}

/// JIT attribution diverges by design: the EVM UID embeds the owner, the
/// Solana intent hash does not, so a JIT-owner order scores on EVM and the
/// equivalent solution dies scoreless on Solana.
#[test]
fn jit_owner_attribution_is_chain_specific() {
    let owner = 0xaa;
    let uid = evm_uid(1, owner);
    let (token_a, token_b) = (Address::repeat_byte(1), Address::repeat_byte(2));
    // Not in fee_policies: contributes only through the JIT owner allowlist.
    let context = AuctionContext::<Evm> {
        surplus_capturing_jit_order_owners: [Address::repeat_byte(owner)].into(),
        native_prices: HashMap::from([(token_b, U256::from(EVM_UNIT_PRICE))]),
        ..Default::default()
    };
    let solutions = vec![Solution::new(
        1,
        Address::repeat_byte(3),
        vec![sell_order::<Evm>(uid, token_a, token_b, 95, U256::from)],
    )];
    let arbitrator = Arbitrator::<Evm> {
        max_winners: 1,
        wrapped_native: Address::repeat_byte(0xff),
    };
    let ranking = arbitrator.arbitrate(solutions, &context);
    assert_eq!(ranking.winners().count(), 1);

    // Same shape on Solana: `uid_owner` is None, the order cannot be
    // attributed, the solution scores zero and is dropped entirely.
    let context = AuctionContext::<Solana> {
        surplus_capturing_jit_order_owners: [Pubkey([owner; 32])].into(),
        native_prices: HashMap::from([(Pubkey([2; 32]), SOL_UNIT_PRICE)]),
        ..Default::default()
    };
    let solutions = vec![Solution::new(
        1,
        Pubkey([3; 32]),
        vec![sell_order::<Solana>(
            sol_uid(1),
            Pubkey([1; 32]),
            Pubkey([2; 32]),
            95,
            |n| n,
        )],
    )];
    let arbitrator = Arbitrator::<Solana> {
        max_winners: 1,
        wrapped_native: Pubkey([0xff; 32]),
    };
    let ranking = arbitrator.arbitrate(solutions, &context);
    assert_eq!(ranking.winners().count(), 0);
    assert!(ranking.ranked.is_empty());
}
