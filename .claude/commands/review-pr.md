---
description: Produce a structured PR review for cowprotocol/services. Invoked locally as `/review-pr` (diff mode, against current branch vs main) or `/review-pr <N|url>` (PR mode). Same command also runs in CI via `.github/workflows/claude-code-review.yml`, where it posts a single review comment instead of printing to terminal. Read-only in local modes; the user posts any comments manually.
---

Review PR: $ARGUMENTS

Follow the instructions in `./docs/COW_PR_REVIEW_SKILL.md` to produce the review report.

## Prologue (execute in order; abort on any failure)

### 1. Detect mode

The skill runs in one of three modes. Detection:

- If the environment variable `$GITHUB_ACTIONS == "true"` → `mode = "pr-ci"`.
  - `$ARGUMENTS` MUST be a PR number, URL, or `owner/repo#N` form (the workflow passes it).
- Else if `$ARGUMENTS` is non-empty → `mode = "pr-local"`. Parse the argument (see [step 2](#2-parse-arguments-pr-modes-only)).
- Else (`$ARGUMENTS` empty, not in CI) → `mode = "diff"`. No `gh` calls needed; source is `git diff $(git merge-base HEAD origin/main)..HEAD` (the actual command runs in step 3 below, after fetching `origin/main`).

### 2. Parse $ARGUMENTS (PR modes only)

*(Skip in `diff` mode.)*

Accept any of:

- A PR number: `4267`
- A full URL: `https://github.com/cowprotocol/services/pull/4267`
- An `owner/repo#N` form: `cowprotocol/services#4267`

Default `owner/repo` to `cowprotocol/services` when only a number is given.

Extract: `<PR_NUMBER>`, `<owner>`, `<repo>`.

If unparseable, print and abort:

```
Usage: /review-pr                       # diff mode (current branch vs main)
       /review-pr <PR_NUMBER>           # PR mode
       /review-pr https://github.com/owner/repo/pull/<N>
       /review-pr owner/repo#<N>
```

### 3. Diff-mode preflight

*(Only in `mode == "diff"`.)*

Run:

```bash
git fetch origin main --quiet
BASE=$(git merge-base HEAD origin/main)
git diff --stat "$BASE..HEAD"
```

If `git diff "$BASE..HEAD"` is empty, print `No diff vs main — nothing to review.` and exit clean (not an error).

There is **no** clean-tree check, **no** rebase, and **no** `git pull` of main. Diff scope comes from the fetched `origin/main` merge-base.

### 4. PR-mode preflight

*(Only in `mode == "pr-local"` or `mode == "pr-ci"`.)*

#### 4a. Working tree (pr-local only)

Run `git status --porcelain`. If non-empty, print it plus:

```
Working tree is dirty. Stash or commit your changes, then re-run.

  git stash        # temporary
  git stash pop    # to restore later
```

Then **abort**. Never auto-stash.

#### 4b. Save the current branch (pr-local only)

Save the current branch name to `<prior_branch>` so the report's NEXT STEPS footer can suggest `git switch <prior_branch>` when the review is done.

#### 4c. Fetch base ref

Run `git fetch origin --quiet`. This makes the diff comparable to base without rebasing or modifying the user's branch.

#### 4d. Checkout the PR

Run `gh pr checkout <PR_NUMBER> -R <owner>/<repo>`. Failure handling:

- **`gh` not installed** → print `Install gh: https://cli.github.com/` and abort.
- **Auth error** → print `gh auth status` output plus `Run: gh auth login` and abort.
- **PR doesn't exist / wrong repo** → surface `gh`'s error verbatim and abort.
- **Fork without checkout permission** → switch to **degraded static-diff mode**:
  - Set `mode_qualifier = "degraded static-diff"`.
  - In the reference doc's §2, replace `gh pr diff <N>` with `gh pr diff <N> --patch -R <owner>/<repo>`.
  - Flag the qualifier in the report header's `Mode:` line.
- **Any other error** → surface verbatim and abort.

In `pr-ci`, the workflow has already checked out the PR branch — skip 4d and just verify `HEAD` matches the expected ref.

### 5. Optional-tooling probe

Detect which optional accelerators are available in the current session: Serena MCP (`mcp__plugin_serena_serena__*`), `actionbook/rust-skills` (`rust-call-graph`, `rust-symbol-analyzer`, `rust-trait-explorer`, `rust-code-navigator`), `ra-qm-skills`, the `m04`/`m06`/`m07`/`m15` modules, `unsafe-checker`.

Build a `loaded_context` list of whichever ones resolved. Pass it through to the reference doc; it prints verbatim in the report header's `Loaded context:` line.

Do not abort if a skill is missing. Do not print install banners.

### 6. Noise filter (before handoff)

Classify each changed file:

**Review surface (read fully):**

- Anything under `crates/*/src/**/*.rs` (excluding `contracts/generated/**`).
- `crates/e2e/tests/**/*.rs`.
- `contracts/solidity/**/*.sol` (authored Solidity).
- Config files: `**/openapi.yml`, `database/sql/**`, `configs/**`, semantically interesting `*.toml`.

**Noise (skip or skim):**

- `Cargo.lock` (any). CI validates the resolution; reviewing the lockfile diff is high-cost, low-signal.
- `contracts/generated/**` and `contracts/artifacts/**` — machine-generated bindings and ABI JSON.
- Auto-generated `Cargo.toml` entries from contract-binding crates.
- Binary fixtures, ABI blobs, snapshot files.

Report the filter in the report's `Scope:` line as `+X −Y across Z files (~N LOC human-written; rest generated/lockfile, filtered)`.

### 7. Handoff

Read `docs/COW_PR_REVIEW_SKILL.md` and follow it from §2 (Metadata Fetch) onward, passing through:

- `mode` (`diff` / `pr-local` / `pr-ci`) and `mode_qualifier` if set.
- `<PR_NUMBER>`, `<owner>`, `<repo>` (PR modes only).
- `prior_branch` (`pr-local` only).
- `loaded_context` (optional skills detected in step 5).
- The noise-filter classification from step 6 — the reference doc uses it to decide what to read.
