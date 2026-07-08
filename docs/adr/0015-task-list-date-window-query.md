# ADR-0015: Task-list date-window filtering — additive UTC epoch-second bounds; TUI owns civil-day math

**Status:** Accepted · 2026-07-08

## Context

Board item [0023][feat-0023] adds two TUI capabilities to the Tasks pane:

1. **Hide tasks older than X days** — X is a client-only, non-persistent knob (default `3`,
   hotkey `F`), so the list defaults to a *last-X-days* window (`[today − X, today]`, a span of
   `X + 1` civil days).
2. **Filter tasks by a selected day D** — hotkey `f`, a `DD/MM/YYYY` dialog; selecting D
   **re-anchors** the same window to `[D − X, D]` (operator decision, 2026-07-08).

Both features need the server to return tasks inside a **contiguous UTC-civil-day window**, not
just "the first 200 tasks overall." The operator was explicit: *"we need a date param to
`ListTasks`, otherwise if I have more than 200 tasks in the database I will not be able to fetch
older tasks."* A purely client-side filter over the existing 200-cap ([ADR-0014][adr-0014]) cannot
reach older tasks once the profile holds more than 200 — so the window **must** be expressed on
the wire and applied by the server. Per hard-constraint **#2** (a wire-shape change is an ADR
event) and [ADR-0005][adr-0005] §8 (`contract` is the compatibility authority, additive-only, no
URI versioning), the request shape is settled here **before any code**.

**Date basis — UTC civil day is retained (resolves the [idea-0009][idea-0009] fork).** 0020's
today/older split and date header already compute a **UTC civil day** (`epoch.div_euclid(86400)`,
`crates/tui/src/app/task_list.rs`), chrono-free and deterministic under test. Idea 0009 flagged the
gap between that and the docs' "local date" wording and deferred the choice to the human. For 0023
the operator chose **keep UTC civil-day** (2026-07-08): no timezone dependency is added, so no
hard-constraint **#6** capability event and no ADR to sanction a crate. The residual cost is the
same cosmetic day-boundary edge idea 0009 described (a task created late in local evening groups
by UTC day); it is accepted at personal single-user scale. The companion doc reconciliation idea
0009 called for — ADR-0014 §5 / the 0020 plan text saying "local date" — is folded into this
cycle's docs work; idea 0009 is thereby resolved (keep-UTC) and may be closed by the human.

## Decision

Add **two optional, additive** fields to `contract::TaskListQuery`, alongside the existing
`limit`/`offset`:

```rust
pub struct TaskListQuery {
    pub limit: Option<u32>,   // unchanged (ADR-0014)
    pub offset: Option<u32>,  // unchanged (ADR-0014)
    /// Inclusive lower bound on `created_at`, UTC epoch **seconds**. Absent → no lower bound.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_from: Option<i64>,
    /// Exclusive upper bound on `created_at`, UTC epoch **seconds**. Absent → no upper bound.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_until: Option<i64>,
}
```

1. **Epoch-second bounds, not "days".** The wire carries raw UTC epoch seconds; the server
   applies a plain `timestamptz` range filter — `created_at >= to_timestamp(created_from)` and
   `created_at < to_timestamp(created_until)` — with **no civil-day arithmetic server-side**. The
   server stays a dumb range filter; **the TUI owns the civil-day math** (consistent with idea
   0009's "UTC-in-the-TUI" posture). Day granularity is a *TUI convention*: it sends
   **day-aligned** boundaries — `created_from = (anchor − X) · 86400`, `created_until =
   (anchor + 1) · 86400` — where `anchor` is the selected day D (else today) as a civil
   day-number. The exclusive upper bound at `(anchor + 1) · 86400` makes the anchor day fully
   inclusive.

2. **Bounds are independent and optional.** Either may be present without the other;
   `skip_serializing_if` omits an absent bound, so `TaskListQuery::default()` still serializes to
   an **empty** query string and **absent-both is byte-identical to pre-0023 behaviour** (the
   whole list within `limit`). This preserves ADR-0014's additive guarantee and the bare-`[Task]`
   array response ([ADR-0005][adr-0005] §5) unchanged.

3. **Validation.** If **both** bounds are present and `created_from > created_until`, the server
   returns `400` with the standard `{code: "validation_failed", message}` body (an inverted,
   necessarily-empty window is a client bug, not a valid "return nothing"). `created_from ==
   created_until` is a *valid* empty window (upper is exclusive) and returns `200 []`. Ordering
   (`created_at DESC`), profile-scoping (#4), and the `limit` clamp/`offset` semantics of
   ADR-0014 are unchanged and compose with the new filter.

4. **The 200-cap still applies within the window.** The TUI continues to hard-code `limit = 200`
   (ADR-0014); the date window changes *which* tasks are eligible, not the cap. If a single
   `[D − X, D]` window holds more than 200 tasks the same future-pagination path (start sending
   `offset`) applies with no further wire break. This is the operator's requirement satisfied:
   older tasks become reachable by *moving the window*, not by lifting the cap.

## Consequences

- **No domain-structure change (#3), no new persistence (#1).** No field is added to `Task`, no
  tag/category/timer knob. `hide_window_days` (default 3) and the selected `filter_date` are
  **ephemeral in-session TUI view-state** — the same class as the existing `hide_older` toggle and
  the timer-duration edit buffer — reset on restart, never written to disk. The TUI stays
  stateless per #1 (every view still derives from a server response; the window is just query
  input).
- **Default fetch is now date-windowed.** With default `X = 3` and `anchor = today`, the TUI's
  default `ListTasks` query carries `created_from = (today − 3) · 86400` (and a `created_until`
  in the future that is effectively a no-op while the anchor is today). This is the intended
  behaviour change: by default only the last 3 days show. Absent-params behaviour on the wire is
  unchanged; the *TUI* simply chooses to always send the lower bound.
- **Idea 0010 (empty-string query param → plain-text 400) is not re-opened.** The shipped reqwest
  client always sends real integers for these params; the malformed-empty-value path remains a
  pre-existing, out-of-scope axum-extractor gap tracked by idea 0010.
- **`.sqlx/` offline cache must be refreshed** for the changed task-list query (server slice).

## Alternatives considered

- **Civil day-number bounds on the wire** (send `from_day`/`to_day` as day-numbers, server
  multiplies by 86400). Rejected: it pushes the civil-day convention onto the server for no
  benefit; epoch-second bounds keep the server a pure timestamp-range filter and localise the
  day math in the one place (the TUI) that already does it.
- **Client-side-only filtering over the 200-cap.** Rejected by the operator: cannot reach older
  tasks once a profile exceeds 200 tasks — the whole motivation for a wire param.
- **Local-date basis via a timezone crate / server-provided local-day boundary.** Deferred: both
  are #6 capability / extra-plumbing events; the operator chose to keep UTC (idea 0009).

[feat-0023]: ../../board/features/0023-tui-task-date-window-and-filter.md
[idea-0009]: ../../board/ideas/0009-local-date-today-grouping.md
[adr-0005]: ./0005-foundational-wire-contract.md
[adr-0014]: ./0014-task-list-pagination-ready-limit.md
