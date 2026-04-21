---
description: Produce a structured local review report for a CoW services PR (read-only; user posts comments manually)
---

Review PR: $ARGUMENTS

Follow the instructions in `./docs/COW_PR_REVIEW_SKILL.md` to produce the review report.

## Prologue (execute in order; abort on any failure)

### 1. Parse $ARGUMENTS

Accept any of:
- A PR number: `4267`
- A full URL: `https://github.com/cowprotocol/services/pull/4267`
- An `owner/repo#N` form: `cowprotocol/services#4267`

Default owner/repo to `cowprotocol/services` if only a number is given.

Extract: `<PR_NUMBER>`, `<owner>`, `<repo>`.

If unparseable, print this usage and abort:
```
Usage: /review-pr <PR_NUMBER>
       /review-pr https://github.com/owner/repo/pull/<N>
       /review-pr owner/repo#<N>
```

### 2. Prereq check

**Hard (abort if missing):**

Check that `<RUST_SKILLS_PATH>` exists as a directory. If missing, print and **abort**:
```
✗ Required skill missing: actionbook/rust-skills

  Install:
    npx skills add actionbook/rust-skills

  Then exit this Claude session (/exit) and restart with:
    claude --continue
```

**Soft (warn and continue if missing):**

Check that `<QM_PATH>` exists as a directory. If missing, print the banner below and **continue**:
```
⚠ Optional skill missing: alirezarezvani/claude-skills/ra-qm-team
  Install (recommended):
    npx ai-agent-skills install alirezarezvani/claude-skills/ra-qm-team
```

Build a `loaded_context` list containing the prereq skills that ARE installed. This list appears in the report header's `Loaded context:` line.

> **Note to maintainer:** Replace `<RUST_SKILLS_PATH>` and `<QM_PATH>` with the absolute paths captured in `docs/superpowers/plans/2026-04-21-pr-review-skill.notes.md` (§0.1). They're left as placeholders because the installer paths are determined empirically post-restart.

### 3. Clean-tree check

Run `git status --porcelain`. If output is non-empty, print it plus the following message, then **abort**:
```
Working tree is dirty. Stash or commit your changes, then re-run.

  git stash        # temporary
  git stash pop    # to restore later
```

**Never auto-stash.**

### 4. Update main

Save the current branch name to `<prior_branch>` (this variable is used again in step 8 of the reference doc).

Then:
- If the current branch IS `main`: run `git pull --rebase origin main`.
- Otherwise: run `git fetch origin main` (do **not** rebase the user's feature branch silently).

On rebase conflict, abort and print:
```
Rebase conflict on main. Resolve manually, then re-run.

Conflicted files:
<output of: git diff --name-only --diff-filter=U>
```

### 5. Checkout PR

Run `gh pr checkout <PR_NUMBER> -R <owner>/<repo>`.

On failure, handle specifically:

- **`gh` not installed** → print `Install gh: https://cli.github.com/` and abort.
- **Auth error** → print the output of `gh auth status` plus `Run: gh auth login` and abort.
- **PR doesn't exist / wrong repo** → surface `gh`'s error verbatim and abort.
- **Fork without checkout permission** → switch to **degraded static-diff mode**:
  - Do **not** abort.
  - Set `mode = "degraded static-diff"`.
  - In step 6 of the reference doc, replace the `gh pr diff` call with:
    ```bash
    gh pr diff <PR_NUMBER> --patch -R <owner>/<repo>
    ```
  - Flag the degraded mode prominently in the report header's `Mode:` line.
- **Any other error** → surface verbatim and abort.

If successful, `mode = "full checkout"`.

### 6. Noise filtering (before handoff)

CoW services PRs frequently bundle generated/mechanical files with the real change. Before handing off, classify each changed file in the diff:

**Review surface (read fully):**
- Anything under `crates/*/src/**/*.rs` (excluding `contracts/generated/**`).
- `crates/e2e/tests/**/*.rs`.
- `contracts/solidity/**/*.sol` (authored Solidity).
- Config files: `**/openapi.yml`, `database/sql/**`, `configs/**`, `*.toml` (only if semantically interesting — dep version bumps with CI green are generally fine).

**Noise (skip or skim):**
- `Cargo.lock` (any): reviewing lockfile diffs is high-cost, low-signal. If CI is green, cargo resolved the graph; a lockfile review won't catch what CI missed. Skip.
- `contracts/generated/**` and `contracts/artifacts/**`: machine-generated bindings and ABI JSON. Skip unless the PR is explicitly *about* the binding generation process.
- Auto-generated `Cargo.toml` entries from contract-binding crates (`contracts/generated/*/Cargo.toml`): skip.
- Binary fixtures, ABI blobs, snapshot files: skip.

Report the filter in the report's header `Scope:` line as `+X −Y across Z files (~N LOC human-written; rest generated/lockfile, filtered)`. This tells the human reviewer the *real* surface size — a 4000-line diff is often 400 lines of actual review.

### 7. Handoff

Read `docs/COW_PR_REVIEW_SKILL.md` and follow it from **§2. Metadata Fetch** onward, passing through:

- `<PR_NUMBER>`, `<owner>`, `<repo>`
- `<prior_branch>` (the branch to return to at the end)
- `loaded_context` (list of installed prereq skills)
- `mode` (`full checkout` or `degraded static-diff`)
- The filter list from step 6 — the reference doc uses it to decide what to `Read`.
