# Skill — pr-blame-walk

Use to investigate *"a symptom appeared in prod between T1 and T2 — which merged PR is the most likely cause?"*. Inputs are a query (the symptom + locating signal) and a merge window; output is a ranked list of suspect PRs, each with a tight context block and a one-sentence evidence-anchored *"Why suspected"*.

This is **not** a per-PR review — that is [`COW_PR_REVIEW_SKILL.md`](../COW_PR_REVIEW_SKILL.md). It is also not a causality proof: output is *suspects*, not *the* cause. Final attribution requires reproducing the symptom against a revert.

## Inputs

- `<query>` — the symptom and any locating signal: error string, endpoint, metric name, network, onset time. Examples:
  - *"500s on `POST /api/v1/orders` mainnet starting 2026-04-20 14:00 UTC"*
  - *"`auction_rewards` dropped 30% on Gnosis from 2026-04-15"*
  - *"settlement gas usage on mainnet up 2× since last week"*
- `<onset>` — the timestamp the symptom began, RFC3339 if known. Used as a hard cutoff: PRs merged after `<onset>` cannot be the cause.
- `<time_window>` — the merge window, ending at or before `<onset>`. Either a date range (`merged:YYYY-MM-DD..YYYY-MM-DD`) or a count (last N merged PRs).
- `<repo>` — `<owner>/<repo>` (defaults to `cowprotocol/services`).
- `<scope>` — optional path filter (e.g. `crates/autopilot/`, `crates/driver/src/domain/competition/`, `database/sql/`). Skip if the symptom is system-wide.

## Procedure

### 1. Enumerate candidates

```bash
gh pr list -R <repo> --state merged \
  --search "merged:<YYYY-MM-DD>..<YYYY-MM-DD>" \
  --json number,title,mergedAt,mergeCommit,author,additions,deletions \
  --limit 100
```

If the result hits `--limit`, narrow `<time_window>` and re-run — over 100 candidates means the window is too wide for the rubric to do useful work. Default `gh pr list` limit is 30; pass `--limit` explicitly.

### 2. Cheap pre-filter (no fetch)

Drop candidates without fetching their diff:

- **Merged after `<onset>`** — `mergedAt > <onset>`. Cannot be cause.
- **Pure dep bump** — title matches `^Bump `, `^chore: bump`, `RUSTSEC`, `dependabot`. Diff is `Cargo.toml` version bumps + `Cargo.lock` only.
- **Pure docs / CI workflow / formatting / test-only** — files confined to `docs/`, `.github/`, `*.md`, `**/tests/`. Keep CI changes only if `<query>` is itself a build/CI symptom.
- **Network mismatch** — title or label explicitly names a chain different from `<query>`'s network *and* the diff doesn't touch shared infra.

### 3. Per-candidate fetch (parallel, capped)

For each surviving candidate, in parallel — **cap concurrency at 5–10**. `gh` shares the GitHub API budget (5000 req/hr authenticated) with the rest of the session.

```bash
gh pr view <N> -R <repo> --json title,body,files,baseRefName,headRefName,labels
gh pr diff <N> -R <repo>
```

If `gh` returns `API rate limit exceeded`, pause and retry with smaller batches. Do not fall back to scraping the web UI.

### 4. Per-candidate context block

Run [`pr-context-synthesis`](pr-context-synthesis.md) per candidate with `<pr_text>` = title+body, `<linked_issue>` = referenced issue if any, `<diff_summary>` = file list + diff hunks. **Cap output at 3–4 sentences** — this skill needs the *what*, not the full *why/how*. The anti-vague-verb rule from that skill's §Rules applies verbatim.

If the diff revives or reverts code whose origin isn't obvious from the PR text, run [`git-blame-historic-context`](git-blame-historic-context.md) on the deleted lines (against `origin/main^`) to recover the originating commit/PR — useful for "Why suspected" when the suspect undoes a prior fix.

### 5. Score, rank, emit

Apply [Scoring rubric](#scoring-rubric). Drop everything below Medium. Sort by tier, then by tiebreaker. Emit per [Output](#output).

## Scoring rubric

For each surviving candidate, evaluate signals and pick the highest tier that applies:

| Tier | Signal |
|---|---|
| **High** | Touches a file/symbol/endpoint **explicitly named** in `<query>`. |
| **High** | Modifies behaviour the symptom directly describes (e.g. `<query>` says *"rewards dropped"* and the PR changes a reward calculation; *"500 on /quote"* and the PR touches the quote handler). |
| **High** | Touches a code path the symptom would naturally route through, on the same network/chain mentioned in `<query>`. |
| **Medium** | Same crate/module as a High signal but a different surface (sibling function, shared util). |
| **Medium** | Touches shared infra (gas estimation, RPC client, DB access, serialization, settlement queueing) that could plausibly chain through to the symptom. |
| **Drop** | Pure rename / move / formatting — no behaviour change. (`additions ≈ deletions` and the diff is `mv`-shaped or whitespace-only.) |
| **Drop** | Pure comment-only changes. |
| **Drop** | Wholly unrelated crate with no plausible chain to the symptom. |

(The pre-filter Drops in step 2 don't reach this rubric — they were dropped without fetching.)

### Tiebreakers

When two PRs land at the same tier:

1. **Smaller diff first.** Smaller change = cheaper to bisect / revert. `additions+deletions` is the proxy.
2. **Hot-path > cold-path.** A change in a request-handling code path beats a change to one-shot init or batch-job code.
3. **Multiple High signals stacked > single High signal.** Note the stack in *"Why suspected"*.

### Single-best mode

If exactly one candidate scores High and the next-highest is Medium-or-lower, surface only that one in *High suspects* with `(only High suspect; next is <tier>)` after the title. Mediums still print.

## Output

```
PR-blame-walk — <query>
Window:     <time_window>
Onset:      <onset or "—">
Repo:       <repo>
Scope:      <scope or "—">
Scanned:    <total candidates after enumeration>
Pre-filter: <count> dropped (post-onset, deps, docs/CI, network mismatch)
Surfaced:   <high count> high, <medium count> medium

───── High suspects ────────────────────────────────────────
1. #<N> — <title>                  [merged <YYYY-MM-DD> by @<author>; +<add>/-<del>]
   What: <3–4 sentence synthesis from pr-context-synthesis>
   Why suspected: <one or two sentences naming a file, symbol, or
                  behaviour change. Vague verbs forbidden.>
   Inspect: gh pr view <N> -R <repo> --web

2. ...

───── Medium suspects ─────────────────────────────────────
<same shape>
```

Empty result:

```
No suspects in window. Symptom likely originates outside this PR set:
config change, upstream dep, infra/RPC, or a PR predating the window.
Consider broadening <time_window>, removing <scope>, or checking
deploys (`gh api -X GET repos/<repo>/deployments?per_page=20`) and infra.
```

## Evidence integration

The skill itself doesn't run evidence queries — but the operator usually has them open already. Cite the query that would sharpen the score; let the operator run it.

| Evidence | Strengthens score when | Read-only query |
|---|---|---|
| **Victoria Logs** | Symptom onset matches the suspect's deploy time | `victorialogs_query` MCP, e.g. `container:!controller AND <symptom> \| fields _time, _msg, all` over the window straddling `mergedAt` |
| **postgres MCP** | Symptom is DB-shaped (timeouts, missing rows, migration drift) and the suspect added a migration | `mcp__postgres-protocol__query` with `SELECT` only; show the SQL before running |
| **Squash-commit → PR lookup** | You have a SHA from `git log` or a deploy log and need its PR | `gh pr list -R <repo> --search "<sha>" --state merged` |
| **Deploys vs merges** | Suspect deploy-time vs merge-time mismatch (a PR can merge hours before deploy) | `gh api -X GET repos/<repo>/deployments?per_page=20` |

The principle: name *which* evidence query would tip the score, not the result of running it.

## Rules

1. **Score from evidence, not vibes.** Every *"Why suspected"* line names a concrete file, symbol, or behaviour change. *"Looks suspicious"* / *"updates X"* / *"changes Y"* are failures — the anti-vague-verb rule from [`pr-context-synthesis`](pr-context-synthesis.md) §Rules applies verbatim.
2. **The query is the lens.** A PR risky in general but unrelated to `<query>` is not a suspect. This skill is not a retroactive review.
3. **Don't synthesise causality.** Output is *suspects*; final attribution requires a revert / bisect.
4. **Time-anchor.** Drop PRs merged after `<onset>` — they cannot be the cause. Watch for force-push edge cases: trust `mergeCommit.oid` and `gh pr view <N>`, not the squash subject's `(#NNNN)` alone.
5. **Read-only.** Every command in this skill is read-only: `gh` (`view` / `list` / `diff` / `api` GET-only), `git` (`log` / `blame` / `show`), MCP query tools (`SELECT` only). Never `gh pr review`, `gh pr comment`, mutating `gh api` verbs (`POST`/`PATCH`/`DELETE`), DB writes, or `git commit`/`push`/`checkout` of suspect branches.

## When to skip

- **Symptom not narrowed to a window yet** (e.g. *"things feel slow"*). Narrow via metrics/logs first, then run this skill.
- **Single suspect already in mind** — use `/review-pr <N>` or read the diff directly. This skill is N→1, not 1→1.
- **Symptom predates available PR history** — broaden the window or accept the cause is older than the rubric can see.
- **Symptom is config / infra / upstream** — there are no source-code PRs in this repo to scan; check deploys, config, RPC provider, or external dependencies instead.
- **Pre-filter drops everything** — the rubric has nothing to score; widen `<time_window>` or relax `<scope>`.

## Used by

- Ad-hoc incident investigations: paged on a fresh prod regression, run this before opening individual PRs.
- May be invoked as a follow-up from [`COW_ORDER_DEBUG_SKILL.md`](../COW_ORDER_DEBUG_SKILL.md) when an order-debug session pins a regression to a window with no obvious cause inside the order's lifecycle.
- [`COW_PR_REVIEW_SKILL.md`](../COW_PR_REVIEW_SKILL.md) is the *complement*, not a caller — once this skill surfaces a suspect, run `/review-pr <N>` on it.
