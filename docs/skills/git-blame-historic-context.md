# Skill — `git blame` for historic context

Use before flagging code that looks unusual, redundant, accidental, or "easy to clean up". Often that code looks weird because it had to.

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
