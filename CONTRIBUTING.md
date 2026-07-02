# Contributing to CoW Protocol Services

Thanks for your interest in contributing. Before opening an issue or PR, please read this guide in full — it sets out how external contributions are evaluated.

A few things worth knowing upfront:

- We welcome external contributions, but the bar to land code here is high and the surface area open to external work is deliberately limited.
- Core-team work and external contributions do not share the same review priority. External PRs are reviewed on a best-effort basis.
- Not every issue is open for external work. Some are reserved for the core team, and the labels below are how we communicate that.

We'd rather be upfront about this than have anyone waste a weekend on a PR we can't merge.

## Where to participate

GitHub is the only venue where contribution proposals are evaluated. Conversations on Discord, Twitter, or email do not count as approval to start coding.

For general user questions and troubleshooting that aren't bug reports, the [CoW Protocol Discord](https://discord.gg/cowprotocol) is the right place.

For security disclosures, please reach out at `security@cow.fi`. **Do not file vulnerabilities as public issues, and do not disclose them publicly without the team's confirmation.**

## How issues are labeled

Every label below is a signal about whether and how external work is welcomed. Maintainers apply these labels manually as part of triage.

- **`core team only`** — actively being worked on by the team, or planned for the near-term roadmap. Please do not open PRs against these.
- **`needs discussion`** — the default state for newly opened external issues. Maintainers have not yet decided whether the work is wanted, in scope, or correctly framed. Code work should not start yet.
- **`accepting contributions`** — scope is agreed and external contributors are welcome to pick this up.
- **`good first issue`** — small, well-scoped, and a good fit for someone new to the codebase.
- **`help wanted`** — explicitly tagged for community pickup, with a 100 DAI bounty attached (see [Reward program](#reward-program)). Applied manually by maintainers.

**Absence of any of these labels means the issue is not greenlit for external work.** Treat unlabeled issues as if they were `core team only` until a maintainer says otherwise.

## Opening an issue

For bugs, feature requests, or contribution proposals, open a GitHub Issue. Please include:

- The problem or use case driving the request.
- Why the change is worth doing: what it fixes, enables, or improves.
- The proposed approach, if you have one in mind.
- Alternatives you've already considered.

> In the name of practicality, you may open a *draft* PR as a instead of an issue, the code should help illustrate your point and evaluate how big of a change it is.

### From proposal to approval

1. **File the issue.** It will land as `needs discussion`.
2. **Triage.** A maintainer reads it and either asks for clarification, applies `core team only`, closes as out of scope, or moves to scoping.
3. **Scoping.** Discussion in the issue settles the approach.
4. **Greenlight.** The maintainer applies `accepting contributions`, `help wanted`, or `good first issue`. The issue is now ready to be worked on.
5. **Claim and implement.** Comment to claim the issue, get it assigned, and open a draft PR linked to it. Significant changes to the agreed scope should be raised back in the issue, not in PR review.

Claimed issues with no activity for two weeks return to the unclaimed pool.

## Submitting a pull request

Once an issue is greenlit, fork the repository, link your PR back to the issue, and follow the PR template carefully.

A CLA bot will prompt you to sign on your first contribution. Please do.

### PR size and scope

Reviews work best on small, focused PRs. Before opening one, please use the following as a self-check — if your change matches any of these, take it back to the issue and agree the approach with maintainers first:

1. Changes more than 200 lines of production code (excluding tests, generated contract bindings and related artifacts, and documentation).
2. Touches more than one crate, module, or service.
3. Modifies a public API or wire format.
4. Introduces a new feature flag, configuration option, or environment variable.
5. Refactors code unrelated to the stated goal of the PR.

## Reviews and expectations

The team's review capacity is finite, and we'd rather set realistic expectations than leave you guessing.

- **Core-team work takes review priority.** External PRs are reviewed on a best-effort basis and may sit for several weeks.
- **Make sure CI is green.** Please do not re-request reviews on a PR with failing checks.
- **One ping per week, maximum.** A polite check-in after seven or more days of silence is welcome; more frequent nudges are not.
- **Use the PR description to do the work for you.** Link the agreed issue, summarize the approach, include screenshots or benchmarks where relevant. The more self-explanatory the PR, the faster it can be reviewed.

## Reward program

For merged PRs that close an issue labeled `help wanted`, we offer **100 DAI** as a thank-you. The `help wanted` label is applied manually by maintainers and is the only label tied to a bounty. PRs against `accepting contributions` or `good first issue` are not bountied.

To claim, leave a Gnosis Chain address in the PR description or DM `mastercow.eth` on Discord.

## What gets a PR closed without detailed review

So that nothing here is a surprise, the following result in a polite close with a link back to this guide:

- The PR targets a `core team only`, `needs discussion`, or unlabeled issue.
- The PR has no associated issue at all, beyond trivial typo or documentation fixes.
- The PR is clearly outside the [size and scope](https://www.notion.so/cownation/New-Contribution-Guidelines-3598da5f04ca80c6a9aef563a4865ac4#pr-size-and-scope) guidelines and the design wasn't agreed in the issue first.
- The PR mixes unrelated changes (for example, a feature alongside an unrelated refactor in the same diff).

In every case, you're welcome to revise the approach in the issue and try again.
