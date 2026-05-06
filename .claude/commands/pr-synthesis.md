---
description: Produce a tight 1–3 paragraph what / why / how synthesis of a single PR. Fetches title, body, linked issue, and diff scope; outputs per `docs/skills/pr-context-synthesis.md`. Read-only.
---

Synthesise PR: $ARGUMENTS

## Parse $ARGUMENTS

Accept any of:

- A PR number: `4267`/`#4267`
- A full URL: `https://github.com/cowprotocol/services/pull/4267`
- An `owner/repo#N` form: `cowprotocol/services#4267`

Default `owner/repo` to `cowprotocol/services` when only a number is given.

If `$ARGUMENTS` is empty or unparseable, print:

```
Usage: /pr-synthesis <PR_NUMBER>
       /pr-synthesis https://github.com/owner/repo/pull/<N>
       /pr-synthesis owner/repo#<N>
```

and abort.

## Procedure

Fetch in this order — context first, then diff. Lets the synthesis read the diff with the author's intent already in mind, and lets `gh pr view` report `additions`/`deletions` so the diff fetch can be sized appropriately.

**Step 1 — PR metadata + closing issues.** Cheap; bounded size.

```bash
gh pr view <N> -R <owner>/<repo> --json title,body,files,additions,deletions,labels,baseRefName,headRefName,closingIssuesReferences
```

**Step 2 — linked issues.** For each entry in `closingIssuesReferences` (GitHub's own parsing of `Fixes #N` / `Closes #N` / `Resolves #N`, more reliable than regexing the body, and follows cross-repo references correctly), fetch the issue:

```bash
gh issue view <issue.number> -R <issue.repository.owner>/<issue.repository.name> --json title,body,labels,state
```

If `closingIssuesReferences` is empty, proceed without a linked issue — never invent one.

**Step 3 — diff fetch (size-gated).** Use `additions + deletions` from step 1 to decide:

- **`additions + deletions <= 2000`** — fetch the full diff:

  ```bash
  gh pr diff <N> -R <owner>/<repo>
  ```

- **`additions + deletions > 2000`** — *do not* fetch the full diff. It can blow the context window (e.g. PR #4217 = 376k lines). Get the complete per-file list via the paginated REST endpoint — `gh pr view --json files` silently caps at 100 files, which underreports scope on big PRs:

  ```bash
  gh api --paginate "repos/<owner>/<repo>/pulls/<N>/files" \
    --jq '.[] | {filename, status, additions, deletions}'
  ```

  Build `<diff_summary>` from these per-file records (`filename`, `additions`, `deletions`, `status` — added / modified / renamed / removed). Bucket lockfiles, generated bindings, vendored artifacts, and codegen output — call them out as a single line each, not file-by-file. State in the synthesis that the diff was summarised at file-scope only.

Build:

- `<diff_summary>` = full hunks (small PR) or per-file scope buckets (large PR). The ground truth.
- `<pr_text>` = title + body
- `<linked_issue>` = fetched issue(s), if any

Then follow `docs/skills/pr-context-synthesis.md` Rules and Shape. Output the synthesis verbatim — no header, no metadata, no separator lines. The caller pastes this directly into a review thread, an incident report, or a Slack message.

## Rules

- **Read-only.** `gh pr view`, `gh pr diff`, `gh issue view`, `gh api` GET-only. No `gh pr review`, no `gh pr comment`, no mutating `gh api` verbs.
- **Don't fetch a diff you can't read.** The 2000-line gate above is a hard preflight, not a suggestion. Above the gate, the per-file scope is enough to write the synthesis honestly; trying to swallow a 100k-line diff just truncates the context and corrupts the output silently.
- **Synthesise, don't copy-paste.** If `<pr_text>` is sparse, say so plainly: *"description is minimal; intent inferred from diff"*.
- **No vague verbs.** *"This PR updates something"* is a failure. Name the component, the change, and the mechanism. The anti-vague-verb rule from the skill doc applies verbatim.
- **Watch for description-vs-diff drift.** `<pr_text>` must describe `<diff_summary>`'s current state. If a claim is no longer true of the diff, note it in the synthesis as *"description claims X; diff shows Y"*. Fetching the issue + metadata before the diff is what lets you spot these — read the intent first, then check whether the diff matches.
