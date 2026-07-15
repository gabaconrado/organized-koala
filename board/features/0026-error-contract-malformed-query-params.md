---
id: 0026
title: Map axum Query-extractor rejections to the {code,message} JSON error contract
type: feature       # feature | chore
status: review          # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: low       # high | medium | low
parent: null
depends-on: []
branch: feature/0026-error-contract-query-params
worktree: .claude/worktrees/0026-error-contract-query-params
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

## Plan(s)

### Plan: Custom `Query` wrapper extractor mapping `QueryRejection` → `{code,message}` JSON

**Approach:** Introduce a thin server-only wrapper extractor `ValidatedQuery<T>` that delegates
to axum's built-in `Query::<T>::from_request_parts` and maps its `QueryRejection` into the
existing boundary error `ApiError::Validation(..)` (→ `400` + `ErrorCode::ValidationFailed` +
JSON `ErrorBody`). This reuses the exact seam the whole API already flows errors through — the
same pattern as the existing custom `AuthUser` extractor (`FromRequestParts` with
`type Rejection = ApiError`, `crates/server/src/auth/session.rs`). The tracer bullet is the
single call-site swap on `list_tasks` (`Query(query): Query<TaskListQuery>` →
`ValidatedQuery(query): ValidatedQuery<TaskListQuery>`); because the wrapper is generic over `T`
and the state `S`, it is the reusable primitive for *any* future query-param endpoint, satisfying
the "applied consistently" criterion without touching the one place that currently uses `Query`
more than once (there is only one: `list_tasks`). The handler's own over-ceiling / inverted-window
validation is untouched (those values parse fine and are rejected inside the handler, already to
contract), so the two 0020 behaviours are preserved by construction.

**ADR:** none required — see "ADR decision" below.

**ADR decision (explicit): NO ADR required.** This change shapes no contract and makes no new
decision; it *enforces* the already-accepted error contract of [ADR-0005][adr-0005] (foundational
wire contract, incl. error codes) on a path that currently bypasses it.

- **#2 (contract is the single source of truth):** holds untouched. The error *shape*
  (`contract::ErrorBody` = `{ code?, message }`) is already the contract, and the *code* reused is
  the existing `contract::ErrorCode::ValidationFailed` (`"validation_failed"`). **No `contract`/DTO
  change** — no new type, no new field, no new `ErrorCode` variant, no wire-shape change. (Had a new
  app-error-code or DTO been warranted, that *would* make this ADR-worthy under #2 — it is not.)
- **#3 (flat domain):** unaffected — no domain structure added.
- This is a conformance bug-fix: ADR-0005 already mandates every error response be status +
  `{code,message}`; the axum default `Query` rejection is the lone violator. Bringing it into line
  is executing an existing decision, not making a new one.

**Slices:**

1. **[server-dev]** Add the wrapper extractor + swap the call site.
   - **New file** `crates/server/src/extract.rs` (or `crates/server/src/extract/mod.rs` if a
     `tests.rs` sibling is wanted per `rust-standards`): a public `ValidatedQuery<T>(pub T)` newtype
     implementing `FromRequestParts<S>` for `S: Send + Sync`, `T: DeserializeOwned`. Its body calls
     `axum::extract::Query::<T>::from_request_parts(parts, state).await`, maps `Ok(Query(v))` →
     `Ok(ValidatedQuery(v))`, and `Err(rejection)` → `Err(ApiError::Validation(rejection.body_text()))`.
     `rejection.body_text()` carries the client-safe deserialization detail (e.g. "…cannot parse
     integer from empty string") — the client's own malformed input, consistent with the existing
     validation messages the API returns; it leaks no server internals. Type derives `Debug` per the
     `missing_debug_implementations` deny; item is documented per `missing_docs`.
   - **`crates/server/src/lib.rs`**: add `pub mod extract;` (alongside `error`, `handlers`).
   - **`crates/server/src/handlers/tasks.rs`**: change the `list_tasks` signature from
     `Query(query): Query<TaskListQuery>` to `ValidatedQuery(query): ValidatedQuery<TaskListQuery>`
     (import from `crate::extract`; drop the now-unused `axum::extract::Query` import if nothing else
     uses it in that file — `State`/`Path` stay).
   - Gate: `./ok.sh build` + `./ok.sh lint` + `./ok.sh fmt --check` green.
2. **[tester]** Pin the JSON error body for malformed query params (server integration tests).
   - **`crates/server/tests/tasks.rs`**: add tests alongside the existing 0020 block
     (`list_tasks_limit_above_ceiling_is_400_validation_failed` at line ~267 is the model), asserting
     `400` **and** `res.expect_error(ErrorCode::ValidationFailed)` (which parses the JSON `ErrorBody`,
     thereby pinning the contract shape) for: `?limit=` (empty string), `?limit=abc` (non-integer),
     `?offset=` (empty string). Add/confirm a regression assertion that absent-params → `200` whole
     list and `?limit=501` → `400 validation_failed` still hold (both already covered by existing
     tests — reference, do not duplicate unless the assertion is strengthened).
   - Gate: `./ok.sh test` green.

**Ordering & tracer bullet:** slice 1 is itself the end-to-end tracer (extractor → boundary error →
JSON body on a real route); slice 2 pins it. Slice 2 depends on slice 1's call-site swap. No
`contract` or `tui` slice — the wire shape and the reqwest client are unchanged (the client never
emits malformed params, so no TUI behaviour changes; the verifier's live pass exercises the server
API + reqwest path per DoD clause 4, confirming the malformed-param branch now returns JSON).

**Assumptions:**

1. **Message source = `QueryRejection::body_text()`.** The ambiguity policy's smallest change: reuse
   axum's own descriptive text as the `message` rather than inventing a fixed string, so the caller
   still gets the helpful parse detail while now inside the contract envelope. It is client-safe
   (echoes the caller's own bad input; exposes no server internals). If a reviewer prefers a fixed
   generic message (e.g. "invalid query parameters"), that is a one-line change with no shape impact.
2. **Wrapper name `ValidatedQuery` and location `crates/server/src/extract`.** A new server-only
   module keeps the extractor out of `handlers/` and mirrors the existing custom-extractor pattern
   (`auth/session.rs`). Naming/placement is server-dev's call; no contract impact.
3. **Scope = the one current `Query` consumer (`list_tasks`).** A repo scan finds `axum::extract::Query`
   used only in `handlers/tasks.rs`. The wrapper is the reusable primitive so *future* query endpoints
   inherit the contract mapping; this slice swaps only the existing consumer (no speculative changes).
4. **`validation_failed` / HTTP 400 is the right classification** for a malformed query
   param — it is a client input error, matching the code the handler already returns for
   over-ceiling `limit`. No new `ErrorCode` is introduced (keeps #2 intact and this ADR-free).

**Risks:**

- **Low blast radius.** One new small module + one call-site swap + tests. No `contract`, no schema,
  no `.sqlx/` change, no `tui` change.
- **Behavioural surface is observable** (plain-text `400` → JSON `400`), so this is correctly a
  `feature` (not a chore): DoD clause 4 (live verifier boot exercising the server API + reqwest path)
  applies and must confirm the malformed-param branch now returns the JSON `{code,message}` body.
- **Regression guard:** the two preserved 0020 behaviours (absent → `200`; over-ceiling → `400`) run
  through the *handler*, not the extractor, so they are unaffected by construction — but slice 2 keeps
  them pinned so a future refactor cannot silently regress them.
- **Extractor generality gotcha:** `ValidatedQuery` must be generic over the state `S: Send + Sync`
  (axum's `Query` requires it) rather than hard-bound to `AppState`, so it composes on any router;
  binding it to `AppState` unnecessarily would be a smell. Server-dev should keep it state-agnostic.

[adr-0005]: ../../docs/adr/0005-foundational-wire-contract.md

## Log / comments

- [x] 2026-07-15 [architect] Planned. No ADR required — this enforces the existing ADR-0005
  error contract on the one path (axum's default `Query` rejection) that bypasses it; no
  `contract`/DTO change, reuses `ErrorCode::ValidationFailed`. Slices: server-dev (wrapper
  extractor `ValidatedQuery<T>` + call-site swap on `list_tasks`) → tester (JSON-body pins for
  `?limit=`/`?limit=abc`/`?offset=`, regression-guard the two preserved 0020 behaviours). Item →
  `ready`, committed on `main` @392ff14.
- [x] 2026-07-15 [orchestrator] Claimed `ready`→`working`. Worktree cut from `main` @392ff14
  (carries the plan). Branch `feature/0026-error-contract-query-params`. Session: drive cycle
  0026. Build dispatch: server-dev (extractor + swap) → tester (JSON-body pins); no
  contract-owner, no tui slice.
- [x] 2026-07-15 [server-dev] Slice 1 done @28a4afd. New `crates/server/src/extract.rs`:
  `ValidatedQuery<T>(pub T)` newtype (`FromRequestParts<S>` for `S: Send + Sync`, `T:
  DeserializeOwned`) delegating to axum's `Query` and mapping `QueryRejection` →
  `ApiError::Validation(rejection.body_text())` (→ `400` + `ErrorCode::ValidationFailed` + JSON
  `ErrorBody`). `pub mod extract;` in `lib.rs`; `list_tasks` swapped to `ValidatedQuery<...>`
  (`Query` import dropped). Extractor state-agnostic (reusable primitive); over-ceiling/inverted-
  window handler validation untouched. No `contract`/`.sqlx`/schema/`tui` change. Gates green:
  build, lint, fmt --check.
- [x] 2026-07-15 [tester] Slice 2 done @8f311f6. Added to `crates/server/tests/tasks.rs`:
  `list_tasks_empty_limit_is_400_validation_failed` (`?limit=`),
  `list_tasks_non_integer_limit_is_400_validation_failed` (`?limit=abc`),
  `list_tasks_empty_offset_is_400_validation_failed` (`?offset=`). Each asserts `400` +
  `expect_error(ErrorCode::ValidationFailed)` (parses JSON `ErrorBody`, pins contract shape +
  non-empty message; axum wording not pinned). The two preserved 0020 behaviours referenced (not
  duplicated): absent→200 (`..default_no_params..`), over-ceiling→400
  (`..limit_above_ceiling..`). Gates green: test, lint, fmt --check.
- [x] 2026-07-15 [orchestrator] Build complete; both slices green. Status `working`→`review`.
  Dispatching cold reviewer.
