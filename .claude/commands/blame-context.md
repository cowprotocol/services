---
description: Recover historic context for a file:line via `git blame` plus the squash-PR pivot. Wraps `docs/skills/git-blame-historic-context.md` end-to-end and prints the strengthens / weakens / inverts decision so the caller can weight a finding before flagging suspicious-looking code. Read-only.
---

Look up historic context for: $ARGUMENTS

## Parse $ARGUMENTS

Accept any of:

- `path:line` — single line. e.g. `crates/driver/src/.../settlement.rs:444`
- `path:start-end` — line range. e.g. `crates/driver/src/.../settlement.rs:434-447`
- `path line` or `path start-end` — space-separated form (handy when paths contain `:`).

If `$ARGUMENTS` is empty or unparseable, print the usage block and abort:

```
Usage: /blame-context <path>:<line>
       /blame-context <path>:<start>-<end>
```

## Procedure

Follow `docs/skills/git-blame-historic-context.md` end-to-end. Concretely:

1. `git blame -L <start>,<end> -- <path>` to find the originating commit.
2. If the surface looks like a wholesale move/refactor (same author across many lines, recent date, identical hashes), retry with `git blame -w -C -C -C -L <start>,<end> -- <path>` to recover the real authoring commit.
3. `git log -1 --format='%s%n%b' <sha>` for the commit body.
4. If the subject ends with `(#NNNN)`, pivot to the PR conversation: `gh pr view <NNNN>` (the gh CLI infers the repo from the working directory). The PR body is usually richer than the squash commit alone.

Then print the report below.

## Output

```
─── Blame for <path>:<lines>
<sha>  <author>  <date>  <subject>

─── Originating commit / PR
<commit body, OR PR title + body if a (#NNNN) PR was found>

─── Decision
<Strengthens | Weakens | Inverts> the suspicion that this code is unusual.
<one-sentence reason naming the concrete signal from the originating PR/commit>
Action: <keep finding | downgrade to Question | drop>
```

## Rules

- **Read-only.** `git blame`, `git log`, `git show`, `gh pr view`, `gh api` GET-only. No `git commit`, no `git checkout`, no mutating `gh api` verbs.
- **Don't comment on the PR.** Don't edit files. The caller owns what to do with the decision — this command just supplies the evidence.
- **Don't invent a decision.** If blame surfaces nothing useful (line is brand new, generator-output, vendored), print `Decision: insufficient signal` and explain why.
