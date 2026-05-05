# Skill — `git blame` for historic context

Use before flagging code that looks unusual, redundant, accidental, or "easy to clean up". Often that code looks weird because it had to.

## How to invoke

Two ways:

- **Procedurally** — follow the steps below when you encounter unusual-looking code in any context (PR review, order-debug session, ad-hoc *"why is this here?"*).
- **Via slash command** — `/blame-context <path>:<line>` wraps the procedure end-to-end and prints the strengthens / weakens / inverts decision. Useful when you want a one-shot answer rather than walking the procedure manually.

## Example

A reviewer sees this in `crates/driver/src/domain/competition/solution/settlement.rs:444`:

```rust
let max_gas = eth::Gas(block_limit.0 / eth::U256::from(2));
```

`/2` looks arbitrary — a reviewer might be tempted to flag it as a magic number. Run the procedure first:

```bash
git blame -L 444,444 -- crates/driver/src/domain/competition/solution/settlement.rs
# → a4ee76aae3  Felix Leupold  2024-03-18  ...

git log -1 --format='%s%n%b' a4ee76aae3
# → subject ends with `(#NNNN)`; pivot to the PR
gh pr view <NNNN> -R cowprotocol/services
# → body: "block builders' default algorithm picks tx whose gas limit
#    fits remaining space; leave headroom for inclusion."
```

Decision: **weakens** the "magic number" suspicion — the constant has documented inclusion-economics rationale. Drop the finding, or downgrade to a `Question:` confirming the rationale still applies on the chain in question.

## Procedure

```bash
git blame -L <start>,<end> -- <path>                # who/what/when

# cowprotocol/services squash-merges. Most blames point at one commit whose
# subject ends with "(#NNNN)" — extract the PR number, then pivot to the
# PR conversation, which is usually richer than the commit body alone.
git log -1 --format='%s%n%b' <sha>
gh pr view <NNNN> -R <owner>/<repo>
```

## Decision

Promote what blame reveals into the finding's Explanation, then weight the finding:

- **Strengthens** — surrounding code was added recently for a reason the diff now contradicts. Keep / raise severity.
- **Weakens** — the originating PR explains *why* the shape is unusual (deliberate workaround, perf fix, cross-version compat). Soften, or pivot from `Action:` to `Question:`. Example: a two-line guard looked over-defensive; blame showed it was a hot-patch lifted from prod logs — Medium → Question, asking whether the failure mode still applies.
- **Inverts** — flagging this would ask the author to undo a hard-won fix. Drop the finding. Example: a `Some(_) =>` arm looked redundant; blame revealed it was added months ago to swallow a panic on an edge case the diff was about to remove. Dropped, with a note that the panic is back.

## Edge cases

- **Merge commit, not squash** — rare here; inspect with `git log -1 --format='%s%n%b' <sha>` and walk parents (`<sha>^1`, `^2`) to find the commit that actually authored the line.
- **Same author as the PR under review, recent** — context is fresh; ask the author directly in the review thread instead of synthesising blame.
- **Refactor moved code wholesale** — surface blame points at the move, not the originating fix. Use `git blame -w -C -C -C -L <start>,<end> -- <path>` (whitespace-insensitive, copy-detection) to recover the real authoring commit.
- **Vendored / generated / contract-binding code** — blame the generator's input (upstream config, source `.sol`, codegen template). Skip if the surface is a JSON ABI or lockfile.

## When to skip

- Lines entirely new in the diff under review (no history yet).
- Pure additions of new symbols (nothing to blame).
- Generated code where the input lives elsewhere — blame the input instead.

## Used by

- [`COW_PR_REVIEW_SKILL.md`](../COW_PR_REVIEW_SKILL.md) §6 — before flagging unusual-looking code.
- [`COW_ORDER_DEBUG_SKILL.md`](../COW_ORDER_DEBUG_SKILL.md) — when investigating *"why is this check here?"* during order debugging.
- Ad-hoc code investigations where a line of code prompts *"this looks accidental"*.
