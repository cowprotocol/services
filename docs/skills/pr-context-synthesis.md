# Skill — PR context synthesis

Use to produce a tight 1–3 paragraph *what / why / how* block for a single PR (or PR-shaped change). Consumed by the PR review report's CONTEXT section and by [`pr-blame-walk`](pr-blame-walk.md), which calls this skill once per candidate PR before scoring.

## Inputs

- `<pr_text>` — PR title and body. If there is no PR yet (e.g. local-diff mode), substitute the branch name plus the relevant commit messages.
- `<linked_issue>` — title and body of any issue referenced via `Fixes #N` / `Closes #N` / `Resolves #N`. May be empty.
- `<diff_summary>` — file scope plus a codemap or per-file note of the actual change. The ground truth the synthesis must stay anchored to.

## Rules

1. **Synthesize, don't copy-paste.** If `<pr_text>` is five words, say so plainly: *"description is minimal; intent inferred from diff"*. Don't pad to look thorough.
2. **Watch for description-vs-diff drift.** `<pr_text>` must describe `<diff_summary>`'s *current* state, not the author's iteration history. If a claim is no longer true of the diff, raise a finding with `Action: update the PR description to match the current diff`. Do **not** flag the absence of a changelog of removed/superseded behaviour — that belongs in commit history, not the description.
3. **No vague verbs.** *"This PR updates something"* is a failure. Name the component, the change, and the mechanism.

## Shape

- **Paragraph 1** — *what* changed. Component + concrete change, drawn from `<diff_summary>`.
- **Paragraph 2** — *why*. Drawn from `<pr_text>` and `<linked_issue>`. If both are thin, say so.
- **Paragraph 3** (only if warranted) — *how*. The approach, not a line-by-line walkthrough.

## When to skip

- Trivial changes (docs typo, single-line dep bump, lockfile-only). One sentence is enough; don't force the three-paragraph shape.
- `<pr_text>` and `<linked_issue>` are both empty *and* `<diff_summary>` already speaks for itself (e.g. a single-file rename). One sentence noting that the diff is self-explanatory is the correct output.
