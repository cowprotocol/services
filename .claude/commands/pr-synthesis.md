---
description: Produce a tight 1–3 paragraph what / why / how synthesis of a single PR. Fetches title, body, linked issue, and diff scope; outputs per `docs/skills/pr-context-synthesis.md`. Useful as a standalone summary or as a building block for review / incident-investigation flows. Read-only.
---

Synthesise PR: $ARGUMENTS

## Parse $ARGUMENTS

Accept any of:

- A PR number: `4267`
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

Fetch in parallel:

```bash
gh pr view <N> -R <owner>/<repo> --json title,body,files,labels,baseRefName,headRefName,closingIssuesReferences
gh pr diff <N> -R <owner>/<repo>
```

For each entry in `closingIssuesReferences` (GitHub's own parsing of `Fixes #N` / `Closes #N` / `Resolves #N`, more reliable than regexing the body), fetch the issue:

```bash
gh issue view <issue.number> -R <issue.repository.owner>/<issue.repository.name> --json title,body,labels,state
```

If `closingIssuesReferences` is empty, proceed without a linked issue — never invent one.

Build:

- `<pr_text>` = title + body
- `<linked_issue>` = the fetched issue, if any
- `<diff_summary>` = a per-file note of the change scope (path + brief description, drawn from the diff hunks)

Then follow `docs/skills/pr-context-synthesis.md` Rules and Shape. Output the synthesis verbatim — no header, no metadata, no separator lines. The caller pastes this directly into a review thread, an incident report, or a Slack message.

## Rules

- **Read-only.** `gh pr view`, `gh pr diff`, `gh issue view`, `gh api` GET-only. No `gh pr review`, no `gh pr comment`, no mutating `gh api` verbs.
- **Synthesise, don't copy-paste.** If `<pr_text>` is sparse, say so plainly: *"description is minimal; intent inferred from diff"*.
- **No vague verbs.** *"This PR updates something"* is a failure. Name the component, the change, and the mechanism. The anti-vague-verb rule from the skill doc applies verbatim.
- **Watch for description-vs-diff drift.** `<pr_text>` must describe `<diff_summary>`'s current state. If a claim is no longer true of the diff, note it in the synthesis as *"description claims X; diff shows Y"*.
