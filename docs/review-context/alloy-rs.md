# Review Context — Alloy-rs Usage

Loaded by `COW_PR_REVIEW_SKILL.md §3` when a PR touches `crates/ethrpc/`, `crates/chain/`, `crates/contracts/`, or adds `alloy::*` / `alloy_*` imports.

**External reference:** https://alloy.rs/introduction/prompting (AI-optimized alloy guide). Fetch it once per session when actually reviewing alloy changes.

## High-signal review checks

1. **Provider reuse — and pass by reference.** Alloy's `Provider` is clonable and meant to be cloned, not rebuilt. Any new `ProviderBuilder::new().connect_http(...)` in a hot path (auction loop, per-order path, per-request handler) is a **Medium** finding — it should reuse an existing provider.

   Additionally, alloy's contract `Instance` constructors accept `IntoProvider`, which is implemented for both owned providers *and references*. Pass by reference when the caller retains ownership:
   ```rust
   // ❌ unnecessary clone
   let vault = IERC4626::new(token, self.provider.clone());
   // ✅ pass by reference
   let vault = IERC4626::new(token, &self.provider);
   ```
   Gratuitous `.clone()` on providers is a **Small (QoL)** finding — and a good candidate for a GitHub `Suggested change` block (mechanical fix, zero-risk).

2. **Batched RPC.** CoW's `ethrpc` crate adds batching on top of alloy. New direct RPC calls that bypass the batching layer can hit rate limits under auction load. Flag any direct `provider.get_*` calls in hot paths that aren't going through `ethrpc::Web3` — **Medium**.

3. **Distinguish contract-revert from transport/network error.** This is the single most CoW-specific alloy gotcha. When code runs a `call()` against an arbitrary on-chain contract and interprets the `Err`, the error has categories:

   - **Contract revert** (valid ERC-20 returned an error, `asset()` not implemented, etc.) → deterministic. Safe to cache "this token is X".
   - **Network / transport error** (RPC timeout, 5xx from the node, connection reset) → transient. **Must not be cached** — a transient failure at cache-write time would permanently mis-classify the token until restart.
   - **Decoding error** (contract returned bytes that didn't decode into the expected type) → treat as contract-level; usually means the contract at that address doesn't implement the interface.

   The `ethrpc::alloy::errors::ContractErrorExt` trait exposes `.is_contract_error()` for this split. Whenever a diff has `Err(_)` in a `call()` result's match arm and then *caches* a verdict, verify that the match is gated on `is_contract_error()`. This is a **High** finding if missing — a single network blip could persistently mark a valid vault as non-vault.

3. **Decoding errors.** Alloy's `sol!` macro emits `Result` types that should be surfaced with context, not `.unwrap()`. An `.unwrap()` on `decode()` output in a code path that handles arbitrary on-chain data is **High** — malformed returns from an uncooperative contract should not panic the auction loop.

4. **Gas estimation.** Avoid `provider.estimate_gas` inside the settlement path. Gas estimation hits the node and can be slow or flaky. Use the precomputed gas values from the solver solution instead. Inline gas estimation in the critical settlement path → **High**; outside it → **Medium** with a suggestion to cache.

5. **Block number or tag.** When reading chain state, verify the block tag is correct for the context:
   - Auction-time reads → the auction's block (explicit tag, not `latest`).
   - Simulation → `latest` or `pending` depending on purpose.
   - Settlement submission → varies by private-RPC strategy; follow existing patterns in the same file.

   A blind `BlockNumberOrTag::Latest` in code that should use the auction block is **High** (inconsistent with the auction's view of the world → score mismatches).

6. **Ethers-rs migration artifacts.** Services is mid-migration from `ethers-rs` to `alloy`. Mixing primitive types across the boundary (e.g. `ethers::H160` passed to a function expecting `alloy::primitives::Address`) is **Medium** — each layer should have a single primitive type. Look for ad-hoc conversions that hint at a leak across the boundary.

7. **Primitives vs. types.** `alloy::primitives::U256` vs. `alloy::sol_types::sol!`-generated types — verify arithmetic is done on the primitives and the sol! types are used only at contract boundaries. Arithmetic on sol! types inside business logic is **Small** at minimum, **Medium** if it obscures overflow handling.

8. **`join!` vs `try_join!` with branch logic.** When code needs the result of *both* futures before deciding what the outcome means (classic pattern: `asset()` + `decimals()` on an unknown token, where the *combination* of successes/failures carries meaning), `tokio::join!` is correct even though `try_join!` looks shorter. `try_join!` short-circuits on the first error, collapsing four outcomes into two — which is wrong for classification code.

   When reviewing: if you see `tokio::join!` where `try_join!` would be briefer, check whether the code's match arms depend on *which combination* of results errored. If yes, it's correct — don't suggest `try_join!`. If no, suggest `try_join!` as a simplification, framed as a question (*"Can this use `try_join!`?"*).

9. **Generated `sol!` bindings vs inline `alloy::sol!`.** CoW prefers generated contract bindings (via the `contracts` crate) over inline `alloy::sol! { #[sol(rpc)] interface X { ... } }` blocks in business code. Generated bindings avoid per-crate compile cost and let `Cmd+Click` jump to the expanded source. An inline `sol!` macro in a non-test business file is **Small** with an Action to move it to the generated-bindings path. (Test code is a softer rule — inline for quick tests is fine.)

## Not findings

- Style choices in how `sol!` is invoked (macro-vs-file).
- Which alloy sub-crate was imported directly vs. through `alloy::*` re-exports.
- Import ordering, `use` grouping.
- Whether to use `Address::ZERO` vs. `alloy::primitives::address!("0x...")` for the null address (both are fine).

These are taste / `rustfmt` concerns. The Anti-nit rule in `COW_PR_REVIEW_SKILL.md §5` forbids surfacing them.
