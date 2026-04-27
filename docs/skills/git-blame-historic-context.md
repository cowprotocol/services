# Skill — `git blame` for historic context

Use before flagging code that looks unusual, redundant, accidental, or "easy to clean up". Often that code looks weird because it had to.

## Procedure

```bash
git blame -L <start>,<end> -- <file>            # who/what/when
git log --format='%H %s' -n 1 <commit>          # commit message
gh pr view <#> -R <owner>/<repo>                # if commit message links a PR
```

## Decision

If the originating commit message or PR explains *why* the code is shaped the way it is, factor that into the finding. A "this looks accidental, did you mean X?" comment is much weaker when blame shows a deliberate fix from six months ago — and risks asking the author to undo a hard-won change.

Mention what blame revealed in the finding's Explanation so the reviewer can defend the point without re-running the lookup.

## When to skip

- Lines entirely new in the diff under review (no history yet).
- Pure additions of new symbols (nothing to blame).
- Generated code — blame the generator's input instead.
