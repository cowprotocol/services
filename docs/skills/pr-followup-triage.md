# Skill — pr-followup-triage

Use when a PR under review already has prior inline review comments and the author has pushed new commits since. Classifies each prior comment as **Addressed**, **Discussion needed**, **Pending**, **Silently dropped**, **Moot**, or **Unclear** at the current HEAD, each with a citation the reviewer can click through to verify in one step.

This is a *triage* skill: it surfaces evidence so the human reviewer can decide whether to resolve a GitHub conversation. It never resolves threads itself, never re-quotes the original comments, and never tries to read sentiment from emoji reactions or upvotes.

Read-only: only `gh api` GET verbs and `git` read commands. Never `gh pr review`, mutating `gh api` verbs (`POST` / `PATCH` / `DELETE`), or comment-resolution endpoints.

## Inputs

- `<PR_NUMBER>`, `<owner>`, `<repo>` — same as [`COW_PR_REVIEW_SKILL.md`](../COW_PR_REVIEW_SKILL.md).
- `<head_sha>` — current PR head, already fetched in [§2 of the main skill](../COW_PR_REVIEW_SKILL.md#2-metadata-fetch).
- `<reviewer_filter>` — logins to track. Defaults:
  - `pr-local`: the `gh`-authenticated user (`gh api user --jq .login`). If that call fails, fall back to "all human reviewers".
  - `pr-ci`: all human reviewers (`user.type == "User"`; excludes `*[bot]`).

## When to skip

- No reviews on the PR (zero rows from `gh api repos/<owner>/<repo>/pulls/<N>/reviews`).
- No reviews from any login in `<reviewer_filter>`.
- For *every* filtered reviewer, their latest review's `commit_id` equals `<head_sha>` — no commits since their last round, so nothing has changed.

The third gate is per-reviewer: triage continues for any reviewer whose latest review predates `<head_sha>`. Only when *all* of them are caught up do we skip the section entirely.

In any skip case, omit the "Prior-comment follow-up" section from the report entirely. Do not print a placeholder.

## Procedure

### 1. Enumerate prior reviews

```bash
gh api repos/<owner>/<repo>/pulls/<PR_NUMBER>/reviews --paginate
```

Drop reviews where `state == "DISMISSED"`. Drop reviews from logins outside `<reviewer_filter>`.

For each remaining reviewer, keep only their **latest** review (by `submitted_at`). Older rounds are assumed superseded — by either the reviewer's later round or the new commits.

If nothing remains, skip per [When to skip](#when-to-skip).

### 2. Fetch the inline comments

```bash
gh api repos/<owner>/<repo>/pulls/<PR_NUMBER>/comments --paginate
```

Filter to comments whose `pull_request_review_id` matches one of the latest reviews from step 1.

For each comment `c`, record:
- `c.id`, `c.path`, `c.original_line` (stable; use this), `c.commit_id` (the commit it was made against), `c.body`.
- Replies — every comment whose `in_reply_to_id == c.id`, sorted by `created_at` ascending.

A review with only a top-level body and no inline comments is general feedback the reviewer can recall directly; it produces no triage entry.

### 3. Classify each comment

For each `c`:

#### a. File-level moot

If `c.path` does not exist at `<head_sha>` (`gh api 'repos/<owner>/<repo>/contents/<c.path>?ref=<head_sha>'` returns 404), check whether the file was *renamed* by scanning the compare endpoint:

```bash
gh api 'repos/<owner>/<repo>/compare/<c.commit_id>...<head_sha>' \
  --jq '.files[] | select(.previous_filename == "<c.path>") | .filename'
```

- If a rename target exists, treat that as the new `c.path` for steps b–d and record the rename in the cite line.
- Otherwise, status = **Moot (file removed)**. Done.

#### b. Code-change detection

Pull the file at the two commits and locate the function/block containing `c.original_line`:

```bash
gh api 'repos/<owner>/<repo>/contents/<c.path>?ref=<head_sha>'      --jq '.content' | base64 -d
gh api 'repos/<owner>/<repo>/contents/<c.path>?ref=<c.commit_id>'   --jq '.content' | base64 -d
```

Compare the relevant span (function body, struct definition, or surrounding ~20 lines if no clean enclosing scope):

- **Block removed** in HEAD → status = **Moot (code removed)**. Note which commit removed it via `git log <c.commit_id>..<head_sha> -- <c.path>` (run locally; falls back to `gh api .../compare/...` in degraded static-diff mode).
- **Span identical** → `code_change = false`. Move to step c.
- **Span differs** → `code_change = true`. Capture the new content.

#### c. Reply detection

Inspect the reply chain for `c`:

- No replies → `reply = none`.
- Latest reply is short and forward-looking (`"will fix"`, `"good catch"`, `"TODO"`, `"later"`) → `reply = commitment`.
- Latest reply asserts done or argues a position (`"done"`, `"fixed in <sha>"`, `"no, because X"`) → `reply = substantive`.

Don't over-classify; if the reply is ambiguous, treat as `substantive` and let the human read it.

#### d. Final status

| `code_change` | `reply` | Status |
|---|---|---|
| yes, addresses the comment | any | **Addressed** |
| yes, diverges from the comment | any | **Discussion needed** |
| yes, can't tell from the diff | any | **Unclear** |
| no | `substantive` | **Discussion needed** |
| no | `commitment` | **Pending** |
| no | `none` | **Silently dropped** |

Whether a code change "addresses" the comment is an LLM judgement. **Be conservative.** Default to **Unclear** when in doubt — a false **Addressed** misleads the reviewer into resolving a thread that should stay open.

## Output

Append one section to the main report, between Codemap and CONTEXT:

```
Prior-comment follow-up — @<reviewer> at <prior_sha_short>
───────────────────────────────────────────────────────────
[A] <c.path>:<original_line>
    Asked:   <≤12-word recap of the comment's request>
    Status:  ✓ Addressed
    Cite:    <c.path>:<line at HEAD>  ← changed in <commit_short>; reads
             "<2-line excerpt of the new code>"

[B] <c.path>:<original_line>
    Asked:   ...
    Status:  💬 Discussion needed
    Cite:    Author replied: "<≤30-char excerpt>"; <c.path>:<line> unchanged.

[C] <c.path>:<original_line>
    Asked:   ...
    Status:  ⏳ Pending
    Cite:    Author replied "will fix" but <c.path>:<line> unchanged.

[D] ...
    Status:  ⚠ Silently dropped
    Cite:    No reply; <c.path>:<line> unchanged since <prior_sha_short>.

[E] ...
    Status:  🚫 Moot (file removed in <commit_short>)

[F] ...
    Status:  ❓ Unclear
    Cite:    Code at <c.path>:<line> changed in <commit_short>, but the
             new shape doesn't obviously match what was asked. Re-read
             the thread.
```

Sort entries by status, with what-needs-attention first: `Discussion needed` → `Pending` → `Silently dropped` → `Unclear` → `Addressed` → `Moot`. The reviewer's eye lands first on what still needs attention.

Stable identifiers `[A]`, `[B]`, ... let new findings reference an old comment with `Re: [A]`.

If `<reviewer_filter>` matched multiple logins, group by reviewer with a sub-heading.

## Rules

1. **Conservative defaults.** When the LLM cannot tell whether a code change addresses a comment, status is **Unclear**, never **Addressed**. False "Addressed" is worse than no claim — it tricks the reviewer into closing a thread that shouldn't close.
2. **Cite, don't paraphrase.** Every `Cite:` line ends with a `<path>:<line>` at HEAD, a `"removed in <commit>"`, or a short reply excerpt. The reviewer must be one click from verifying.
3. **No sentiment parsing.** Don't infer satisfaction from `:+1:` reactions or "ok thanks". The reviewer makes that judgement, not the AI.
4. **Don't quote the original comment.** The reviewer wrote it; they remember. The output's job is the *delta* — what changed about the comment's status since it was made.
5. **Read-only.** No `gh pr review`, no comment-resolution endpoints, no mutating verbs. Even if the AI is certain everything is fine, the human resolves.
6. **One round per reviewer.** Triage the latest review per reviewer only. Earlier rounds are out of scope; if they matter, the reviewer leaves a fresh round.

## Used by

- [`COW_PR_REVIEW_SKILL.md`](../COW_PR_REVIEW_SKILL.md) — invoked from the new follow-up triage step when prior human reviews exist on the PR.
