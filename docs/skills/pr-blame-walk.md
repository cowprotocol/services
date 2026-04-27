# Skill — pr-blame-walk

Use to investigate *"did one of the last N merged PRs cause X?"* — a regression, alert, metric drop, or fresh prod incident. Inputs are a query (the symptom) and a time window of merged PRs; output is a ranked list of suspect PRs with evidence per suspect.

This is **not** a per-PR review. The unit of work is a window of merged PRs scored against an external observation (an alert, a regression report, a metric drop), not a single PR being assessed for merge-readiness. For per-PR review, use [`COW_PR_REVIEW_SKILL.md`](../COW_PR_REVIEW_SKILL.md).

## Inputs

- `<query>` — what we're hunting. The symptom and any locating signal: error string, endpoint, metric name, network, onset time. Examples:
  - *"500s on `POST /api/v1/orders` mainnet starting 2026-04-20 14:00 UTC"*
  - *"`auction_rewards` dropped 30% on Gnosis from 2026-04-15"*
  - *"settlement gas usage on mainnet up 2× since last week"*
- `<time_window>` — the merge window to scan. Either a date range (`merged:2026-04-15..2026-04-20`) or a count (last N merged PRs). The window must bracket the symptom's onset; over-broad windows produce noise.
- `<repo>` — `<owner>/<repo>` (defaults to `cowprotocol/services`).
- `<scope>` — optional path filter (e.g. `crates/autopilot/`, `crates/driver/src/domain/competition/`, `database/sql/`) to narrow candidates when the symptom localises clearly. Skip if the symptom is system-wide.

## Procedure

### 1. Enumerate candidates

```bash
gh pr list -R <repo> --state merged --search "<time_window>" \
  --json number,title,mergedAt,mergeCommit,author,files,additions,deletions \
  --limit 100
```

If `<scope>` is set, drop PRs whose `files[].path` list doesn't intersect it. Drop pure docs/lockfile/dep-bump PRs unless the symptom is a build/CI issue.

### 2. Per-candidate fetch (parallel)

For each surviving candidate, in parallel:

```bash
gh pr view <N> -R <repo> --json title,body,files
gh pr diff <N> -R <repo>
```

### 3. Per-candidate context block

Run [`pr-context-synthesis`](pr-context-synthesis.md) with `<pr_text>` = title+body, `<linked_issue>` = the referenced issue (if any), `<diff_summary>` = file list + diff hunks. Keep the output to **one paragraph** — this skill needs the *what*, not the full *why/how*.

### 4. Score against the query

Apply the rubric below. Drop everything that scores below Medium. Keep High and Medium.

### 5. Rank and emit

Sort by score (High first, then Medium). Output uses the shape in [Output](#output).

## Scoring rubric

For each PR, evaluate signals and pick the highest tier that applies:

| Tier | Signal |
|---|---|
| **High** | Touches a file/symbol/endpoint **explicitly named** in `<query>`. |
| **High** | Modifies behaviour the symptom directly describes (e.g. `<query>` says "rewards dropped" and the PR changes a reward calculation; or `<query>` says "500 on /quote" and the PR touches the quote handler). |
| **High** | Touches a code path the symptom would naturally route through, on the same network/chain mentioned in `<query>`. |
| **Medium** | Touches the same crate/module as a High signal but a different surface (sibling function, shared util). |
| **Medium** | Touches shared infra (gas estimation, RPC client, DB access, serialization) that could plausibly chain through to the symptom. |
| **Drop** | Pure docs / dep bumps / lockfile / formatting / test-only / CI config — unless `<query>` is itself a build/CI symptom. |
| **Drop** | Touches a wholly unrelated crate with no plausible chain to the symptom. |

If two High signals stack (e.g. names the symbol *and* touches the same network's config), note that in the "Why suspected" line — it's a stronger lead than a single signal.

## Output

```
PR-blame-walk — <query>
Window:     <time_window>
Repo:       <repo>
Scope:      <scope or "—">
Scanned:    <total candidates after scope filter>
Surfaced:   <high count> high, <medium count> medium

───── High suspects ────────────────────────────────────────
1. #<N> — <title>                  [merged <YYYY-MM-DD> by @<author>]
   What: <one-paragraph synthesis from pr-context-synthesis>
   Why suspected: <one or two sentences anchoring the score against
                  the query — must name a file, symbol, or behaviour
                  change>
   Inspect: gh pr view <N> -R <repo> --web

2. ...

───── Medium suspects ─────────────────────────────────────
<same shape>
```

If zero candidates score High or Medium:

```
No suspects in window. Symptom likely originates outside this PR set:
config change, upstream dep, infra/RPC, or a PR that predates the window.
Consider broadening <time_window> or removing <scope>.
```

## Rules

1. **Score from evidence, not vibes.** Every "Why suspected" line must point at a concrete file, symbol, or behaviour change. *"Looks suspicious"* is not an answer.
2. **The query is the lens.** A PR that is risky in general but unrelated to `<query>` is not a suspect. This skill is not a retroactive review — that is what the per-PR review skill is for.
3. **Don't synthesise causality.** The output identifies *suspects*, not *the* cause. Final attribution requires reproducing the symptom against a revert (or the equivalent) of the suspect.
4. **Time-anchor the window.** If `<query>` includes "started at 14:00 UTC on 2026-04-20", the window must bracket that timestamp. A multi-month sweep produces noise, not signal.
5. **Read-only.** Never run mutating `gh` verbs, never post on GitHub from this skill, never check out a suspect's branch. The output goes to terminal.

## When to skip

- The symptom predates the available PR history in the window — broaden the window or accept that the cause is older.
- The symptom is clearly config / infra / upstream — there are no PRs in this repo to scan.
- A single-PR investigation is what's needed (you already have a suspect) — use the per-PR review skill (`/review-pr <N>`) instead.
- Symptom hasn't been narrowed enough for the rubric to do work (e.g. *"things feel slow"*) — narrow the symptom first via logs / metrics, then run this skill.
