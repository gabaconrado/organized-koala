---
id: 0026
title: Map axum Query-extractor rejections to the {code,message} JSON error contract
type: feature       # feature | chore
status: inbox           # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: low       # high | medium | low
parent: null
depends-on: []
branch: null
worktree: null
created: 2026-07-15
updated: 2026-07-15
---

## Feature request

**Goal:** Malformed query-parameter values must return the standard
`{ "code": <app-error-code>, "message": <string> }` JSON error body like the rest of the API,
instead of axum's built-in plain-text `Query`-extractor rejection.

**Why:** Promoted from idea [`board/ideas/0010-empty-string-query-param-error-contract.md`][idea-0010]
(surfaced by the verifier during 0020, operator-accepted 2026-07-15). On the task-list endpoint
added in 0020, an **empty-string** query value — `GET …/tasks?limit=` or `?limit=&offset=` —
returns `400` with a **plain-text body** (`Failed to deserialize query string: limit: cannot
parse integer from empty string`), which is axum's default `Query` rejection. That plain-text
response **bypasses the standard `{code,message}` JSON error contract** the rest of the API
honours, so a TUI cannot match it on `code`. This is distinct from the two cases 0020 already
handles to contract: *absent* params (→ `200`, whole list) and an over-ceiling value
(`?limit=501` → `400` JSON `{"code":"validation_failed", …}`).

Impact in practice is negligible — the **shipped reqwest client never emits empty-string params**
(it sends real integers, `limit=200` / `offset=0`), so no real client reaches this path; it is
only hit by a hand-crafted URL. But it is a consistency gap in the error contract worth closing,
and it touches how the server maps extractor rejections across *all* endpoints, not just this one.

**Acceptance criteria:**

- [ ] A malformed query-parameter value (e.g. `?limit=`, `?limit=abc`, `?offset=`) returns `400`
      with the standard `{ "code": "validation_failed", "message": <string> }` JSON body — not a
      plain-text axum rejection.
- [ ] The mapping is applied consistently (a custom `Query` rejection handler / wrapper extractor)
      so malformed query params on any endpoint using it return the contract shape, not only the
      task-list endpoint.
- [ ] The existing behaviours 0020 established are preserved unchanged: absent params → `200`
      whole list; over-ceiling `?limit=501` → `400` JSON `{"code":"validation_failed", …}`.
- [ ] Tests pin the JSON error body for a malformed query param (server-side).
- [ ] No `contract`/DTO change — the error *shape* is already the contract (#2 holds). The error
      *code* reused is the existing `validation_failed`; if a new app-error-code is warranted the
      architect records it. Confirm at plan time whether any ADR is required (expected: none).

**Notes:** The idea's non-binding sketch: a custom `Query` rejection handler (or wrapper
extractor) that maps axum's `QueryRejection` into the standard `{code,message}` JSON body. The
`architect` writes the plan; a `server-dev` slice + a `tester` slice pin the JSON body.

[idea-0010]: ../ideas/0010-empty-string-query-param-error-contract.md
