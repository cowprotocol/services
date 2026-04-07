# Pod Network Integration — Handoff

> Branch: `aryan/pod-network-integration` (PR [#4205](https://github.com/cowprotocol/services/pull/4205))
> Tip at handoff: `12f486d78` (`lint: fixed failing linter job`)
> Diff vs `main`: **33 files changed, +2281 / −127**

---

## 1. State of the PR right now

- Branch is rebased onto `main` and rebuilt cleanly.
- Reviewer feedback from José Duarte (struct-vs-getter-trait, "is there a better way", file-by-file simplification) has been addressed in two refactor passes that have been merged into the branch tip.
- All CI checks except the merge gate are green:

```
just fmt --check         clean
just clippy              clean (-D warnings, all-features, all-targets, --locked)
just fmt-toml --check    clean
test-driver              pass
test-db                  pass
test-local-node          pass
test-forked-node         pass
unit-tests               pass
doc-tests                pass
generated-contracts      pass
openapi                  pass
cargo-audit / trivy      pass
```

- Status block on the PR page: **"Review required — Code owner review required by reviewers with write access"**, "Waiting on code owner review from `cowprotocol/backend`". Nothing technical is blocking merge.

### `e2e pod_*` tests
Three ignored e2e tests live in `crates/e2e/tests/e2e/pod.rs` and require live pod-network connectivity. They are run manually via the helper recipe:

```bash
just test-pod
# = cargo nextest run -p e2e 'pod_' --test-threads 1 --failure-output final --run-ignored ignored-only
```

Last verified locally (Apr 26, 2026, after pod RPC was restored):

```
PASS  pod_test_basic
PASS  pod_test_multi_order
PASS  pod_test_multi_solver
3 passed, 108 skipped
```

Pod's JSON-RPC at `cow.pod.network:11600` has gone down once during this work. Symptom: bid submit / fetch panic with `Connection refused (os error 61)`. That is **not** a code issue — confirm with `nc -z cow.pod.network 11600` before debugging.

---

## 2. What this PR does (1 minute version)

The driver gains a parallel, fire-and-forget "shadow" path. After it scores its own best solution for a given auction, it:

1. Submits that solution as a bid to a pod-network auction contract (with locked-account auto-recovery).
2. After the deadline, fetches every participant's bid back from pod.
3. Runs a local copy of the same `winner-selection` arbitration over those bids.
4. Logs winners / non-winners. **Never** mutates or delays the response to autopilot.

Everything happens inside one `tokio::spawn` in `Competition::solve()` — if pod is unconfigured, no task is spawned and there is zero overhead.

---

## 3. Where things live (review order)

| Path | Purpose |
|---|---|
| `crates/winner-selection/src/solution_hash.rs` | Deterministic `keccak256` over `(solution_id, solver, sorted orders, sorted prices)`. Tie-breaker so all observers process bids in the same order. Tests are inline (`#[cfg(test)] mod tests`). |
| `crates/driver/src/infra/pod/config.rs` | `[pod]` TOML: `endpoint`, `auction-contract-address`. Pod is opt-in. |
| `crates/driver/src/infra/pod/recovery.rs` | Pod's locked-account recovery: `pod_getRecoveryTargetTx` RPC → `recover(tx_hash, nonce)` on the `0x50d...0003` precompile (compile-time constant via `address!` macro). Returns `Result<Option<RecoveryTarget>>`. |
| `crates/driver/src/infra/solver/mod.rs` | `Solver::pod()`, `arbitrator()`, `try_build_pod_provider()` (`anyhow::Result` flavor wrapped by `build_pod_provider`). Balance/nonce fetching is best-effort. |
| `crates/driver/src/domain/competition/mod.rs` | `solve()` spawns `pod_flow` → `pod_submit_bid` → `submit_bid_with_recovery` → `pod_fetch_bids` → `pod_local_arbitration`. All fire-and-forget; failures are `tracing::warn!`/`error!` only. |
| `crates/driver/src/domain/competition/solver_winner_selection.rs` | Driver-side `SolverArbitrator` wrapper around `winner_selection::Arbitrator`. `Bid<Unscored>` / `Bid<Scored>` typestate. |
| `crates/driver/src/infra/api/routes/solve/dto/solve_response.rs` | `Solution::as_hashable()` and `TradedOrder::as_hashable()` — what the driver serializes into a pod bid. |
| `crates/e2e/tests/e2e/pod.rs` | `pod_test_basic`, `pod_test_multi_order`, `pod_test_multi_solver`. |

### "Is this safe?" sniff test
- Exactly one `tokio::spawn` in `competition/mod.rs` (the pod block). Confirms shadow mode cannot block the main response.
- `.await` calls inside `solve()` after the pod block: only `resimulate_until_revert` (existing main-side code, unchanged).
- No `pod` references in `crates/driver/src/infra/api/` outside `solve_response.rs::as_hashable`.

---

## 4. Key design decisions worth knowing

- **Best-only bid.** Main now returns `Vec<Solved>` (multi-solution proposals). Pod still gets only `scored.first()` because each pod account can submit **one** bid per auction.
- **Solution hashing.** `winner_selection::solution_hash::hash_solution` is what makes independent observers agree on tie-break order regardless of bid arrival order.
- **Locked-account recovery.** Triggers on `error.contains("Another transaction") && error.contains("is still pending")`. Bounded to one retry. If recovery itself fails, the bid is silently dropped — by design, never retry-loop into the deadline.
- **Malformed bids are skipped, not fatal.** A hostile or buggy bidder cannot break local arbitration for the rest. Counted and logged.
- **WETH address.** Currently looked up via `eth.contracts().weth_address()` — chain-aware, no longer hardcoded.

---

## 5. Known follow-ups / out of scope

1. **`max_winners = 10`** is a hardcoded constant in `Solver::try_new`'s `SolverArbitrator::new(10, ...)`. Fine for shadow mode, could be config-driven later.
2. **No Prometheus metrics yet.** Pod-side observability is tracing-only (`pod_flow`, `pod_submit_bid`, `pod_fetch_bids`, `pod_local_arbitration` spans). Adding metrics for bid-submission success rate and arbitration agreement is a natural follow-up but intentionally out of this PR.
3. **`crates/winner-selection`** is a new crate. It now has `[lints] workspace = true`, so it inherits the workspace-level `clippy::cast_possible_wrap = "deny"`.

---

## 6. Lint pitfall worth remembering

CI runs **`just clippy`** with a newer rustc than my local toolchain (CI on 1.95, local on 1.94). The most recent failure was `clippy::unnecessary_sort_by` firing on two `sort_by(|(a,_),(b,_)| a.cmp(b))` lines in `solution_hash.rs`. Fixed by switching to `sort_by_key(|(uid, _)| *uid)`. **Always** run `just clippy` (which uses `--locked --all-features --all-targets -- -D warnings`) before pushing — `cargo clippy` alone is not enough.

If you hit a Rust **internal compiler error** during local `cargo check -p driver`, it is the incremental cache. Clear with `CARGO_INCREMENTAL=0 cargo check -p driver`. CI does not hit this (clean checkout).

---

## 7. Quick verification recipe

```bash
# 1. Lint, exactly what CI runs
just fmt --check && just clippy && just fmt-toml --check

# 2. Unit tests for the touched crates
cargo nextest run -p winner-selection -p driver -p autopilot

# 3. Pod e2e (needs pod RPC reachable on cow.pod.network:11600)
just test-pod
```

---

## 8. Useful commits at the tip

```
12f486d78 lint: fixed failing linter job
83054b4c8 refactoring, add order-quoting config section with baseline price estimation driver
7b1551063 fix: wrap score_value in vec for SolveResponse::new to match updated signature
```

The earlier 30+ commits (full history at `git log origin/main..HEAD`) tell the story of: initial pod integration → account recovery → solution hashing → e2e tests → rebase → review-feedback refactor passes → lint fix.

---

## 9. Open questions for the next session

1. Should we land the metrics follow-up in this PR or as a separate one?
2. Should `max_winners` be moved to config now or later?
3. Do we want the pod e2e tests to opt out gracefully when `cow.pod.network:11600` is unreachable (so local dev is not gated on pod liveness), or keep them strict?
4. Does the team want this PR squashed before merge, or merged as-is so individual refactor commits stay reviewable?
