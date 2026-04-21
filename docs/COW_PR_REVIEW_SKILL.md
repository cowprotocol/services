# CoW Services PR Review Skill

This document instructs Claude how to produce a local PR review report. It is invoked by `.claude/commands/review-pr.md` after the prologue (arg parsing, prereq check, clean-tree check, main update, PR checkout).

At this point you have:

- A parsed `<PR_NUMBER>`, `<owner>`, `<repo>`.
- A `mode` flag: `full checkout` (default) or `degraded static-diff` (fork fallback).
- A `loaded_context` list of installed prereq skills.
- A `prior_branch` variable holding the branch name to return to.

## Core Principles (read before executing)

- **Signal over noise.** Report genuine concerns only. LGTM is a perfectly valid verdict and is the correct one whenever the PR is clean.
- **Never post to GitHub.** Output is strictly for the user's terminal. No `gh pr review`, no `gh pr comment`, no `gh pr close`. The user decides what to say on GitHub.
- **Explain, don't just flag.** Each finding must give the reviewer enough context to understand *and defend* the point — not just forward AI-generated text.
- **Actionable framing.** Every finding ends with either a concrete `Action:` or a specific `Question:`. Never both.

## Execution Flow

1. Fetch PR metadata and linked issue(s) — [§2. Metadata Fetch](#2-metadata-fetch)
2. Classify diff paths and load sibling context docs — [§3. Classification](#3-classification)
3. Synthesize the context block — [§4. Context Synthesis](#4-context-synthesis)
4. Produce findings by severity — [§5. Review and Severity](#5-review-and-severity)
5. Print the structured report — [§6. Report Template](#6-report-template)
6. Offer verification (background) — [§7. Verification Offer](#7-verification-offer)
7. Print cleanup hint — [§8. Cleanup](#8-cleanup)

Error behavior is consolidated in [§9. Error Playbook](#9-error-playbook).

---

## 2. Metadata Fetch

Run these commands in **parallel** (single message, multiple Bash tool calls):

```bash
# Full metadata
gh pr view <PR_NUMBER> -R <owner>/<repo> --json title,body,author,labels,files,baseRefName,headRefName,additions,deletions,commits,reviewDecision,isDraft,state

# Full diff
gh pr diff <PR_NUMBER> -R <owner>/<repo>
```

In **degraded static-diff mode** (fork fallback), replace `gh pr diff` with:
```bash
gh pr diff <PR_NUMBER> --patch -R <owner>/<repo>
```

### Linked issues

Parse the PR body for `Fixes #<N>`, `Closes #<N>`, `Resolves #<N>` (case-insensitive). For each match, fetch in parallel with the above:
```bash
gh issue view <N> -R <owner>/<repo> --json title,body,labels,state
```

If no linked issue is referenced, proceed without one. Do not manufacture an issue link.

### State handling

- `state == "CLOSED"` or `state == "MERGED"` → proceed, but prepend to the report:
  > ⚠ This PR is {closed,merged}; review is informational.
- `isDraft == true` → proceed, but prepend:
  > ⚠ This PR is a draft; author may still be iterating.

---

## 3. Classification

Walk the file list from `gh pr view --json files`. For each changed file, evaluate these rules and accumulate a `context_docs` list:

| Rule | Match | Load |
|---|---|---|
| R1 — Alloy usage (path) | Any change under `crates/ethrpc/`, `crates/chain/`, or `crates/contracts/` | `docs/review-context/alloy-rs.md` |
| R2 — Alloy usage (import) | Any `.rs` file whose diff hunks add `use alloy::*;`, `use alloy_*;`, or `alloy::` qualified paths | `docs/review-context/alloy-rs.md` |
| R3 — DB migrations | Any file under `database/sql/**` | `docs/review-context/database-migrations.md` |
| R4 — OpenAPI | Any file matching `**/openapi.yml` | `docs/review-context/openapi.md` |

R1 and R2 are OR'd — load `alloy-rs.md` **once** if either matches.

### Loading

Read each matched sibling doc via the `Read` tool. Record the loaded list — it will appear in the report header's `Loaded context:` line.

If no rules match, `context_docs` is empty and the review proceeds with only the always-loaded `CLAUDE.md` + `actionbook/rust-skills`.

### Future siblings

This list will grow. When adding a new sibling doc (e.g. `solver-engine.md`, `autopilot.md`), add its trigger row to the table above. Keep the filters tight — a sibling should load only when its content is actually relevant.

---

## 4. Context Synthesis

Produce a 1-3 paragraph block combining:

- PR title.
- PR description (Description / Changes / How to test sections from the template, if filled).
- Linked issue title + description, if any.
- File scope — breadth of the diff (single crate, cross-crate, docs-only, DB, API spec).
- Rough intent — new feature, bugfix, refactor, dep bump, docs, test-only.

### Rules

1. **Synthesize, do not copy-paste.** If the description is five words, say so: *"PR description is minimal — intent inferred from diff"*. Don't pretend.
2. **Flag description-vs-diff mismatches as findings.** If the description says "docs-only" but `.rs` files are touched, open a **High**-severity finding titled `PR description contradicts diff scope` immediately. This is the one case where a finding precedes the normal review loop.
3. **Linked-issue context goes into synthesis, not as a separate block.** Summarise motivation from the issue alongside the PR's own description.
4. **No vague verbs.** *"This PR updates something"* is a failure. Name the component, the change, and the mechanism.

### Shape

- **Paragraph 1** — *What* the PR changes (mechanics, file scope).
- **Paragraph 2** — *Why* the change exists (from description + linked issue).
- **Paragraph 3** (only if warranted) — *How* it's implemented (the approach, not a line-by-line walkthrough).

---

## 5. Review and Severity

Read the diff. For non-trivial hunks, read the full changed file(s) for surrounding context. Apply, in order:

1. Generic Rust review from `actionbook/rust-skills`.
2. CoW services conventions from `CLAUDE.md`.
3. Conditionally loaded sibling docs from [§3](#3-classification).
4. Soft QM skill (`ra-qm-team`), if in `loaded_context`.

### Severity Rubric

| Severity | Meaning | Example |
|---|---|---|
| **High** | Merging as-is is a real risk: correctness bug, data loss, security issue, incompatible DB migration, auction/settlement invariant broken, likely panic, unsound `unsafe`. | `.unwrap()` on a solver response path; SQL migration that rewrites a multi-million-row table without `CONCURRENTLY`. |
| **Medium** | Worth fixing before merge — won't break prod but will cost later. Missing error context, test gap on a new invariant, public API ergonomics, missing doc-comment on a cross-crate `pub`, unhandled edge case. | New `pub fn` in `shared` with no doc-comment; `?` swallowing an error without context. |
| **Small / QoL** | Would genuinely improve the code. **Not a nit.** | `Vec` could be `impl Iterator` in a hot path; duplicated 3-line block could be a helper. |

### Anti-nit Rule (mandatory)

- If the only reason to change it is personal taste or stylistic preference, **do not report it**.
- Formatting findings belong to `rustfmt` / CI, not to this skill. Never surface a finding whose fix is "run `cargo +nightly fmt`".
- Clippy lints are a CI concern by default. **Exception:** a clippy warning inside the new code may be reported as **Small** if and only if it improves correctness or clarity — never for style.
- If you're uncertain whether something is a nit, omit it. LGTM when clean.
- **Don't inflate severity.** The severity of a finding is what a senior reviewer would actually call it in GitHub, not what sounds safer. Most substantive comments are just unmarked; only the few purely cosmetic ones carry a `nit:` prefix in practice. Don't tier-down everything to "Small" to avoid confrontation — that dilutes the signal. Either it's a Medium worth discussing or it's omitted.

### Reviewer Discipline — Heuristics Beyond "Is This Bug"

A senior reviewer catches things that aren't bugs. They shape the code for future readers and future maintainers. These are the patterns a good CoW review surfaces:

1. **Motivation before mechanism.** Before reviewing *how* the code works, verify *why* it exists. If the PR description doesn't justify the change, or justifies it with an assumption ("EIP-4626 tokens will have coverage problems"), ask the author to confirm the assumption — as a `Question:`, not a demand. You can't judge tradeoffs against a motivation you don't understand.

2. **Root cause over workaround.** If a new file contains logic that feels like it exists to cancel out a wart somewhere else ("insert WETH into non_vault_tokens", "treat this special case differently"), investigate the wart. Fixing the root cause upstream is usually cleaner than adding a compensating wart downstream. Ask the author: *"this logic looks like it's working around X — could X be fixed instead?"*

3. **Suggest simpler primitives; listen to pushback.** If code uses `join!` where `try_join!` would work, ask. If it uses `Mutex<HashSet<_>>` where `DashSet` would work, ask. The author often has a reason (as in the eip4626.rs case where `join!` is deliberate because the branch logic depends on both results), and the back-and-forth resolves the question. Framing: *"Can this be `<simpler alternative>`?"*, not *"Use `<alternative>`"*.

4. **Return-type consistency over side-effects at distance.** A function returning `Result<Option<(T, U)>, Error>` explicitly encodes its three states (value / no-value / error). A function that writes to a cache two layers down the call stack as a side effect hides the same information. Prefer explicit return types. The cost of a verbose type signature is always smaller than the cost of a reviewer (or future you) missing a hidden mutation.

5. **Top-to-bottom readability.** Within a file, order functions in the direction callers-before-callees or high-level-before-low-level (pick one and keep it consistent). When reviewing, if you find yourself scrolling back and forth between `fn do_thing` and `fn helper_for_thing`, flag the ordering — *not as a nit, as a Medium if it's non-trivial navigation*.

6. **Stale comments.** When behavior changes, comments describing that behavior often rot. Read the comments *against* the code around them and flag any drift. A comment saying "zero timeout signals best-effort" attached to code that no longer has a zero-timeout path is a Medium finding — future readers will believe it.

7. **Description-vs-code mismatches.** If the PR body describes `Mutex<HashSet>` and the code has `DashSet`, the description is stale. Flag it — not because it changes the code, but because whoever reads the PR as history will be confused. Small finding, usually.

8. **Bundled orthogonal changes.** When a PR contains a change that isn't clearly required by the main feature, ask: does this belong in its own PR? Sometimes the answer is "it's required, here's why" (fine — ask for the reason to be in the commit message or a code comment). Sometimes the answer is "you're right, let me split it" (better git history).

9. **Error taxonomies in web3 code.** Blockchain RPC errors have multiple categories: contract reverts, RPC transport failures, provider-side rate-limiting, timeout errors, decoding failures. Each should lead to *different* handling. If new code treats `Err(_)` as a single bucket when the branches should differ (e.g. cache a non-vault verdict on contract revert, but *not* on a network error), this is a correctness issue — Medium or High depending on whether the wrong classification can poison downstream behavior.

10. **Generated-code and lockfile diffs.** Never include generated-bindings or `Cargo.lock` findings. CI's type-check validates the generated code; if CI is green, there's nothing a reviewer will see that the compiler won't. Filter these at the entry-point (see `.claude/commands/review-pr.md` §6) and report only on human-written changes.

### Using GitHub "Suggested change" Blocks in the Output

When a finding has a clear mechanical fix (e.g. remove a `.clone()`, swap a type), phrase the `Action:` line so the user can paste it straight into a GitHub *Suggested change* block. Example:

```
Action: Replace `self.provider.clone()` with `&self.provider` — the alloy
        Instance constructors take `IntoProvider` which is implemented for
        both owned and borrowed providers. (GitHub suggested-change:
        `let vault = IERC4626::new(token, &self.provider);`)
```

The bracketed suggestion is copy-pasteable into GitHub's suggestion feature, which makes the author's acceptance a single click.

### Per-finding Shape

Every finding contains exactly these four parts:

1. **Title** — short noun phrase (≤ 8 words).
2. **Location** — `path/to/file.ext:line` (or `path/to/file.ext:start-end` for a range).
3. **Explanation** — enough that the reviewer understands *and can defend* the point without re-reading the diff. Must include:
   - **The mechanism** (what's wrong).
   - **The impact** (why it matters for CoW services specifically — auction, settlement, solver competition, DB migration, etc.).
   - **Repo-specific context** where it helps (e.g. "this path runs per-auction, ~12s cadence, so an extra RPC is costly").
4. **Action OR Question** — exactly one, never both:
   - `Action: <concrete task the author should do>`
   - `Question: <specific clarification needed before the reviewer can decide>`

---

## 6. Report Template

Print the report in this exact shape. Omit sections that don't apply (e.g. no `VERIFICATION` block if the user declined).

```
═══════════════════════════════════════════════════════════
PR #<N> — <title>
═══════════════════════════════════════════════════════════
Author:       @<author>
Scope:        +<additions> −<deletions> across <N> files
Labels:       <labels, comma-separated; or "—">
Base/Head:    <baseRef> ← <headRef>
Linked issue: #<N> — <issue title>            (omit line if none)
Loaded context: <comma-separated loaded_context list>
Mode:         full checkout   |   degraded static-diff

───────────────────────────────────────────────────────────
CONTEXT
───────────────────────────────────────────────────────────
<synthesis from §4>

───────────────────────────────────────────────────────────
VERDICT:  <LGTM | Changes requested | Needs clarification>
───────────────────────────────────────────────────────────

FINDINGS
  [High]    <count>
  [Medium]  <count>
  [Small]   <count>    (QoL-only; true nits omitted)

───── High ─────────────────────────────────────────────────
1. <title>
   Location:  <path>:<line>

   <explanation>

   Action:    <task>      OR
   Question:  <question>

───── Medium ───────────────────────────────────────────────
<same shape>

───── Small / QoL ─────────────────────────────────────────
<same shape>

───────────────────────────────────────────────────────────
VERIFICATION   (only if user opted in)
───────────────────────────────────────────────────────────
cargo check            <status>
cargo clippy           <status>
cargo +nightly fmt     <status>

───────────────────────────────────────────────────────────
NEXT STEPS
───────────────────────────────────────────────────────────
Currently on:  <current branch>
Return with:   git switch <prior_branch>
```

### LGTM short form

When there are **zero** findings (High, Medium, and Small all zero), collapse everything between `VERDICT:` and `NEXT STEPS` to a single line:

```
VERDICT:  LGTM — no blocking or notable issues.
```

The header, CONTEXT, and NEXT STEPS sections still print.

### Verdict selection

- **LGTM** — no High or Medium findings; zero or a handful of Small findings that the reviewer may still choose to post.
- **Changes requested** — any High findings, or Medium findings that block safety/correctness.
- **Needs clarification** — no High findings, but one or more Medium/Small findings use the `Question:` form because the reviewer can't decide without author input.

---

## 7. Verification Offer

After printing the report, ask the user inline:

> "Run local verification in the background? Reply with one of:
> `check` — `cargo check --locked --workspace --all-features --all-targets`
> `clippy` — `cargo clippy --locked --workspace --all-features --all-targets -- -D warnings`
> `fmt` — `cargo +nightly fmt --all -- --check`
> `all` — run all three
> `skip` — don't run anything"

On user reply:

- **`skip`** → omit the VERIFICATION block entirely. Proceed to [§8](#8-cleanup).
- **`check` / `clippy` / `fmt` / `all`** → dispatch each selected command as a **background** Bash invocation (`run_in_background: true`). Do not sleep, do not poll — the runtime sends a completion notification. On completion, call `BashOutput` to retrieve the result and append it to the VERIFICATION block:
  ```
  cargo check            ✅  clean
  cargo clippy           ⚠   2 warnings (details below)
  cargo +nightly fmt     ✅  clean
  ```

### When verification output produces findings

- If `cargo check` or `cargo clippy` surfaces errors/warnings **inside files changed by this PR**, add them to the Findings list:
  - Compile errors → **High** (the PR doesn't build).
  - Clippy warnings that flag a correctness issue → **Medium**.
  - Clippy warnings that flag clarity (e.g. `needless_return` inside new code) → **Small** — but only when they pass the [§5](#5-review-and-severity) Anti-nit rule.
- Do **not** surface warnings from files the PR did not modify.
- `cargo +nightly fmt -- --check` failures are **not** findings. The Anti-nit rule forbids surfacing format findings. The VERIFICATION block reports the status; that's where it ends.

### Never run tests by default

The menu deliberately omits `cargo nextest run`. Services' test suite is large and takes minutes — gating every review on a full test run is too expensive. If the reviewer wants tests, they run them outside the skill.

---

## 8. Cleanup

The NEXT STEPS footer (already part of the report template in [§6](#6-report-template)) names the current branch and the exact command to return to the prior branch:

```
Currently on:  <current branch>
Return with:   git switch <prior_branch>
```

**Do not run `git switch` yourself.** The user may want to stay on the PR branch to continue investigating, run their own tests, or browse related code. The command is a hint, not an action.

---

## 9. Error Playbook

| Condition | Behavior |
|---|---|
| `gh` not installed | Print `Install gh: https://cli.github.com/`, abort. |
| `gh` not authenticated | Print `gh auth status` output + `Run: gh auth login`, abort. |
| PR number not parseable | Print usage string (see `.claude/commands/review-pr.md` step 1), abort. |
| PR doesn't exist / wrong repo | Surface `gh` error verbatim, abort. |
| PR is closed or merged | Prepend state warning to the report ([§2](#2-metadata-fetch)), proceed. |
| PR is a draft | Prepend draft warning to the report ([§2](#2-metadata-fetch)), proceed. |
| Working tree dirty | Print `git status --porcelain`, instruct `git stash` or commit, abort — **no auto-stash**. |
| `git pull --rebase origin main` conflict | Abort, print conflicted files, instruct manual resolution. |
| `gh pr checkout` fails (fork permission) | Degrade to static-diff mode, flag in report header `Mode:` field. |
| Hard prereq skill missing | Print install command, abort. Handled in the entry-point prologue, not here. |
| Soft prereq skill missing | Print install-command banner, continue. Handled in the entry-point prologue, not here. |
| Verification offer declined (`skip`) | Omit VERIFICATION block. |
| Verification command fails or warns | Surface output in VERIFICATION block; add findings only for issues inside changed files. |

**Rule of thumb:** never silently degrade behavior without the `Mode:` header reflecting it. If something went sideways, the reviewer should know from the report itself.

---

## Maintenance Notes

- **When you find yourself adding a project-specific heuristic more than twice**, move it into a sibling context doc under `docs/review-context/` and add a trigger rule to [§3](#3-classification).
- **When you find the skill making a false-positive finding**, add a counter-example to the Anti-nit rule section in [§5](#5-review-and-severity). The rubric calibrates over time.
- **When CoW introduces a new subsystem with its own review considerations** (e.g. a new crate, new auction policy, new settlement path), create a sibling doc for it and wire in a trigger.

This skill is expected to grow — both `docs/review-context/*.md` and this reference's rubric will accumulate CoW-specific knowledge over time.
