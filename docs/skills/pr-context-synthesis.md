# Skill — PR context synthesis

Use to produce a tight 1–3 paragraph *what / why / how* block for a single PR (or PR-shaped change). Consumed by the PR review report's CONTEXT section and by [`pr-blame-walk`](pr-blame-walk.md), which calls this skill once per candidate PR before scoring.

## How to invoke

Two ways:

- **Procedurally** — follow the rules below when you need a tight 1–3 paragraph synthesis (CONTEXT block of `/review-pr`, per-candidate block in `pr-blame-walk`, ad-hoc *"summarise this PR for me"*).
- **Via slash command** — `/pr-synthesis <N|owner/repo#N|url>` fetches the PR, linked issue, and diff for you and prints the synthesis verbatim.

## Inputs

- `<pr_text>` — PR title and body. If there is no PR yet (e.g. local-diff mode), substitute the branch name plus the relevant commit messages.
- `<linked_issue>` — title and body of any issue referenced via `Fixes #N` / `Closes #N` / `Resolves #N`. May be empty.
- `<diff_summary>` — file scope plus a codemap or per-file note of the actual change. The ground truth the synthesis must stay anchored to.

## Rules

1. **Synthesize, don't copy-paste.** If `<pr_text>` is five words, say so plainly: *"description is minimal; intent inferred from diff"*. Don't pad to look thorough.
2. **Watch for description-vs-diff drift.** `<pr_text>` must describe `<diff_summary>`'s *current* state, not the author's iteration history. If a claim is no longer true of the diff, note it in the synthesis as *"description claims X; diff shows Y"*. Don't raise an `Action:` finding here — this skill reports facts; the consumer (e.g. `/review-pr`) decides whether to escalate. Do **not** flag the absence of a changelog of removed/superseded behaviour either — that belongs in commit history, not the description.
3. **No vague verbs.** *"This PR updates something"* is a failure. Name the component, the change, and the mechanism.

## Shape

- **Paragraph 1** — *what* changed. Component + concrete change, drawn from `<diff_summary>`.
- **Paragraph 2** — *why*. Drawn from `<pr_text>` and `<linked_issue>`. If both are thin, say so.
- **Paragraph 3** (only if warranted) — *how*. The approach, not a line-by-line walkthrough.

## Example

Inputs (real PR — cowprotocol/services#4371):

- `<pr_text>` — *"Enforce EIP-7825 per-tx gas cap on settlement"*; body explains the Fusaka mempool cap and references the existing quote-side enforcement (#4261).
- `<linked_issue>` — #4368, *"Driver doesn't enforce the EIP-7825 per-tx gas cap"* (labels: bug, good first issue).
- `<diff_summary>` — +58 −1 in `settlement.rs`; new const `EIP_7825_TX_GAS_CAP`, `Gas::new` capped via `min(half_block, EIP_7825)`, three tests.

Output:

> Fusaka introduced EIP-7825, capping any single tx at 2^24 − 1 gas. The driver's `Gas::new` was applying only the older `block_limit/2` heuristic; a solution between the EIP-7825 cap and that heuristic could pass validation in `/solve` and never settle. Issue #4368 flagged this as theoretical (no production hits); the fix ports the same idea PR #4261 applied to the quote path.
>
> Mechanically: `max_gas = min(half_block, EIP_7825)`, preserving inclusion-economics on chains with low block gas limits via the `min`.

## When to skip

- Trivial changes (docs typo, single-line dep bump, lockfile-only). One sentence is enough; don't force the three-paragraph shape.
- `<pr_text>` and `<linked_issue>` are both empty *and* `<diff_summary>` already speaks for itself (e.g. a single-file rename). One sentence noting that the diff is self-explanatory is the correct output.
