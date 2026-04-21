# Review Context — OpenAPI Spec Changes

Loaded by `COW_PR_REVIEW_SKILL.md §3` when a PR touches any `**/openapi.yml`.

This file extends — does not replace — the reminder in `.github/nitpicks.yml` about breaking-change communication.

## Classify the change first

Before reviewing, bucket the diff into:

- **Breaking** — any of:
  - Removing an endpoint, path, or response field.
  - Renaming a field, path parameter, or query parameter.
  - Changing a field's type (string → number, array → object, enum widening into a non-enum, etc.).
  - Making a previously optional field required.
  - **Narrowing** an enum's value set (removing a value). Adding enum values is non-breaking in most clients but may still surprise strict parsers — see below.
  - Changing auth requirements on an endpoint.
  - Changing an endpoint's HTTP method or path.
- **Non-breaking but noteworthy** — new required field on a request body; new endpoint; changed default; stricter validation; added enum value.
- **Pure additive** — new response field (old clients ignore it), new optional request field with a server-side default.

The bucket drives the severity ceiling.

## Breaking changes

If the PR contains any breaking change:

1. **High** — unless the PR description explicitly:
   - Calls out the break.
   - Names the affected consumers (at minimum: **Frontend team** and **SAFE team**).
   - Describes the versioning / migration path (rollout sequence, deprecation window, any shim endpoints).

2. Always include a `Question:` asking whether the consumers have been notified. Even if the description says they have, a reviewer-visible "yes, confirmed" is worth forcing.

3. If the break is irreversible (e.g. a response field's semantic meaning changes while the name stays the same), escalate to **High** even if communication looks clean — semantic breaks are the ones that silently corrupt client behavior.

## Non-breaking but worth flagging

- **New required field on a request body** → check whether a server-side default exists. If not, existing clients will 400 — effectively breaking. **High**.
- **New endpoint without a mention in the PR description** → **Medium**. Clients discover endpoints via the spec; teammates should know a new one landed.
- **Missing `description:` on a new field or endpoint** → **Small**.
- **New enum value** on a field clients parse strictly → **Medium** with a `Question:` — many OpenAPI-generated clients fail hard on unknown enum values; worth checking whether the Frontend / SAFE generators are lenient.

## Not findings

- YAML style, key ordering, indentation.
- Whether the change could have been split into multiple PRs — only worth flagging if the change genuinely mixes unrelated concerns.
- Minor description rewording that doesn't change the contract.

## Communication checklist (use in Action:)

When the PR warrants a Changes-requested verdict for missing communication, the `Action:` can reference this list verbatim:

- [ ] Breaking changes called out in the PR description.
- [ ] Frontend team notified (link the Slack thread or issue).
- [ ] SAFE team notified (link the Slack thread or issue).
- [ ] Versioning/migration path documented — what rolls out when, what deprecates.
- [ ] Any shim or compatibility endpoint planned; if yes, link its PR or issue.
