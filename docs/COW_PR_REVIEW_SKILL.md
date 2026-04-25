# CoW Services PR Review Skill

This document instructs Claude how to produce a PR review for cowprotocol/services. It is invoked by `.claude/commands/review-pr.md` (locally) or by `.github/workflows/claude-code-review.yml` (in CI). One skill, three operating modes.

At the point this document is read, the entry-point has already determined:

- **`mode`** — one of:
  - `diff` — local, no PR yet. Source is `git diff $(git merge-base HEAD main)..HEAD` (the whole feature-branch worth of work). No PR metadata, no `gh` calls. Output to terminal.
  - `pr-local` — local, PR exists. Source is `gh pr diff <N>` plus PR metadata. Output to terminal.
  - `pr-ci` — running inside `.github/workflows/claude-code-review.yml`. Source is `gh pr diff <N>` plus PR metadata. Output is a single review comment posted to the PR.
- **`<PR_NUMBER>`, `<owner>`, `<repo>`** (only in `pr-local` / `pr-ci`).
- **`prior_branch`** (only in `pr-local` — the branch to print at the end).

The mode shapes which steps run and how the report is delivered, but the *content* of the review is the same in all three.

---

## Core Principles (read before executing)

- **Signal over noise.** Report genuine concerns only. LGTM is a perfectly valid verdict and is the correct one whenever the PR is clean. The goal is not to maximise finding count — it is to be worth a senior reviewer's attention.
- **Local modes never post to GitHub.** In `diff` and `pr-local`, output is strictly terminal. No `gh pr review`, no `gh pr comment`. The user posts whatever they choose. In `pr-ci`, post exactly one consolidated review comment — no per-line spam.
- **Code is the primary source of truth.** `CLAUDE.md`, design docs in `docs/`, and this skill's own sibling docs can go stale. When a finding turns on *"X is called from Y"* or *"this field is read by Z"*, verify with `git grep` / `rg` / LSP — not by citing a doc.
- **Inverted: this PR can make existing docs / comments / its own description stale.** If a code change makes a comment, a `docs/` page, or the PR's own description no longer match the diff's current state, that is itself a finding (`Action:` → update X).
- **`git blame` before flagging code that looks unusual.** Often code looks weird because it had to. Before suggesting a "cleanup", blame the affected lines, read the originating commit message and (if any) linked PR. A comment that says *"this looks accidental, did you mean X?"* without that step risks asking the author to undo a hard-won fix.
- **Explain, don't just flag.** Each finding must give the reviewer enough context to understand *and defend* the point — not just forward AI-generated text.
- **One framing per finding:** end with either `Action:` (concrete task) or `Question:` (clarification needed). Never both.
- **Token discipline.** Don't read whole files when grep or LSP suffices. Build a codemap before reading file bodies.

---

## Universal Guardrails

Apply these as the default lens for every change. Pull in CoW-specific siblings ([§3](#3-conditional-context)) only when the diff warrants them.

1. **Keep the public API surface minimal.** A new `pub fn`, `pub struct`, or `pub mod` that isn't required by an external caller is a Medium finding asking why it isn't `pub(crate)` / `pub(super)` / private. Smaller surface = fewer downstream breakages = freer refactor for the next person.
2. **Avoid rightward drift.** Code that's deeply nested (4+ levels, especially `match` inside `if let` inside `for` inside `async`) is hard to read and usually hides a missing extraction. Suggest an early-return, a helper, or `let-else`.
3. **One responsibility per component.** A function, struct, or module that does two unrelated things (validates *and* persists; parses *and* renders) is harder to test and to reuse. Flag with a `Question:` if you're not sure the split is artificial.
4. **Split big files.** A new file pushing past ~500 lines, or a touched file growing past ~1000 lines, is worth flagging. Suggest a sensible split (often by responsibility from #3).
5. **Avoid argument bloat.** A function taking 6+ positional arguments is a code smell — usually missing a config struct, a builder, or a method on a context object. Especially flag if the arguments are mostly being threaded through unchanged.
6. **Errors carry context.** `?` propagating a low-level error to a high-level boundary without enrichment loses the *what was the caller trying to do* information. `anyhow!("{err}")` flattens cause chains. Both are findings; severity depends on the path.

---

## Execution Flow

Steps run in this order. `diff` mode skips PR-metadata steps; `pr-ci` swaps the report sink at the end.

1. Fetch PR metadata and linked issue(s) — [§2](#2-metadata-fetch). *(`pr-local` / `pr-ci` only.)*
2. Classify diff paths and load conditional context — [§3](#3-conditional-context).
3. Build a targeted codemap — [§4](#4-codemap-phase).
4. Synthesize the context block — [§5](#5-context-synthesis).
5. Review and produce findings — [§6](#6-review-and-severity).
6. Emit the report — [§7](#7-report-templates). Sink depends on mode.
7. Offer verification (background) — [§8](#8-verification-offer). *(Local modes only.)*
8. Print cleanup hint — [§9](#9-cleanup). *(`pr-local` only.)*

Error behaviour is consolidated in [§10](#10-error-playbook).

---

## 2. Metadata Fetch

*(Skip in `diff` mode — there is no PR yet.)*

Run in **parallel** (single message, multiple Bash tool calls):

```bash
gh pr view <PR_NUMBER> -R <owner>/<repo> \
  --json title,body,author,labels,files,baseRefName,headRefName,additions,deletions,commits,reviewDecision,isDraft,state

gh pr diff <PR_NUMBER> -R <owner>/<repo>
```

In **degraded static-diff mode** (fork without checkout permission, only relevant in `pr-local`), replace `gh pr diff` with:

```bash
gh pr diff <PR_NUMBER> --patch -R <owner>/<repo>
```

### Linked issues

Parse the PR body for `Fixes #<N>`, `Closes #<N>`, `Resolves #<N>` (case-insensitive). Fetch each in parallel with the above:

```bash
gh issue view <N> -R <owner>/<repo> --json title,body,labels,state
```

If no linked issue is referenced, proceed without one. Do not manufacture one.

### State handling

- `state == "CLOSED"` or `"MERGED"` → proceed; prepend a one-line warning to the report.
- `isDraft == true` → proceed; prepend `Draft — author may still be iterating.`

---

## 3. Conditional Context

For each changed file, evaluate this table and accumulate a `context_docs` list. Read each matched doc once.

| Match | Load |
|---|---|
| Any file under `database/sql/**` | `docs/review-context/database-migrations.md` |

Add a new sibling only when:

- A real review surfaced a CoW-specific concern the AI consistently missed, **and**
- That concern can't reasonably be inferred from the [Universal Guardrails](#universal-guardrails) plus general Rust judgment, **and**
- It can be expressed as a tight checklist (≤30 lines), not a sprawling rulebook.

### One inline guardrail worth keeping

When a PR touches **any `openapi.yml`**: scan for breaking changes (removed/renamed/typed-changed fields, new required request fields, narrowed enums, changed auth or HTTP method). If any are present, ask whether the goal could be achieved non-breakingly (additive field, new optional, deprecation window) and whether the affected consumer teams (Frontend, SAFE, etc.) have been notified. Severity: High when undisclosed in the PR description; Medium otherwise.

---

## 4. Codemap Phase

**Purpose:** before reading file bodies, map the symbols the diff touches, their callers, and their call sites. A codemap turns a 1000-line diff into a ~20-line mental model and catches findings that only become visible at the *shape* level (caller-count inconsistencies, dead abstractions, leaky public APIs).

### What to map

For each non-trivial symbol the diff adds, modifies, or deletes:

1. **New public types / traits / functions** — fields, methods, signatures.
2. **Modified function signatures** — caller count, were all sites updated?
3. **New trait impls** — which types implement the trait? Is the trait used outside this PR?
4. **Error-type changes** — where do callers match on this error?

### Tools (cheapest viable option first)

| Tool | Status | When to use |
|---|---|---|
| `Grep` / `rg` with `-n` on a symbol name | Always available | Caller counts and basic location lookups. Example: `rg 'OrderValidator::new\b' crates/`. |
| `gh api` / `git blame` | Always available | Historic context on suspicious-looking lines. |
| `mcp__plugin_serena_serena__find_symbol` / `find_referencing_symbols` / `get_symbols_overview` | Optional (LSP-backed; available when Serena MCP is configured) | Precise location + kind + signature without reading the full file. |
| Skills from `actionbook/rust-skills` (`rust-call-graph`, `rust-symbol-analyzer`, `rust-trait-explorer`, `rust-code-navigator`) | Optional (installed via `npx skills add actionbook/rust-skills`) | Richer cross-crate analysis. Not present in CI by default. |
| `Read` of a full file | Last resort | Only when the diff hunks plus the cheaper tools don't pin down what you need. |

**Fallback rule:** in CI (or any environment where the optional tools aren't installed), every codemap step still works with `rg` and `git`. The optional tools are accelerators, not requirements.

### What the codemap produces

A short block in the report that looks like:

```
Codemap
───────────────────────────────────────────────────────────
New symbols (crate::module):
  <Name>  (kind, key fact)
  ...

Callers of <Name>: <count> sites (<real> real, <test> test). All updated ✓
```

This is the raw material §5 (synthesis) and §6 (findings) work from.

### When to skip

Trivial PRs (docs-only, single-line bump, pure test addition) — skip. Pure refactors with no added public API — skim. Everything else — do it.

---

## 5. Context Synthesis

Produce 1–3 paragraphs combining the PR title, description, linked issue(s), file scope, and intent (feature, bugfix, refactor, dep bump, docs, test).

### Rules

1. **Synthesize, don't copy-paste.** If the description is five words, say so plainly: *"description is minimal; intent inferred from diff"*.
2. **Watch for description-vs-diff drift.** A PR description must describe the diff's *current* state, not the author's iteration history. If a claim in the description is no longer true of the diff, raise a finding with `Action: update the PR description to match the current diff`. Do not flag the absence of a changelog of removed/superseded behaviour — that belongs in commit history, not the description.
3. **No vague verbs.** *"This PR updates something"* is a failure. Name the component, the change, and the mechanism.

### Shape

- **Paragraph 1** — *what* changed.
- **Paragraph 2** — *why* (description + linked issue).
- **Paragraph 3** (if warranted) — *how* (the approach, not a line-by-line walkthrough).

---

## 6. Review and Severity

Apply, in order:

1. The [Universal Guardrails](#universal-guardrails).
2. The conditional context from [§3](#3-conditional-context), if any was loaded.
3. CoW-services conventions from `CLAUDE.md`.
4. Optional skills from [§4 → tools table](#tools-cheapest-viable-option-first), activated by what the diff actually contains. If installed, invoke `m06-error-handling` for `Result`/`Option`/`?` changes, `m07-concurrency` for `tokio::*` / async / locking, `m04-zero-cost` for new generics or trait objects, `m15-anti-pattern` for general sanity, `unsafe-checker` for any `unsafe` (mandatory High). If they aren't installed, reason from general Rust knowledge plus the [Universal Guardrails](#universal-guardrails).

### Use `git blame` for historic context

Before flagging code that looks unusual, redundant, or "easy to clean up":

```bash
git blame -L <start>,<end> -- <file>            # who/what/when
git log --format='%H %s' -n 1 <commit>          # commit message
gh pr view <#> -R cowprotocol/services          # if commit message links a PR
```

If the originating commit message or PR explains *why* the code is shaped that way, factor that into your finding. A "this looks accidental" comment is much weaker when blame shows a deliberate fix from six months ago. Mention what blame revealed in the finding's Explanation so the reviewer can defend the point.

### Severity Rubric

| Severity | Meaning |
|---|---|
| **High** | Merging as-is is a real risk: correctness bug, data loss, security issue, incompatible DB migration, auction/settlement invariant broken, likely panic, unsound `unsafe`. |
| **Medium** | Worth fixing before merge — won't break prod but will cost later. Missing error context, public-API ergonomics, unhandled edge case, n-1 rollout incompatibility, undisclosed breaking API change. |
| **Small / QoL** | Would genuinely improve the code. **Not a nit.** |

### Anti-nit Rule (mandatory)

- If the only reason to change it is taste or stylistic preference, **do not report it**.
- Formatting belongs to `rustfmt` / CI. Never raise a finding whose fix is "run `cargo +nightly fmt`".
- Clippy lints are a CI concern by default. **Exception:** a clippy warning inside the new code may be reported as **Small** if and only if it improves correctness or clarity — never style.
- If you're uncertain whether something is a nit, omit it. LGTM is the right verdict when the PR is clean.
- **Don't inflate severity** to look thorough. Each finding's severity is what a senior reviewer would actually call it on GitHub. Either it's worth discussing or it's omitted.

### Per-finding shape

1. **Title** — short noun phrase (≤ 8 words).
2. **Location** — `path/to/file.ext:line` or `path:start-end`.
3. **Explanation** — mechanism, impact, and (if relevant) what `git blame` / the codemap revealed.
4. **`Action:` OR `Question:`** — exactly one.

When a finding has a clear mechanical fix, phrase the `Action:` so the reviewer can paste it as a GitHub *Suggested change* block.

---

## 7. Report Templates

### Terminal form (`diff` and `pr-local`)

```
═══════════════════════════════════════════════════════════
PR #<N> — <title>                       (or "Diff review — <branch>" in diff mode)
═══════════════════════════════════════════════════════════
Author:       @<author>                 (omit in diff mode)
Scope:        +<add> −<del> across <N> files
              (~X LOC human-written; rest generated/lockfile, filtered)
Labels:       <labels or "—">           (omit in diff mode)
Base/Head:    <baseRef> ← <headRef>     (or "main ← <branch>" in diff mode)
Linked issue: #<N> — <title>            (omit if none)
Mode:         diff | pr-local | pr-ci   ;  full checkout | degraded static-diff

Codemap
───────────────────────────────────────────────────────────
<from §4 — omit if skipped>

───────────────────────────────────────────────────────────
CONTEXT
───────────────────────────────────────────────────────────
<synthesis from §5>

───────────────────────────────────────────────────────────
VERDICT:  <LGTM | Changes requested | Needs clarification>
───────────────────────────────────────────────────────────

FINDINGS
  [High]    <count>
  [Medium]  <count>
  [Small]   <count>    (QoL-only; nits omitted)

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
NEXT STEPS                              (pr-local only)
───────────────────────────────────────────────────────────
Currently on:  <current branch>
Return with:   git switch <prior_branch>
```

#### LGTM short form

When there are zero findings at all severities, collapse everything between `VERDICT:` and `NEXT STEPS` to a single line:

```
VERDICT:  LGTM — no blocking or notable issues.
```

Header, CONTEXT, and (in `pr-local`) NEXT STEPS still print.

### Comment form (`pr-ci`)

In CI, post **one** review comment via the action. Use the same body shape as the terminal form, minus:

- The NEXT STEPS section (no local branch state to print).
- The VERIFICATION block (CI runs its own checks separately).
- ANSI box-drawing characters (Markdown headings instead).

GitHub-render the body using `##`/`###` headings and fenced code blocks. Keep findings collapsible with `<details>` if there are more than ~5.

### Verdict selection

- **LGTM** — no High or Medium findings.
- **Changes requested** — any High, or Medium that blocks safety/correctness.
- **Needs clarification** — no High, but one or more findings use the `Question:` form.

---

## 8. Verification Offer

*(Local modes only. Skip in `pr-ci` — CI runs its own check/clippy/fmt jobs in parallel.)*

After printing the report, ask the user:

> "Run local verification in the background? Reply with one of:
> `check` — `cargo check --locked --workspace --all-features --all-targets`
> `clippy` — `cargo clippy --locked --workspace --all-features --all-targets -- -D warnings`
> `fmt` — `cargo +nightly fmt --all -- --check`
> `all` — run all three
> `skip` — don't run anything"

Dispatch each selected command as a **background** Bash invocation. On completion, append the result to a VERIFICATION block.

If `cargo check` or `cargo clippy` surfaces issues **inside files this PR modified**, fold them into Findings (compile errors → High, correctness clippy → Medium, clarity clippy → Small subject to the Anti-nit rule). Do not surface warnings from files the PR didn't touch. `fmt --check` failures are a status, never a finding.

Tests are intentionally not in the menu — services' suite is too long-running to gate every review on.

---

## 9. Cleanup

*(`pr-local` only.)*

The NEXT STEPS footer names the current branch and the `git switch` to return to. Print it; never run it. The user may want to stay on the PR branch.

---

## 10. Error Playbook

| Condition | Behaviour |
|---|---|
| `gh` not installed (pr modes) | Print `Install gh: https://cli.github.com/`, abort. |
| `gh` not authenticated | Print `gh auth status` output + `Run: gh auth login`, abort. |
| PR number not parseable | Print usage, abort. |
| PR doesn't exist / wrong repo | Surface `gh` error verbatim, abort. |
| PR closed/merged | Prepend warning, proceed. |
| PR is draft | Prepend warning, proceed. |
| Working tree dirty (pr-local) | Print `git status --porcelain`, instruct stash/commit, abort. **No auto-stash.** |
| `gh pr checkout` fails (fork permission) | Degrade to static-diff mode, flag in report header. |
| Optional skill not installed | Continue using `rg` / general Rust knowledge. Do **not** abort. |
| `diff` mode but no diff (branch == main) | Print `No diff vs main — nothing to review.`, exit clean. |
| Verification command fails | Surface output in VERIFICATION block; raise findings only for issues inside changed files. |

**Rule of thumb:** never silently degrade. If a tool was missing or a step was skipped, the report's Mode/Header line should reflect it.

---

## 11. Maintenance Notes

- When the AI consistently misses a CoW-specific concern across multiple reviews, first try expressing it as one more bullet in [Universal Guardrails](#universal-guardrails). Only carve a sibling doc if it can't be expressed generically.
- When the skill produces a false-positive finding, add a one-line counter-example to the [Anti-nit Rule](#anti-nit-rule-mandatory).
- Keep this document under 500 lines.
