# Handoff — engineering journal

Reverse-chronological. `eng-manager` appends one entry per completed cycle at the **top** and
keeps the "What works right now" snapshot at the bottom current.

---

## Handoff — 2026-06-27 (0015 footer-fix re-entry — single-row flush footer; `feature`)

Operator feedback re-opened 0015 from `awaiting-merge` back to `working` to fold in a
**footer-margin fix**: the trimmed single-line caption sat too high — two blank rows of bottom
margin in the terminal (operator wanted zero). Root cause: 0015 trimmed the footer caption to a
single non-wrapping line but left `BOTTOM_BAND_ROWS = 3` (sized for the OLD wrapping captions per
ADR-0006 §8.3 / learned 0010), so the top-aligned single-line caption left two dead rows. A 0015
loose end — the trim created it — so the operator folded it back into 0015 rather than minting a
new item; per verdict-pinning the prior approved + verified verdicts were **void** once code
changed and the item re-ran review + verify.

What changed:

- **ADR-0006 §8.3 amended** (on `main`, commit `93a503b`): the footer is now a **single flush
  row**; the textual `(Esc to cancel)` affordance is **relocated to the `?` help modal** (the
  keymap is unchanged — `Esc` still cancels an in-flight/loading request; only the on-screen
  textual hint moved).
- **tui-dev** shrank `BOTTOM_BAND_ROWS 3 → 1` and dropped the textual `(Esc to cancel)` from
  `caption_with_spinner` (the in-flight spinner glyph still appends to the stable caption). The
  pending caption is now 60 cols (was 76) so it does not wrap; on a rare wide-timer state the
  trailing spinner glyph may clip at the row edge — accepted per the single-row decision. Pure
  `tui::ui` presentation; #1/#2/#3 untouched. File `crates/tui/src/ui/mod.rs`.
- **tester** realigned the five in-flight asserts that pinned the old `(Esc to cancel)` footer
  (`rendering.rs` `auth_/task_list_/offline_retry_in_flight_…`, `tasks.rs`
  `delete_in_flight_renders_spinner_and_keeps_caption`, `timer.rs`
  `in_flight_appends_a_spinner_without_replacing_the_caption`) — each now asserts the in-flight
  render appends the spinner glyph and keeps the base caption with `"Esc to cancel"` NOT in the
  footer — and added two positive pins: `navigation.rs`
  `footer_is_a_single_flush_row_with_no_blank_trailing_rows` (caption AND timer on the terminal's
  last row, last row non-empty — the operator's zero-bottom-margin ask) and `dialogs.rs`
  `help_modal_documents_that_esc_cancels_an_in_flight_request` (the affordance's new home).
- **One cold-review nit fixed.** The re-review (`changes-requested` at code-hash `542f19aa…`)
  caught a stale `FOOTER_CAPTION` doc comment still describing the removed `(Esc to cancel)`
  affordance and the old multi-row band; tui-dev rewrote it (comment-only, value unchanged),
  moving the code-hash to `b4bc0cdb93086adb620ffbe66bc5d66a524e4ffd`.

Final state: reviewer **approved** + verifier **VERIFIED**, both pinned to code-hash
`b4bc0cdb93086adb620ffbe66bc5d66a524e4ffd`. Gates green (`./ok.sh test | lint | fmt --check`).
The re-verify booted the stack (`./ok.sh up`, migrate exit 0 — no cross-worktree conflict) and
confirmed the reqwest/API paths are byte-identical to the earlier VERIFIED tree (the reopened diff
is pure `tui` presentation). Back at the AI-terminal `awaiting-merge` on the branch.

coverage: **73.80%** line (re-captured via `./ok.sh coverage` in the worktree after the re-entry;
docker + throwaway test Postgres booted cleanly. The footer fix realigned five asserts and added
two pins but the headline `TOTAL` line-coverage is unchanged at 73.80%). Report-only — never a
gate.

**Process note — the test layer caught the operator's intent precisely.** The fix is small but
the failure mode (dead margin rows) is exactly the kind of layout regression that is invisible to
unit logic and only visible in a rendered buffer; the new `navigation.rs` single-flush-row pin
asserts the terminal's last row is non-empty and carries both caption and timer, so a future
band-sizing change cannot silently re-introduce the margin.

**Follow-up / idea filed this cycle:** `board/ideas/0002-serialize-db-backed-integration-tests.md`
(`status: open`, source 0015, raised-by reviewer). The reviewer found — out of scope of 0015, not
blocking — that the default parallel `./ok.sh test` is **flaky under throwaway-Postgres
connection-pool contention**: intermittent `register → HTTP 500 {"code":"internal"}` in the server
DB-backed suites (auth/notes/profiles) that vanishes when DB tests are serialized
(`RUST_TEST_THREADS=1`). A `platform-dev` infra concern (serialize DB-backed integration tests,
bound the test pool, or raise the test Postgres `max_connections`); parked as an idea for human
triage, not minted. It interacts with the open idea 0001 (per-worktree compose isolation) — both
touch how `ok.sh` boots the test Postgres.

**No new gotcha, no agent/skill/standards change warranted.** The "trim a caption but leave the
band sized for the old wrap" loose end is captured in the Log + the ADR-0006 §8.3 amendment; it is
0015-specific, not a recurring cross-cutting gotcha. The flaky-test observation is a `platform-dev`
infra idea, not a standards rule.

---

## Handoff — 2026-06-26 (0015 — TUI dialog system: modals, trimmed footer, purple focus; `feature`)

Phase 2 of the three-part TUI overhaul (0014 → **0015** → 0016). A **`tui`-crate-only** dialog
system — a reusable centred-modal framework, every add/delete/timer-config sub-flow moved off the
inline message band and into dialogs, a `?` help modal with a trimmed footer caption, and a purple
focus border — with **no `contract`/server/domain change** (the presentation-only boundary binds per
[ADR-0010][adr-0010-0015] §5; reviewer + verifier both confirmed
`contract`/`server`/`migrations`/`deploy`/`ok.sh` byte-identical to `main`). Branch
`feature/0015-tui-dialog-system`; reviewer **approved** + verifier **VERIFIED**, both pinned to
code-hash `b9884943f36f3ac6c9d56fd2be46e31057a9060a`. Stopped at the AI-terminal `awaiting-merge`
on the branch.

What shipped (on the branch, `crates/tui/` only — **no** wire surface touched):

- **Reusable dialog framework.** A deep, narrow `draw_dialog` helper in `ui/mod.rs` (one `Dialog`
  struct fed by all six dialog kinds + the help overlay), drawn after the panes via `Clear` +
  `centered_rect` so it floats centred over the tabbed view, carrying a title, fields and/or a
  confirm/cancel prompt, and an optional inline error line.
- **`?` help modal + trimmed footer.** A transient `App.help_open` flag toggled by
  `Event::ToggleHelp` (`?`) renders a centred help modal with the full hotkey reference; the three
  long `*_CAPTION` constants collapse into one short `FOOTER_CAPTION` (movement, tab switch, `?`,
  `q`) plus the unchanged in-flight spinner + "(Esc to cancel)" affordance.
- **Add / delete / timer dialogs.** Task add/edit, note add/edit, profile add/rename, the three
  delete-confirmations, and the timer duration edit all moved out of the 2-row message band into
  dialogs; the message band now carries only the pane's transient status/error (the `last_profile`
  refusal preserved). State machines + submit/cancel + chained-refresh + error routing are
  untouched — only the render site moved (ADR-0010 §5 presentation-only).
- **Purple focus border.** `draw_field` renders a focused field's border in `Color::Magenta`
  (replacing `Modifier::BOLD`), on the auth form fields + all dialog fields; non-focused fields keep
  the plain border.
- **Unified suppression rule + two-tiered Esc.** A single `App::overlay_capturing_input()` predicate
  replaces the scattered `adding.is_some()`/`in_sub_flow()`/`editing_duration` gates: while any
  overlay captures input the globals (`q`/`r`/`?`/`p`/`d`/tab-switch) are suppressed and `Esc`
  cancels the overlay; `Esc` with no overlay on a post-auth screen still quits, and in-flight
  `Esc`-cancel is preserved.

**Process-relevant event — the test layer caught a UX/keymap inconsistency before review.** Tester's
slice 5 flagged (as a finding, not worked around — no src edit) that `draw_help`'s footer advertised
`?/Esc: close`, yet a live `?` keypress was suppressed by the open help overlay at the keymap, so
only `Esc` actually closed help. tui-dev corrected it **in-cycle** (fix-now) rather
than deferring: a distinct `help_open` param threaded into `map_key` (now 5-arg
`(screen, overlay_capturing, help_open, editing_duration, key)`) special-cases `?` to **toggle** —
opening from an idle post-auth screen and closing while the help overlay is active (the core already
folds `Event::ToggleHelp` into a close); `?` stays suppressed while a *non-help* dialog captures input
(A3). A clean example of the `TestBackend` layer (ADR-0003 layer 2) catching a keymap/affordance
mismatch the moment the tests pinned the advertised behaviour, corrected before the cold review rather
than reaching `awaiting-merge` as a latent inconsistency.

Agents: **tui-dev** (slice 1 overlay/suppression seam, slices 2+3 dialog framework + help modal +
footer trim, slice 4 docs, + the `?`-closes-help fix-now) and **tester** (slice 5 `TestBackend`
suite — new `tests/dialogs.rs` + extensions across the existing suites, plus the follow-up updating
the suite for the 5-arg `map_key` and flipped `?`-closes-help behaviour).

Tests: `./ok.sh test` **380 passed / 0 failed**; `./ok.sh lint` clean (`--all-targets`);
`./ok.sh fmt --check` clean. Verifier confirmed `tests/dialogs.rs` 21/0 covering all six acceptance
criteria, all supporting suites 0-fail, and (clause-4 part 2) booted `./ok.sh up` clean to confirm
the reqwest/API paths the dialogs drive are unchanged (no server/contract delta to exercise).

coverage: **73.80%** line (the headline `TOTAL` line-coverage from a fresh `./ok.sh coverage` in the
worktree; docker plus the throwaway test Postgres booted cleanly — no cross-worktree volume conflict).
Report-only — never a gate.

**Cycle ran clean — no new gotcha, no agent/skill/standards change warranted.** The "unify scattered
suppression gates into one predicate but keep a distinct flag for the toggle-able overlay so its own
toggle key isn't swallowed" lesson is genuine but **0015-specific design detail**, already captured
in the plan's slice 1 + the `?`-closes-help fix Log — not a cross-cutting, recurring gotcha, so none
manufactured. The fix-now process note above is recorded here as a journal observation, not a
standards rule.

**Follow-ups / ideas filed this cycle: none.** Reviewer and verifier both reported no out-of-scope
findings; the `?`-closes-help issue was an **in-scope fix-now** (the advertised affordance had to
work to meet acceptance), not a deferred follow-up. The pre-existing per-worktree compose-isolation
idea (`board/ideas/0001`) remains open and untouched. No new idea minted.

**Forward note.** 0016 (`depends-on: [0015]`) is the final phase: per-field task/note detail views +
the complete hotkey remap (`c`→`Space`, `x`→`d`, `p`→`t`, `t` finally bound) — all still under
[ADR-0010][adr-0010-0015], inheriting and citing it (a new shell ADR is only warranted if 0016 needs
a wire/server/domain change, which is not expected). Merge 0015 before claiming 0016.

[adr-0010-0015]: ./adr/0010-tui-navigation-and-interaction-model.md

---

## Handoff — 2026-06-26 (0014 — TUI layout shell: tabs, centred auth/title, tight footer; `feature`)

Phase 1 of the three-part TUI overhaul (0014 → 0015 → 0016). A **`tui`-crate-only** reshape of the
structural shell — navigation model, auth screen, title bar, footer position — with **no
`contract`/server/domain change** (the boundary is binding per [ADR-0010][adr-0010-0014] §5). Branch
`feature/0014-tui-layout-shell`; reviewer **approved** + verifier **verified**, both pinned to
code-hash `bf65aa9612bf1633bf75e64f66a3dfddcfb4aa10` (commit `c8b1217`). Stopped at the AI-terminal
`awaiting-merge` on the branch.

What shipped (on the branch, `tui` crate only — **no** wire surface touched):

- **Tabbed post-auth view.** `Screen::TaskList`/`Notes`/`Profiles` collapsed into one
  `Screen::Main(Box<MainState>)` holding the active `Tab{Tasks,Notes,Profiles}` + all three live
  panes (new `crates/tui/src/app/main_view.rs`). New `Event::NextTab`/`PrevTab`; `map_key` remaps
  `Tab`/`BackTab` to tab-switching on an idle list (cycle both directions), arrows move list
  selection, a tab switch re-derives the active pane from a **fresh server load** for the active
  profile (#1, #4) preserving the selected row. Removed `OpenNotes`/`OpenProfiles`/`Back`, the
  idle-`Esc`-back path, and the `n`/`s` cross-screen bindings; `t` left **deliberately unbound** for
  0016's timer. Pick-active re-homed onto the Profiles tab. Every other binding unchanged — no
  sub-flow/CRUD behaviour change.
- **`Session`/`AuthState` gained `account: String`** — the entered identifier captured **client-side**
  at auth time (no new wire; ADR-0010 §2) so the title renders `<user>`.
- **Presentation.** Centred bounded auth form (toggle + all fields + error band intact); centred
  verbatim title `organized koala - <user> @ [<profile>]` (literal hyphen + brackets); footer flushed
  to the bottom row (outer margin dropped, band kept at 3 rows — caption + spinner + cancel still fit
  at 80×24). Full captions retained (caption trim is 0015).

Tests: new `crates/tui/tests/navigation.rs` (14 tests) covering every 0014 acceptance criterion, plus
the existing `TestBackend` suites re-pointed to the tabbed shell (tab-switch via `NextTab`/`PrevTab`
replaces the removed cross-screen events; pane accessors replace the old destructures), preserving each
test's intent (CRUD reachability, error-code branching, in-flight, JWT redaction). Only mock is the
`Client` trait.

coverage: **72.96%** line (the headline `TOTAL` from a fresh `./ok.sh coverage` in the worktree;
docker plus the throwaway test Postgres booted cleanly). Report-only — never a gate.

**Cycle ran clean** — docker available, no cross-worktree migration-history conflict (0014 adds no
migration), no scope creep into 0015/0016, no review/verify friction. **No new gotcha and no
agent/skill/standards change** is warranted (none manufactured).

**Forward note (load-bearing for the next two cycles).** [ADR-0010][adr-0010-0014] governs the whole
0014–0016 arc: its **presentation-only boundary (§5)** and the **tab/Esc/keymap invariants** bind 0015
and 0016. Those phases **inherit and cite ADR-0010** rather than opening new TUI-shell ADRs — a new
shell ADR is only warranted if a phase needs a wire/server/domain change (none is expected; 0015 is
the dialog system + caption trim + focus styling, 0016 is detail views + the full hotkey remap incl.
`t` for the timer). 0015 `depends-on: [0014]`; 0016 `depends-on: [0015]` — merge 0014 first.

**Follow-ups / ideas filed this cycle:** none. The verifier noted footer-caption wrapping at 80×24,
but caption trimming is **already planned for 0015** (per the 0014 item's out-of-scope list and
ADR-0010) — not a new unplanned follow-up, so no idea filed.

[adr-0010-0014]: ./adr/0010-tui-navigation-and-interaction-model.md

---

## Handoff — 2026-06-26 (0013 — redact the session JWT in the `tui` `Session` Debug leak; high `chore`)

A security `chore`: the `tui` session **bearer JWT was held as a bare `String`** inside structs and
enums that `#[derive(Debug)]` (`Session`, all 17 `ClientRequest::*` variants, `Outcome::ListProfiles`),
so the secret was reachable through any `{:?}` — a log line, a `tracing` span field, a panic message,
or future auto-instrumentation. A direct violation of `rust-standards` → *Sensitive data*. Branch
`feature/0013-session-token-debug-leak`; cold reviewer **approved** (chore invariant attested), the
live verifier pass correctly **skipped** (chore track — no live-observable change). Stopped at the
AI-terminal `awaiting-merge` on the branch.

What shipped (on the branch, `tui` crate only — **no** wire surface touched):

- **`crates/tui/src/app/token.rs`** (new) — a `SessionToken(String)` newtype with a **hand-written
  `Debug` → `[REDACTED]`** (no `Display`/`Serialize`) and `expose(&self) -> &str` for use only at the
  point the bearer string is attached. Mirrors the in-repo `contract::Password` template; doctest
  asserts both `expose()` returns the value AND `format!("{token:?}") == "[REDACTED]"`.
- Bare `token: String` → `SessionToken` across every Debug-reachable holder: `Session.token`
  (`app/mod.rs`), all 17 `ClientRequest::*` `token` fields + `Outcome::ListProfiles.token`
  (`app/protocol.rs`). The worker (`client/worker.rs`) exposes the bearer string only at point of use
  (`token.expose()` → `bearer_auth`); the test worker-analogue executor got the same mechanical
  `&token` → `token.expose()` adaptation (no test-intent change). The `Client` trait's `token: &str`
  params are ephemeral point-of-use borrows (not stored Debug-reachable fields) — left as-is; the wire
  bearer string is byte-identical. No bare `token: String` remains in `crates/tui/src/`.
- **Tests** — `crates/tui/tests/redaction.rs` (public-API only): three tests formatting `{:?}` of
  `Session`, `ClientRequest::ListTasks`, and `Outcome::ListProfiles`, each asserting the token
  substring is **absent** and `[REDACTED]` is **present**, with a non-plausible placeholder token
  (`SECRET.JWT.VALUE`) so the secret scan passes.

**Redaction-shape decision.** Implementer chose the **local redacting newtype over
`secrecy::SecretString`** — the `Password` pattern is already in-repo, the `Client` methods take
`token: &str` so the newtype redacts in one type with no new dependency and no per-call-site
`expose_secret()` churn, keeping trait/wire signatures byte-identical. Trade-off recorded: the
newtype redacts but does **not** zeroize on drop (which `secrecy` would) — acceptable for a
process-lifetime in-memory token under #1.

coverage: **66.90%** line (the headline `TOTAL` from a fresh `./ok.sh coverage` in the worktree;
docker + throwaway test Postgres booted cleanly). Report-only — never a gate.

Verdict (@ code-hash `e5925c5139e52846d8593c4be3ab2d0516d49fa0`, last code sha `e86f956`):

- **reviewer — REVIEW-STATUS: approved.** Mechanical gate green (`fmt --check`/`lint`/`test`); leak
  closure confirmed (`SessionToken` redacts, exposed only at point of use, nothing re-exposes it; all
  17 variants + `Outcome::ListProfiles` + `Session` covered — redaction complete, not merely moved).
  **Chore invariant attested:** no behaviour change (wire bearer string byte-identical), no
  `contract`/wire change #2 (`git diff main..HEAD -- crates/contract/` empty; no `Cargo.toml`/
  `Cargo.lock` change), no domain-structure change #3.

**The load-bearing learning this cycle — a mechanical lint pulls *against* a prose-only secret
rule, and that tension is how the leak survived from 0004 through 0011.** The operator's root-cause
comment nailed it: the `Session` struct was introduced in 0004 (`4b9eda0`) **after** both the
`rust-standards` secret rule and the `contract::Password` redacting template already existed — so it
was a violation of a **pre-existing documented rule**, missed by the 0004 author and cold reviewer,
then carried silently through 0005/0008/0010/0011 because **cold review is diff-scoped** (pre-existing
code is out of each cycle's review scope) until 0012's reviewer flagged it. Two contributing factors:
(1) the secret rule is **prose-only**, with no clippy/lint enforcement for "bare secret reachable from
`Debug`"; (2) `[workspace.lints] rust.missing_debug_implementations = "deny"` actively pushes devs to
add `#[derive(Debug)]` to **every** public type — colliding with the secret rule, and by default the
bare derive wins. Durable fix this cycle: a **`rust-standards` callout** (home #1, on `main`) under
*Sensitive data* making the `missing_debug_implementations`-vs-secret tension explicit and naming the
resolution pattern (a redacting newtype: `contract::Password` / `tui::app::SessionToken`), framed as
a per-secret-field checklist item since cold review can't catch the pre-existing case. **A mechanical
guard remains the real durable fix** — recorded below as a recommended future Board `chore`.

Durable learnings: **one `rust-standards` addition** — the Debug-lint-vs-secret-redaction callout
(above). **No new ADR** (a `tui`-internal representation change, no contract/domain decision), **no
new crate → no new dev agent** (`SessionToken` is a module inside the existing `tui` crate, already
owned by `tui-dev`). **No new CLAUDE.md hard-constraint or gotcha** earned this cycle: the secret-leak
rule already lives in `rust-standards` (its correct home); this cycle sharpens that skill rather than
adding a cross-cutting domain rule.

**Recommended future Board item (mintable `chore`, low priority) — a mechanical secret-in-`Debug`
guard.** The prose callout is the safety net, not the fix. The durable fix is a **mechanical check**
that a bare secret cannot be reachable from a derived `Debug` (e.g. a clippy lint / custom static
check / a CI grep-and-fail over `token`/`password`/`secret`-typed bare fields on `#[derive(Debug)]`
types, or a marker-trait convention the lint keys on). This is its own scoped piece of work (likely
`platform-dev` + a tooling decision), **not** in scope for 0013 — flagged here so the orchestrator can
mint it directly. Until it exists, the `rust-standards` callout + the two redacting-newtype templates
are the guard.

**Homes.** Feature-local on the branch (home #2): the item's `## Summary` (with the coverage line),
committed on `feature/0013-session-token-debug-leak` (Board-only, code-hash unchanged → verdict
intact). Cross-cutting/derived on `main` (homes #1/#3): this `docs/handoff.md` entry (+ the "What
works right now" snapshot refreshed for the 0013 state), the `rust-standards` callout, the brought-
current `docs/build-plan.md`, and the regenerated `board/README.md`. **`main`'s frozen copy of
`board/features/0013-session-token-debug-leak.md` stays untouched** at the claim snapshot (`ready`)
until the human's merge. The orchestrator flips the branch status to `awaiting-merge` after this step.

---

## Handoff — 2026-06-25 (0012 — Profiles create/update/delete + TUI switcher; the final domain feature)

The **last domain feature** shipped: full profile management. Today's only profile surface was
`GET /api/profiles` (list) plus the register-time default-profile bootstrap; 0012 adds create,
rename, delete (with cascade) and a TUI profile-picker/switcher, governed by [ADR-0009][adr-0009]
(profile mutations, referencing ADR-0005 §2/§4/§6 — the **two new error codes** are an append-only
ADR event). Branch `feature/0012-profiles-crud-and-switcher` (code commit `e6afefd`, code-hash
`71fb7ecf327fbd42a14cb19456207885c782fe49`); reviewer **approved** + verifier **verified**, both
pinned to that hash. Stopped at the AI-terminal `awaiting-merge` on the branch.

What shipped (on the branch, contract → server → tui):

- **`contract`** (`7d0979a`) — `CreateProfileRequest { name }`, `UpdateProfileRequest { name }`,
  and two **append-only** error codes `ErrorCode::ProfileNameTaken` / `ErrorCode::LastProfile`
  (`Unknown` forward-compat fallback intact). No DTO redefinition (#2).
- **`server`** (`9960653`) — `POST /api/profiles` (201), `PATCH /api/profiles/{id}` (200),
  `DELETE /api/profiles/{id}` (204), owner-scoped. Race-safe DB unique-violation →
  `409 profile_name_taken` (no TOCTOU); atomic single-statement last-profile guard →
  `409 last_profile`; delete **cascades** tasks **and** notes via FK `ON DELETE CASCADE` (no app
  fan-out, #4); unowned → `404`, blank name → `400`. Reversible `UNIQUE (user_id, name)` migration
  `20260612163050_profile_name_unique` (ordered after 0010); `.sqlx/` refreshed.
- **`tui`** (`5886060`) — `Client` create/rename/delete profile + `ClientRequest`/`Outcome` worker
  arms; `Screen::Profiles` switcher opened by `s` (Enter = pick-active; `a`/`e`/`x` =
  create/rename/delete). Switch is **client-side only** — rebinds the in-memory
  `active_profile_id`, re-scopes subsequent task/note calls, **no** server endpoint, **no**
  persistence (#1); deleting the active profile re-points to the first remaining;
  `ProfileNameTaken`/`LastProfile` inline.
- **Tests** (`e6afefd`) — contract `profile.rs` 8 / `error.rs` 16; server `profiles.rs` 20 incl.
  the headline cascade test asserting **both** task AND note gone (DB count + 404), cross-account
  same-name allowed, auth; tui `profiles.rs` 16 + `keybindings.rs` 25 (pick-active carries the new
  id with **no** switch call, inline conflict codes, in-flight/stale-drop, active-repoint). All
  gates green at `e6afefd`: `./ok.sh prepare | build | test | lint | fmt --check`.

coverage: **66.91%** line (the headline `TOTAL` from a fresh `./ok.sh coverage` in the worktree;
docker + throwaway test Postgres booted cleanly). Report-only — never a gate.

**The load-bearing learning this cycle — `./ok.sh prepare` was never self-contained, and the
permission guard surfaced the gap.** `cmd_prepare` had been bare since the first scaffold
(`cargo sqlx prepare --workspace`, no DB wiring). Every prior server cycle that refreshed `.sqlx/`
did so via an **ad-hoc out-of-band `DATABASE_URL`** pointed at some live PG — which this session's
permission guard (correctly) denied, exposing that the verb itself had never carried its own DB.
Slice 2 needed a `.sqlx/` refresh for 3 new compile-checked queries; `server-dev` **blocked rather
than improvise** (#6). The operator authorized **Option A**: `platform-dev` made `cmd_prepare`
self-contained on **`main`** (`3e0094b`) — boot the throwaway test PG → apply migrations **via the
sqlx CLI** (deliberately **not** the server binary, which would hit the offline-build circularity
on a feature branch whose `.sqlx/` is mid-refresh) → `cargo sqlx prepare` → teardown — mirroring
`cmd_test` (0003) and `cmd_coverage` (0007). Validated by a zero-`.sqlx`-diff run on `main`. This
**completes the "every DB-needing `ok.sh` verb self-boots the shared test PG" pattern** the 0007
handoff first named in `bash-standards` — `test`, `coverage`, and now `prepare` all use the one
`DATABASE_URL`/compose/`RETURN`-trap wiring. Recorded durably below (`bash-standards`).

Verdicts (both @ code-hash `71fb7ecf327fbd42a14cb19456207885c782fe49`, code commit `e6afefd`):

- **reviewer — REVIEW-STATUS: approved.** Gate clean; no contract drift (#2, append-only); all
  hard constraints hold (#1 client-side in-memory switch, #4 owner-scoped + FK cascade, #3 no
  domain structure, #5 auth unchanged); race-safety correct (DB unique-violation mapped, atomic
  last-profile guard); migration reversible, ordered after 0010; headline cascade test asserts BOTH
  children gone. No fix-now findings.
- **verifier — VERDICT: verified.** `./ok.sh up` booted clean (no cross-worktree migration-history
  conflict; all 6 migrations applied). RAN live against `localhost:8080`: create 201 / trim /
  empty→`400`; duplicate→`409 profile_name_taken` + cross-account same-name 201; rename 200,
  unowned→`404`; **cascade** delete → DB-confirmed `tasks=0, notes=0, profile=0` + HTTP 404 (#4);
  last-profile→`409 last_profile`; no cross-leak; no-token→`401`; bodies standard status +
  `{ code, message }`; OTel handler spans observed. TUI `TestBackend` suite present + green
  (ADR-0003).

Durable learnings: **one `bash-standards` addition** — the `cmd_prepare` pattern-completion (below).
**No new ADR** (ADR-0009 already on `main` with the plan), **no new crate → no new dev agent** (the
profile surface is modules inside the existing crates — `crates/contract/src/profile/`,
`crates/server/src/handlers/profiles.rs`, `crates/tui/src/app/profiles.rs`, each already owned). **No
new CLAUDE.md hard-constraint or gotcha** earned this cycle: the prepare gap is an infra/`ok.sh`
discipline (its home is `bash-standards`, not a cross-cutting domain rule), and the
cross-worktree-volume gotcha did **not** recur (a clean `./ok.sh up`).

**Homes.** Feature-local on the branch (home #2): the item's `## Summary` (with the coverage line),
committed on `feature/0012-profiles-crud-and-switcher` (`3fedcbe`; Board-only, code-hash unchanged).
Cross-cutting/derived on `main` (homes #1/#3): this `docs/handoff.md` entry (+ the "What works right
now" snapshot refreshed for the 0012 state), the `bash-standards` `cmd_prepare` learning, and the
regenerated `board/README.md`. The `cmd_prepare` source change itself already landed on `main`
(`3e0094b`, by `platform-dev`, mid-build). **`main`'s frozen copy of
`board/features/0012-profiles-crud-and-switcher.md` stays untouched** at the claim snapshot
(`ready`) until the human's merge. The orchestrator flips the branch status to `awaiting-merge`
after this step.

**Free pickup noted (mintable `chore`, low priority):** the reviewer flagged a pre-existing,
out-of-scope nit — `Session.token` is a bare `String` and `Session` derives `Debug`, so the raw JWT
is reachable via the derived `Debug` impl (e.g. if a `Session` is ever logged). Predates 0012 and is
unchanged here; not fixed in-cycle because it would change the code-hash and void the approved +
verified verdicts. The orchestrator may mint it as a `type: chore`, `priority: low` item carrying
just a `## Feature request` (wrap `token` in a redacting newtype, or give `Session` a manual `Debug`
that elides the token — mirroring the `contract` `Password` redacting-newtype pattern).

[adr-0009]: ./adr/0009-profile-mutations.md

---

## Handoff — 2026-06-25 (0011 re-cycle — re-rebased onto post-0010 `main`, re-reviewed + re-verified)

The operator merged **0010 (Notes)** to `main`, then **0011 was re-rebased onto post-0010 `main`**
(`5ad5ba9`). Unlike the prior docs-only step-7 freshen, this rebase **changed code**: it pulled the
entire Notes feature into 0011's `crates/` tree, with real conflicts in the TUI (`app/mod.rs`,
`protocol.rs`, `client/mod.rs`, `client/worker.rs`, `terminal/mod.rs`, `ui/mod.rs`) and the test
helpers — exactly the files both features extended. They were resolved as a **union** preserving both
surfaces: 0011's breaking removal (`CloseTask`/`close_task`/`CloseSelected` dropped) plus
`UpdateTask`/`DeleteTask` and the Notes variants all kept; the `map_key` caption merged to carry all
keys with `BOTTOM_BAND_ROWS = 3`.

Because `./ok.sh code-hash` is a whole-`crates/`-tree digest (not per-feature), pulling Notes in
**changed 0011's code-hash** `e66426f0a6fcb9c0ba3f7e6baf1f3b606708a6cf` →
`ee5047c9abf1e4196ed1933655a61fcf41148bcb`, which per the verdict-pinning rule **voided** the prior
`approved`/`verified` verdicts — so 0011 **re-entered review + verify** even though task-mutation and
Notes never touched the same behaviour. Both **re-passed** at the new hash `ee5047c9…`: reviewer
**approved** (cold re-review confirming the union merge preserves both surfaces), verifier
**verified** (live re-boot — the earlier cross-worktree migration-history collision is gone now that
0011's tree legitimately carries the `20260612163049 notes` migration; all 8 task flows ran). Stopped
again at the AI-terminal `awaiting-merge` on the branch, awaiting the human's merge.

coverage: **68.24% line** (62.99% region / 70.77% function), freshly measured on the merged tree —
now **reflects the Notes feature** the re-rebase pulled in (the pre-rebase 0011 snapshot was lower;
this figure matches 0010's because the tree now contains both). Report-only — never a gate.

**Durable learning recorded (the load-bearing one this re-cycle).** When two independent features
both sit at `awaiting-merge` and one merges, rebasing the second onto the new `main` pulls the merged
feature's files into the second's `crates/` tree, **changing its code-hash and voiding its
approved/verified verdicts** — forcing a re-review/re-verify on a feature that changed no behaviour
of its own. Recorded as a new CLAUDE.md gotcha (near the cross-worktree volume gotcha). **Plan for
it:**
merge parallel `awaiting-merge` features in a deliberate order and budget a re-review/re-verify pass
for the trailing one; the conflicts land in the files both features extended (enum variants, trait
methods, worker/dispatch arms, key handling, captions), resolved as a union. **No new ADR** (ADR-0008
already on `main`), **no new crate → no new dev agent**.

**Homes.** Feature-local on the branch (home #2): the refreshed `## Summary` (coverage line +
verdict-hash references updated to `ee5047c9…`), committed on `feature/0011-task-update-delete-reopen`
(`915005c`; code-hash unchanged — Board-only). Cross-cutting/derived on `main` (homes #1/#3): this
`docs/handoff.md` entry (+ the "What works right now" snapshot refreshed) and the new CLAUDE.md
gotcha, plus the regenerated `board/README.md`. **`main`'s frozen copy of
`board/features/0011-task-update-delete-reopen.md` stays untouched** at the claim snapshot until the
human's merge.

---

## Handoff — 2026-06-25 (0011 — task update/delete/reopen; `close` removed, breaking)

The one-way task `close` was generalized into full task **edit / toggle-done / reopen / delete**.
This is a **breaking** contract change ([ADR-0008][adr-0008-0011], referencing ADR-0005 §5/§8): the
`POST .../tasks/{id}/close` route is **removed**, not deprecated. With a single in-repo consumer (the
TUI, migrated in the same item) and ADR-0005 §8 making `contract` the compatibility authority +
forbidding URI versioning, a clean removal is the correct shape — there is no external client to keep
a deprecated route alive for. Branch `feature/0011-task-update-delete-reopen`; reviewer
**approved** + verifier **verified**, both pinned to code-hash
`e66426f0a6fcb9c0ba3f7e6baf1f3b606708a6cf` (last code
sha `6c3b987`). Stopped at the AI-terminal `awaiting-merge` on the branch.

What shipped (on the branch, contract → server → tui):

- **`contract`** — `UpdateTaskRequest { title?, description?, status? }`, an all-optional partial-update
  DTO (`skip_serializing_if = "Option::is_none"`); no `updated_at`, flat (#3).
- **`server`** — `PATCH …/tasks/{task_id}` via a single static parameterized `UPDATE … RETURNING`
  (`COALESCE`/`CASE`): `status: done` sets `closed_at`, `status: open` (reopen) clears it to null,
  absent leaves it untouched, empty patch is a 200 no-op, blank title → 400 `validation_failed`.
  `DELETE …/tasks/{task_id}` → 204, second/missing → 404. The `close_task` handler + `…/close` route
  are gone. Both routes ownership-joined (`WHERE id=$1 AND profile_id=$2`), unowned → 404 never 403
  (#4). **No migration** — the existing `tasks` table already supports the in-place update.
- **`tui`** — task list gains edit (`e`), toggle-done/reopen (`c`), delete (`x`, two-step confirm);
  all mutations chain a `ListTasks` refresh (stateless, #1); `client`/`protocol` `CloseTask` →
  `UpdateTask` plus `DeleteTask`.

coverage: **62.87%** line (the headline `TOTAL` from a fresh `./ok.sh coverage` in the worktree).
Report-only — never a gate.

**Cross-worktree infra gotcha (the load-bearing learning this cycle).** The live verifier's first run
**failed to boot the stack** — and it was **not** a 0011 defect. Concurrent feature worktrees all use
the **same docker compose project name (`deploy`)** and therefore share the **persistent named volume
`deploy_postgres-data`**. That volume still carried 0010's `notes` migration (`20260612163049`), but
0011's migration tree correctly ends at `20260612163048_timer` (0011 needs no schema change). sqlx's
strict migration-history consistency check then refused to proceed — *"migration 20260612163049 was
previously applied but is missing in the resolved migrations"* — and the `run` service, gated on the
one-shot `migrate`, never came up. Per #6 the verifier did **not** work around it (the clean fix,
`docker compose down -v`, would destroy another branch's local data). The operator authorized resetting
the `deploy_postgres-data` volume; the next `./ok.sh up` recreated it clean and the verifier re-ran
green. Recorded as a CLAUDE.md gotcha. **Recommended follow-up (a `platform-dev` concern):** give each
worktree an isolated compose project name / volume (e.g. derive `COMPOSE_PROJECT_NAME` from the
worktree slug) so concurrent branches never share migration history — this removes the failure mode
rather than relying on an operator volume reset.

**Homes.** Feature-local on the branch (home #2): the item's `## Summary` (with the coverage line),
committed on `feature/0011-task-update-delete-reopen`. Cross-cutting/derived on `main` (homes #1/#3):
this `docs/handoff.md` entry (+ the "What works right now" snapshot refreshed), the new CLAUDE.md
gotcha, and the regenerated `board/README.md`. **`main`'s frozen copy of
`board/features/0011-task-update-delete-reopen.md` stays untouched** at the claim snapshot until the
human's merge. The orchestrator flips the branch status to `awaiting-merge` after the step-7 freshen.
**No new ADR** (ADR-0008 already on `main` with the plan), **no new crate → no new dev agent**.

**Free pickup noted (mintable `chore`, low priority):** the reviewer flagged `crates/tui/README.md:15`
still says "list/add/**close** tasks" — stale after the close→update/delete migration (the server
README route table was correctly updated). A doc-only fix; not touched here because it would change
the code-hash and void the approved+verified verdicts. The orchestrator may mint it as a `type: chore`,
`priority: low` item carrying just a `## Feature request` (update the line to reflect edit/toggle/delete).

[adr-0008-0011]: ./adr/0008-task-mutation-generalization.md

---

## Handoff — 2026-06-24 (0010 — Notes, the final domain feature, end-to-end across all three crates)

The **last missing domain feature** shipped: Notes, a near-exact structural clone of the task
surface, governed by [ADR-0007][adr-0007] (notes wire contract — already on `main` with the plan).
Branch: `feature/0010-notes` (code sha `2a4074d` at verification; current branch HEAD after this
eng-manager step). Reviewer **approved** and verifier **verified**, both pinned to code-hash
`46c1c60f1eb3865eb127a72502982827ebb09d65` (re-confirmed equal at this step — verdicts carry
forward, no relabelling). The cycle stopped at the AI-terminal `awaiting-merge` on the branch.

What shipped (on the branch, contract → server → tui per the slice order):

- **`contract`** — a new `note` module: `Note { id, title, content, created_at }`,
  `CreateNoteRequest { title, content }`, `UpdateNoteRequest { title, content }`, reusing the
  `{ code?, message }` error contract with **no** new `ErrorCode`. Flat (#3), no `updated_at`
  (editing mutates in place; only `created_at` is a timestamp, operator-locked).
- **`server`** — five CRUD routes under `/api/profiles/{id}/notes` (create 201 / list 200 bare
  array newest-first / get 200 / update 200 in-place / delete 204), every query ownership-joined
  so an unowned or missing profile/note id is `404 not_found` (never 403, #4 / ADR-0005 §4
  non-observability). Reversible migration `20260612163049_notes` (paired up/down; `ON DELETE
  CASCADE`, `(profile_id, created_at DESC)` index) + a `.sqlx/` refresh.
- **`tui`** — five `Client` trait methods + `HttpClient` impls, `ClientRequest`/`Outcome` variants
  (carrying `token` + `profile_id`) + worker arms, and a `Screen::Notes` view (list +
  create/edit/delete sub-flows) opened by `n` from the task list. Stateless (#1); no `chrono` in
  `tui` (A8 — timestamp formatting at the render seam).
- **`fix(tui)`** — a caption-layout regression the TUI suite surfaced (see learning below): adding
  `n: notes` grew `TASK_LIST_CAPTION` so the pending caption + spinner clipped the cancel
  affordance at 80×24 (ADR-0006 §8.3); the bottom band was widened to 3 rows and both captions
  re-phrased with ` | ` separators, no assertions weakened.

Tests in all three crates: `contract` note DTOs 11 (+ doctests), `server` notes integration 28
(incl. profile-scoping + auth-required per route), `tui` `TestBackend` notes suite 13 (+
rendering 11). All four gates green at branch head (`build | test | lint --all-targets |
fmt --check`).

Verdicts (both @ code-hash `46c1c60f1eb3865eb127a72502982827ebb09d65`):

- **reviewer — REVIEW-STATUS: approved.** Hard constraints clear (#1 stateless, #2 DTOs only in
  `contract`, #3 flat no-`updated_at`, #4 every query ownership-joined → 404 never 403); no new
  `ErrorCode`; migration up/down paired + cascade; the caption `fix(tui)` in-scope (ADR-0006 §8.3).
- **verifier — VERIFY-STATUS: verified.** Booted the real stack (`./ok.sh up`, docker 29.5.3);
  migration applied; flat schema confirmed (`id,profile_id,title,content,created_at`, no
  `updated_at`); the full wire surface exercised live (shapes, status codes, `{code,message}`
  contract, profile-scoping → 404, all five OTel handler spans). One stated inference: the reqwest
  `HttpClient` path verified by structural equivalence (curl drove the wire; the `tui` Client maps
  one-for-one + the 13-test suite drives the trait), not a literal live reqwest harness — not a
  coverage gap.

coverage: **68.24% line** (62.99% region, 70.77% function), the headline `TOTAL` from a fresh
`./ok.sh coverage` run in the worktree (docker + throwaway test Postgres booted cleanly — nothing
acquired, #6 intact). Up from the 0009 snapshot (66.36% line) — the notes server handlers and TUI
view land well-tested (`handlers/notes.rs` 100% line, `app/notes.rs` 90%). **Report-only — no
threshold, never a gate.**

Durable learning recorded (one, in the `tui-dev` agent): **caption width and bottom-band height
are coupled at the 80×24 test viewport.** This bit on 0008-R1 (the append-spinner work) and **again
on 0010** (adding the `n: notes` hotkey), so it earned a durable agent note rather than staying a
per-cycle surprise: growing a fixed-width caption can wrap the stable caption + appended spinner +
cancel affordance an extra line and clip it — a render regression the `TestBackend` suite catches,
not the compiler. The fix is always to budget the band row count (and pick ` | ` wrap points) in
the *same* change that grows the caption; the invariant is owned by ADR-0006 §8.3, and the
rendering code already carries inline comments naming the 80×24 boundary. No CLAUDE.md hard
constraint earned (this is a TUI-layout discipline, not a cross-cutting domain rule), no
standards-skill edit, **no new ADR** (ADR-0007 governs the contract and was already on `main`),
**no new crate → no new dev agent**.

**Homes.** Feature-local on the branch (home #2): the item's `## Summary` (with the coverage line)
and this `[eng-manager]` Log context, committed on `feature/0010-notes`. Cross-cutting/derived on
`main` (homes #1/#3): this `docs/handoff.md` entry (+ the "What works right now" snapshot refreshed
for the 0010 state), the `tui-dev` caption/band learning, and the regenerated `board/README.md`.
**`main`'s frozen copy of `board/features/0010-notes.md` stays untouched** at the claim snapshot
(`ready`) until the human's merge. The orchestrator flips the branch status to `awaiting-merge`
after this step.

**Free pickup noted (mintable `chore`):** none this cycle.

[adr-0007]: ./adr/0007-notes-wire-contract.md

---

## Handoff — 2026-06-24 (0009 — coverage capture wired into the cycle + each Summary; chore, `main`-only)

The operator's process request — *"add the coverage run in the process, and report the code
coverage percentage in the summary of the tasks when they are awaiting merge"* — shipped as a
`main`-only governance `chore` (commit `6b6e373`, code-hash
`3fa0adefce8cd6d67ae716dae7a24ce6dbf9defd`). **No worktree was cut**: every edited file is home-#1
shared state (drive SKILL, CLAUDE.md, the eng-manager agent def) that must never ride a feature
branch, so `eng-manager` applied the edits directly on `main` and the orchestrator advanced status
in place. It ran the **lighter chore DoD** — gates green + a cold reviewer approval **attesting the
chore invariant** (code-path digest byte-identical to pre-0009 `cef68fe` ⇒ zero code delta) — and
the **live verifier pass was correctly skipped** (chore clause 4 N/A).

What shipped (three governance edits, all on `main`):

- **`drive` SKILL step 6** — step 6 now runs `./ok.sh coverage`, parses the headline workspace
  coverage %, and writes a `coverage: NN.N%` line into the item's `## Summary` (or
  `coverage: unavailable (docker)` when docker / the throwaway test Postgres cannot boot). Runs on
  **every** cycle (feature and chore); **report-only — never a gate**.
- **`CLAUDE.md` Definition of done** — a short gate-neutral note: the Summary records the coverage
  % for both `feature` and `chore`, for visibility only — not a clause, no threshold, never
  blocking; docker-unavailable becomes `unavailable (docker)` and the cycle proceeds. Consistent
  with the "How to run" `coverage` row.
- **`.claude/agents/eng-manager.md` charter** — the Summary-filling bullet now explicitly includes
  the coverage capture + `unavailable (docker)` fallback (report-only).

**This cycle dogfoods the very feature 0009 introduces.** 0009's own `## Summary` is the **first
item Summary to carry a coverage line**: `coverage: 66.36% line (61.48% region, 66.67% function)`,
the headline `TOTAL` from a fresh `./ok.sh coverage` run (docker + throwaway test Postgres booted
cleanly, same as `./ok.sh test` this cycle — nothing acquired, hard constraint #6 intact). Matches
the ~66% line / ~61% region 0007 baseline; report-only, no target to hit.

**0009 depended on 0007** (the `./ok.sh coverage` verb), which **merged first** — 0009 consumes
that verb and could not start until it landed on `main` (`grep -c cmd_coverage ok.sh` == 2).

Durable learnings:

- **`drive` SKILL + `git-standards` (reinforcing learned 0003/0004).** The
  `noreply@anthropic.com`-in-a-dispatch-prompt failure surfaced **again** on 0009: the dispatch
  prompt hardcoded `Co-authored-by: … <noreply@anthropic.com>`, corrected to the
  `*@organized-koala.local` form per git-standards (the footer identity is owned by that skill,
  never copied from a prompt; `<noreply@anthropic.com>` is never correct in this repo). Because
  this is now a **third recurrence**, the durable fix moved to the *dispatcher* side: a new
  **"Dispatch discipline"** note in `drive`'s Procedure preamble — never write a `Co-authored-by:`
  line into a dispatch prompt; state the committing agent's role and let `git-standards` supply the
  trailer — plus a cross-referencing one-liner appended to `git-standards`. The agent-side rule was
  already correct; the gap was that prompts kept injecting the wrong trailer, so the prevention
  belongs where the prompt is authored.
- **No new ADR, no new crate, no new dev agent, no new CLAUDE.md hard-constraint.** A chore makes no
  contract/domain decision; the coverage metric is already operator-sanctioned (0007) and stays
  report-only. The three-home model, chore DoD, scope guard, and verdict-pinning all behaved as
  written.

Process note worth keeping (not an edit): a `main`-only governance chore has **no worktree and no
branch**, so step 6's "coverage line is committed on the branch (home #2)" guidance resolves to
"on `main`" for this item — the Summary lives on `main` alongside the rest of the change. The
SKILL/CLAUDE.md wording already states this explicitly, so it needed no correction; recorded here
as the worked example of the `main`-only path through the new step-6 rule.

**Homes.** Everything is on `main` (this is a `main`-only item): the three governance edits
(`6b6e373`), this `docs/handoff.md` entry (+ the "What works right now" snapshot refreshed for the
0009 state), the `drive`/`git-standards` dispatch-discipline edits, the item's `## Summary`
(home 1 for a `main`-only item — there is no branch), and the regenerated `board/README.md` (home 3).
`branch: null` / `worktree: null` stay. The orchestrator flips 0009 to `awaiting-merge` in place on
`main` after this step.

**Free pickup noted (mintable `chore`):** none this cycle.

---

## Handoff — 2026-06-23 (0007 — report-only `./ok.sh coverage` verb; chore, lighter DoD)

The operator-sanctioned coverage follow-up (captured in the 2026-06-12 0003 handoff, item #2,
and carried on the dashboard as the "sanctioned follow-up" note) shipped as a `chore`. Branch:
`feature/0007-ok-coverage-verb` (code sha `e65a097`, code-hash
`3fa0adefce8cd6d67ae716dae7a24ce6dbf9defd`). It ran the **lighter chore DoD** — gates green + a
cold reviewer approval attesting the chore invariant — and the **live `verifier` pass was
correctly skipped** (a chore changes no behaviour/wire/API, so there is nothing for a live boot
to exercise). The cycle stopped at the AI-terminal `awaiting-merge` on the branch.

What shipped (`ok.sh` only, on the branch):

- **A `coverage` verb** — `cmd_coverage` + a `coverage)` case branch + a no-arg usage/help line.
  It runs `cargo llvm-cov --workspace --summary-only "$@"` (extra ARGS pass through) and
  **mirrors `cmd_test`'s live-DB wiring verbatim**: honour a caller-supplied `DATABASE_URL`, else
  boot the throwaway test Postgres via the test compose file and tear it down on a `RETURN` trap.
- **Report-only, no gate.** Prints a per-file table + a `TOTAL` line and exits 0 regardless of
  the number; **no threshold**, not wired into any Definition-of-done clause. This was the
  operator-sanctioned shape: coverage made *visible* without becoming a brittle pass/fail bar.
- **Coverage baseline at implementation time:** ~66% line / ~66% function / ~61% region
  (`TOTAL` line reported 61.48% region / 66.36% line). Captured here as a reference point, not a
  bar — there is no target to hit.
- **Chore invariant held.** No crate source, no behaviour, no `contract`/wire (#2), no
  domain-structure (#3) change — the diff is `ok.sh` (+31) plus the Board file. `cargo-llvm-cov`
  0.8.7 was already present and operator-sanctioned (hard constraint #6) — nothing acquired.

Verdict (chore track): **reviewer REVIEW-STATUS approved** @ code-hash
`3fa0adefce8cd6d67ae716dae7a24ce6dbf9defd` (commit `c4387b7`, for reference). Gates green
(`fmt --check` / `lint` / `test`); the **chore invariant is explicitly attested** (no behaviour,
no `contract`/wire, no domain-structure change); the verb is report-only. The code-hash is
byte-identical to the last-merged head, corroborating the tooling-only scope. No live verifier
pass (chore clause 4 skipped).

Durable learning: one small `bash-standards` addition (learned 0007) — *a report-only tooling
verb reuses the shared live-DB wiring (the `cmd_test` `DATABASE_URL`/compose/`RETURN`-trap
pattern) rather than re-deriving it, and stays honest by exiting 0 regardless of the metric; a
verb that can fail the build on a value is a gate, not a report.* No new ADR (a chore makes no
contract/domain decision), no new crate → no new dev agent, no `CLAUDE.md` hard-constraint
addition beyond documenting the verb in the "How to run" table.

**Homes.** Cross-cutting/derived on `main` (homes #1/#3): the `CLAUDE.md` "How to run" `coverage`
row, this `docs/handoff.md` entry (+ the "What works right now" snapshot refreshed for the 0007
state), the `bash-standards` learning, and the regenerated `board/README.md`. **Feature-local on
the branch (home #2):** only the item's `## Summary` (and its Log entries/verdict, already
committed on the branch). `main`'s frozen copy of `board/features/0007-ok-coverage-verb.md` stays
untouched at the claim snapshot (`ready`) until the human's merge.

**Free pickup noted (mintable `chore`):** none this cycle.

---

## Handoff — 2026-06-23 (0008-R1 — feedback re-entry: Pomodoro becomes a global widget; TUI-only)

**Feedback re-entry, not a fresh feature — the first re-entry on an item that had already reached
`awaiting-merge`.** 0008 (the account-global Pomodoro timer) was at `awaiting-merge` (verified
code-hash `708ee8d0…`) when the operator authored two `[human]` UI-feedback lines in its Log. The
cycle re-entered (drive step 0 feedback sweep), `architect` triaged, the work ran forward TUI-only,
and the item is back at the AI-terminal state on its branch. Branch: `feature/0008-pomodoro-timer`
(source `97b2b32`, tests `67e40af`; current HEAD `7ea1292` after this eng-manager step). Both
`[human]` boxes are now `[x]`.

The two feedback items:

- **suggestion(ui) — no dedicated timer page; make it an always-visible global widget.** The timer
  is a global concept, so it should be visible across pages (bottom-right), `p` to start/stop, and
  listed in the bottom-left help caption.
- **issue(ui) — flicker + over-frequent refresh.** The "(working…)" text replacing the hotkey
  caption every coarse poll causes flicker; append a spinner to the end instead, and check the
  session ~1/min rather than ~5 s.

What changed (TUI-only, on the branch):

- **ADR amendment first, on `main`.** Because this is scope/approach feedback, `architect` amended
  **[ADR-0006][adr-0006] §8** (commit `af582e6` on `main`) before re-implementation — §8.1 global
  widget (not a `Screen`), §8.2 global `p` toggle + help-caption entry, §8.3 append-spinner (not
  caption-replacement), §8.4 ~1-min coarse cadence. **ADR-0002 (timer authority/render model) is
  unchanged** — the server still owns the countdown; the TUI still renders from `ends_at` +
  `server_now` + a monotonic `Instant`. The branch was rebased onto `af582e6` before `tui-dev`
  cited §8.
- **Source (`tui-dev`, `97b2b32`).** Removed `Screen::Timer` and its `t`/`Esc` navigation; promoted
  the timer's transient render state to an app-level `app::timer::Timer` field rendered bottom-right
  on every post-auth screen (auth/offline excluded). Added `Event::ToggleTimer` mapped to `p`
  (resolves to start when idle/completed, stop when running, stamping the timer's own in-flight
  marker independent of the screen marker); `p` + `d: set duration` added to the bottom-left
  caption; `p`/`d` suppressed while a text-entry sub-flow owns keystrokes. Replaced `working_hint`
  (caption substitution) with `caption_with_spinner` that **appends** a trailing spinner + "Esc to
  cancel" to the stable caption on every screen. Raised `TIMER_REFRESH_TICKS` 63 → **750** (~1 min);
  the refresh + initial load now fire on any post-auth screen. **No `contract`/protocol/client/
  worker shape changed** — the existing timer wire/protocol is reused verbatim; account-global
  preserved (no `profile_id`).
- **Tests (`tester`, `67e40af`).** Adapted the `TestBackend`/core suite to the global-widget model
  (`map_key` now takes `editing_duration: bool`; the timer loads off edge hooks, not an `Event`) and
  extended coverage to the re-entry acceptance criteria by name: global widget render, `p`
  start/stop/when-completed, second-`p`-while-pending no-op, append-spinner-no-flicker regression
  guard, `p` suppressed-while-editing, `t`-opens-nothing regression guard, coarse-refresh picks up
  the server verdict, account-global call-shape sweep. Counts: tui keybindings 19 / rendering 11 /
  timer 17 / flows 9 / in_flight 5 / error_branches 10; full workspace green.

Verdicts (both pinned to code-hash `3fa0adefce8cd6d67ae716dae7a24ce6dbf9defd`; the original 0008
`708ee8d0…` verdicts were **voided** when the re-entry moved the code-tree):

- **Reviewer: REVIEW-STATUS approved** (`67e40af`). Gates green; **#1** holds (app-level `Timer` is
  transient render state, no stored remaining-seconds); **#2** holds and is **byte-identical** —
  `git diff` over `crates/contract` + `crates/server` + the `tui` protocol/client is empty (this
  bounds the verifier to the TUI surface); **#4** holds (no `profile_id`); ADR-0006 §8 fidelity
  confirmed. No blocking findings, no out-of-scope nits worth a chore.
- **Verifier: VERIFY-STATUS verified** (`09470e9`). Independently confirmed the #2 byte-identity
  (full delta confined to `crates/tui/src/{app,terminal,ui}` + `crates/tui/tests/**` + the Board
  file). Docker present + sanctioned (installed nothing, as in the original pass), so the live wire
  pass was re-performed, not deferred: `./ok.sh up`, live `GET/PUT /api/timer/config`, session
  start/stop, error contract `{code,message}`, OTel spans on all five handlers; `./ok.sh down`
  clean. The `TestBackend` suite asserts the re-entry behaviour by name.

Durable learnings: **none earned a durable `CLAUDE.md`/standards-skill edit.** The candidate was
this being **the first feedback re-entry on an already-`awaiting-merge` item** — but the mechanics
played out *exactly* as the existing CLAUDE.md "Feedback re-entry" + "Verdict pinning" + three-home
text already prescribes, with no ambiguity to resolve: the unchecked `[human]` box was the only
re-entry signal; the scope/approach feedback required an ADR amendment, which (as home #1
cross-cutting state) landed on `main` first; the branch was rebased onto it; the item dropped
`awaiting-merge` → `working`; the prior `approved`/`verified` verdicts were void **because the
code-tree hash moved** (`708ee8d0…` → `3fa0adef…`, not because shas changed); and the full feature
track re-ran (build → review → verify) on the new tree. That is the written rule exercised
faithfully, not a gap in it — so it is recorded here as the worked example rather than manufactured
into a new gotcha. One observation worth keeping (not an edit): **an ADR *amendment* is home #1
just like a new ADR** — it must land on `main` before the branch can cite it, and the code-hash
movement it implies is what voids the prior verdicts; the re-entry confirmed both halves hold for
an amendment, not only a fresh ADR. No new `docs-/bash-/coding-/git-standards` edit, no new ADR
beyond the §8 amendment, no new crate → no new dev agent.

**Homes.** Cross-cutting/derived on `main` (homes #1/#3): this `docs/handoff.md` entry (+ the "What
works right now" snapshot refreshed for the 0008 end state), and the regenerated `board/README.md`.
The ADR-0006 §8 amendment + decisions-index row already landed on `main` (`af582e6`, by
`architect`). **Feature-local on the branch (home #2):** the item's updated `## Summary`, the
`[eng-manager]` Log entry, and the two `[x]`-checked `[human]` feedback boxes. The orchestrator
flips the branch status back to `awaiting-merge` after this; **`main`'s frozen copy of
`board/features/0008-pomodoro-timer.md` stays untouched** at the claim snapshot until the human's
merge.

---

## Handoff — 2026-06-23 (0008 — account-global Pomodoro focus timer, end-to-end across all three crates)

Branch: `feature/0008-pomodoro-timer` (last code sha `fc894ce`, code-hash
`708ee8d0085ce9b3af68eb7e1b76dbe56a6185da`). The **first feature of the Focus phase** — the
Pomodoro timer end-to-end, implementing [ADR-0002][adr-0002] (timer authority) without reopening
or amending it. The cycle ran build → cold review → live verify and stopped at the AI-terminal
`awaiting-merge` on the branch.

What shipped (on the branch):

- **`contract` — a new `timer` module.** `TimerConfig { duration_minutes }`,
  `UpdateTimerConfigRequest { duration_minutes }`, and a tagged `TimerSession` enum
  (`#[serde(tag = "state", rename_all = "lowercase")]`) with `Idle` / `Running` / `Completed`;
  the running/completed variants carry `started_at`, `ends_at`, `duration_minutes`, and
  `server_now`. Datetimes serialize RFC 3339 `Z` exactly as `Task::created_at`; the established
  derive/rustdoc/doctest layout is followed; the three items are re-exported from `lib.rs`. No new
  `ErrorCode`, no secrets, nothing beyond the ADR-0002 shapes (#3 flat).
- **`server` — five account-global endpoints + a reversible migration.** All keyed on
  `AuthUser.user_id` with **no `profile_id` in any path** (#4 / ADR-0002 §5): `GET`/`PUT
  /api/timer/config` (default 30 lazily, upsert, `[1, 1440]` bound → `400 ValidationFailed`
  outside, reusing the `{ code?, message }` contract, no new `ErrorCode`); `GET
  /api/timer/session` (idle / running / completed, completion decided read-time when `server_now
  >= ends_at`); `POST /api/timer/session/start` (snapshots the configured duration;
  start-while-active replaces — A5); `POST /api/timer/session/stop` (clears the active row,
  idempotent when idle). Migration `20260612163048_timer.{up,down}.sql` creates `timer_configs` +
  `timer_sessions`, both `user_id UUID PRIMARY KEY` (schema-enforced at-most-one config / one
  active session per account); `ends_at` is **derived** (`started_at + duration_minutes`), never
  stored; the `down` drops both tables. `#[tracing::instrument]` spans on every handler;
  `i32`↔`u32` at the DB boundary via `try_from`, never `as`. `.sqlx/` refreshed against the
  sanctioned project test Postgres.
- **`tui` — a focus/timer view with a render-only countdown.** `Screen::Timer`, reachable with
  `t` from the task list (`s` start, `x` stop, `d` set duration, `r` refresh, `Esc` back). The
  live `MM:SS` countdown is **render-only** (#1-safe): **no** authoritative remaining-seconds
  integer is stored — the label is recomputed every ~80 ms render tick as `ends_at` minus
  `(server_now + elapsed_since_response)`, where `elapsed_since_response` comes from a monotonic
  `Instant` captured when the response landed. Coarse session re-reads are ~5 s (A3) — never
  per-second, no tick stream (stays inside [ADR-0006][adr-0006]). On reaching `00:00` locally it
  shows "Completed (awaiting server confirmation)" until the server's authoritative `Completed`
  verdict arrives.
- **Tests (tester).** `contract` 19 (round-trip, tagged-enum wire shape, `Z` offsets), server 21
  `#[sqlx::test]` (config default/persist/bounds, start→running with consistent instants, stop,
  start-replaces-active, account-global with two accounts, auth-required), tui 14 `TestBackend` +
  5 keybinding (navigation, running countdown rendered via `countdown_label`, stop, set-duration +
  inline validation, completed render, in-flight spinner, cancel/stale-id drop, account-global /
  profile-switch-unchanged). The one positive completion-at-`ends_at` transition is deliberately
  left to the live verifier (forcing `now >= ends_at` would need a real ~60 s sleep the suite
  avoids — noted inline in `shortest_session_reads_running_not_completed`).

Verdicts (both pinned to code-hash `708ee8d0085ce9b3af68eb7e1b76dbe56a6185da`, sha `fc894ce`):

- **Reviewer: REVIEW-STATUS approved.** Gates green (contract 19 / server 21 / tui 14 + 5
  keybinding, lint `--all-targets` clean, fmt clean). Risk-surface all HOLD: **#1** stateless
  (countdown recomputed each draw, nothing persisted); **#4 / ADR-0002 §5** account-global (the
  routes and client methods key on `user_id`, tables `user_id PRIMARY KEY`); **#3** flat
  (duration the only knob, no pause); **#2 / ADR-0002** contract is single source of truth, no
  new/amended ADR; reversible migration with `ends_at` derived; `{ code?, message }` reused, no new
  `ErrorCode`; no `as` at the DB boundary; spans on all five handlers; the three `#[allow]` are the
  sanctioned test-only exception.
- **Verifier: VERIFY-STATUS verified.** Live against `./ok.sh up` (docker present, migrate
  one-shot exited 0, both tables created). **Completion DIRECTLY OBSERVED** (not inferred): a
  1-min session polled every 5 s flipped running→`completed` when `server_now >= ends_at`; the row
  was kept (`count=1`, re-read still `completed`) until `stop` (`count=0`, idle). **Persistence
  across `docker compose restart server`**: config + running session survived (only `server_now`
  advanced) → state lives in Postgres. Account-global (no `profile_id`; second account
  independent), auth (no-bearer → `401 unauthenticated`), and OTel spans for all five handlers
  with `code.namespace: server::handlers::timer` + the `user_id` attribute. ADR-0003 handshake:
  the `TestBackend` suite present + green. Stack torn down cleanly.

Durable learnings captured this cycle: **none earned a durable `CLAUDE.md`/standards-skill
edit.** The candidate considered was the **render-only countdown pattern** — a #1-safe
live-updating-but-server-authoritative value computed each draw from a server-provided absolute
instant (`ends_at` + `server_now`) plus a monotonic `Instant`, never stored as a counter. It is a
clean, reusable idiom, but it does **not** generalize beyond what is already written: it is a
direct specialization of [ADR-0006][adr-0006] §5 (transient process-lifetime render state, the
same category as the in-flight spinner marker), the #1 statelessness invariant, and the
pure-core/effectful-shell rule already in `rust-standards` (learned 0004/0005). Manufacturing a
new skill entry would duplicate those, so the pattern is recorded **here** as the worked example
rather than promoted into a standard. No new `CLAUDE.md` gotcha (no recurring miss surfaced — the
three-home model, contract-frozen boundary #2, statelessness #1, and account-global #4 all held
cleanly), no `docs-/bash-/coding-/git-standards` edit, no new ADR (inside ADR-0002/0003/0006).

**No new crate** → no new dev agent: the timer is a module *inside* the existing crates
(`crates/contract/src/timer/`, `crates/server/src/handlers/timer.rs`, `crates/tui/src/app/timer.rs`),
each already owned by `contract-owner` / `server-dev` / `tui-dev`. Confirmed, not skipped.

**Free pickup spotted (mintable `chore` for a future cycle):** the `tui` timer-edit sub-flow
mirrors the existing `AddTaskState` text-entry pattern closely enough that the two could share a
small `TextEntryState` helper — a pure refactor with no behaviour / contract / domain change. Not
filed here (recorded so the orchestrator can mint it directly if desired); low priority.

**Homes.** Cross-cutting/derived on `main` (homes #1/#3): this `docs/handoff.md` entry (+ the
"What works right now" snapshot refreshed), `docs/build-plan.md`, and the regenerated
`board/README.md`. **Feature-local on the branch (home #2):** the item's `## Summary` + the
`[eng-manager]` Log entry. The orchestrator advances the branch status to `awaiting-merge` after
this; **`main`'s frozen copy of `board/features/0008-pomodoro-timer.md` is left untouched** at the
claim snapshot until the human's merge.

[adr-0002]: ./adr/0002-pomodoro-timer-authority.md

---

## Handoff — 2026-06-23 (0006 — inaugural `chore`: stale `tui/src/main.rs` doc comment fixed)

Branch: `feature/0006-tui-mainrs-stale-doccomment` (last code sha `e218f73`, code-hash
`401ad3de59c4cc7e33c3ebf8308c171d80659e4e`). **The first `chore` through the pipeline** — the
new lightweight item type (introduced as a learned-0005 governance follow-up) made its first
real trip end-to-end. The cycle ran mint → claim → build → cold review → (verify skipped) and
stopped at the AI-terminal `awaiting-merge` on the branch.

What shipped (on the branch):

- **Comment-only fix.** The module doc comment at `crates/tui/src/main.rs:1` described an
  *"initial health probe so an unreachable server is reported up front"* — behaviour 0005
  removed when it reshaped the entrypoint to ADR-0006 Model A. The comment was rewritten to
  describe the actual entrypoint: resolve base URL → build the `reqwest` client → **spawn the
  worker thread that owns it** → hand control to the interactive loop, where the UI thread
  drives the pure `tui::app::App` core and never blocks on I/O. The `anyhow`
  error-propagation note was kept. The diff vs `main` is the `//!` block only.
- **Chore invariant held.** No code path, signature, behaviour, `contract`/wire (#2), or
  domain-structure (#3) change.

Verdict (chore track):

- **Reviewer: REVIEW-STATUS approved** pinned to code-hash
  `401ad3de59c4cc7e33c3ebf8308c171d80659e4e` (sha `5b5c788`). The cold pass verified the new
  comment line-by-line against `main()` (no health probe; worker-spawn / pure-`App` /
  `terminal::run` / `anyhow`), gates green, and — as the strengthened chore-DoD clause 6
  requires — **explicitly attested the chore invariant** (no behaviour, no contract/wire, no
  domain-structure change; comment-only).
- **Verifier: SKIPPED (clause 4 N/A).** Per the chore track, the live boot was not run — a
  comment-only change has nothing new to exercise, and the cold reviewer is the safety net in
  its place.

The chore lane worked exactly as designed: mint-without-`architect`-plan → claim →
single-agent build → invariant-attesting cold review → live verify skipped → `awaiting-merge`.
The 0005 handoff's **"free pickup" prose is now resolved** — it was tracked as `0006` and has
flowed to terminal.

Durable learnings: **none earned a durable edit.** The chore machinery (DoD, scope guard,
three-home model, verdict pinning) was freshly exercised and behaved as written — no clause
was ambiguous, the mint-without-plan path was unambiguous, and verdict pinning to the
code-tree hash held (the branch was already current on `main`, code-hash unchanged, so
step-7 was a no-op freshen). No `CLAUDE.md` gotcha, no standards-skill edit, no agent edit,
no new ADR, no new crate. Recording explicitly rather than inventing churn: **the first chore
needed zero process correction.** One observation worth keeping (not an edit): the chore lane's
value is precisely that a one-line doc fix no longer has to masquerade as a full `feature`
cycle — the thing it was created to fix.

**Homes.** Cross-cutting/derived on `main` (homes #1/#3): this `docs/handoff.md` entry (+ the
"What works right now" snapshot refreshed), `docs/build-plan.md`, and the regenerated
`board/README.md`. **Feature-local on the branch (home #2):** the item's `## Summary`. The
orchestrator advances the branch status to `awaiting-merge` after this; **`main`'s frozen copy
of `board/features/0006-tui-mainrs-stale-doccomment.md` is left untouched** at the claim
snapshot until the human's merge.

---

## Handoff — 2026-06-22 (0005 — TUI responsive (non-blocking) event loop + `tui::app` reorg)

Branch: `feature/0005-tui-responsive-event-loop` (last code sha `a4f99fd`, code-hash
`bc89672d4be5cdecd0bb54b340a24a5b8741cf21`). The first item past the foundational slice: it
resolves 0004's re-homed responsiveness feedback (*"the TUI freezes during every HTTP
request"*) and folds in the requested `tui::app` submodule reorg (both restructure the same
module). The cycle ran build → review → verify and stopped at the AI-terminal `awaiting-merge`
on the branch. **TUI-only — `crates/contract` and `crates/server` are byte-identical to base
`f0204fd`; no wire change, no new ADR beyond 0006.**

What shipped (on the branch):

- **Responsive UI per [ADR-0006][adr-0006] Model A** — synchronous `Client` on a worker thread,
  `std::sync::mpsc` request/response, a polled render loop, **no `tokio`/async runtime**. The UI
  thread never blocks on IO; a spinner animates and Esc(cancel)/Ctrl+C,`q`(quit) stay live in
  flight. `client/worker.rs` is a single thread owning the real `HttpClient`, mapping a
  `ClientRequest` → `Outcome` over two `mpsc` channels (no new dep). `terminal::run` is now a
  poll loop: `event::poll(80ms)` for input + `try_recv` response drain + per-tick redraw. A 30s
  `reqwest` timeout bounds an abandoned request (the `Client` trait is unchanged). `main.rs`
  spawns the worker and passes the channels in.
- **Client-free pure core.** The `App<C>` generic is gone. The core is two pure seams:
  `handle_event(Event) -> Option<Dispatch>` and `apply_response(ClientResponse) ->
  Option<Dispatch>` (chaining follow-ups — post-auth profile→task load, post-create refresh).
  Error-code branching is preserved unchanged and routes async-arriving responses through the
  same handlers.
- **One-in-flight + cancel.** Each screen carries a transient `pending: Option<RequestId>`;
  while set, request-triggering events are no-ops, `Cancel`/`Quit` stay live. Cancel is
  user-perceived — the screen leaves the in-flight state at once and a superseded response is
  dropped by `RequestId`-mismatch in `apply_response`; the worker is not force-killed.
- **`tui::app` reorg.** `app/mod.rs` keeps `App`/`Screen`/`Session`/`Event` + the
  `handle_event`/`apply_response` wiring; `app/protocol.rs` holds the pure
  `ClientRequest`/`ClientResponse`/`Outcome`/`RequestId`/`Dispatch` types; feature submodules
  `auth.rs`/`task_add.rs`/`task_list.rs` each own their screen state and handlers.
- **Tests (tester).** Added a synchronous request executor to `tests/common/mod.rs`
  (`execute`/`drive`/`submit`) — the test-side analogue of the worker thread: it maps a
  `Dispatch`'s `ClientRequest` through the `FakeClient` (the sole external-service mock) to a
  `ClientResponse` and feeds it back into `apply_response`, looping on chained follow-ups until
  the flow settles. No internal collaborator is mocked. The `TestBackend` suite (ADR-0003 layer
  2) is green and extended for in-flight render/no-op, cancel + stale-`RequestId` drop,
  at-most-one-chained-request, and Esc→Cancel/Ctrl+C→Quit while pending (tui: flows 9,
  error_branches 10, in_flight 5, keybindings 13, rendering 11).

Verdicts (both pinned to code-hash `bc89672d4be5cdecd0bb54b340a24a5b8741cf21`):

- **Reviewer: REVIEW-STATUS approved.** Gates green; `handle_event`/`apply_response` pure,
  `App<C>` gone; one-in-flight invariant holds; stale/superseded `RequestId`-mismatch drop
  correct; error-code branching preserved; `contract` diff empty; no tokio/async
  (`reqwest::blocking` + `std::thread` + `std::mpsc`); 30s timeout + clean worker teardown; no
  secret-leak path; tests are public-API with only the `Client` trait mocked. One non-blocking,
  **pre-existing** nit: a stale doc comment at `main.rs:4` (about an initial health probe —
  already stale at base `f0204fd`, out of scope here; **flagged for opportunistic cleanup in a
  future TUI-touching cycle**).
- **Verifier: VERIFY-STATUS verified.** Confirmed `crates/server`+`crates/contract` diff vs
  `f0204fd` empty. Live over `./ok.sh up` (Docker 29.5.3 + Compose; postgres → migrate one-shot
  → server → otel-collector): register/login, `GET /api/profiles`, task create/list/close, the
  `{code,message}` error contract with correct statuses, two-user profile-scoping isolation (no
  cross-profile read/write, 404 no existence leak), OTel server spans for every client call.
  ADR-0003 delegation handshake: `TestBackend` suites present + green. Inferred (code-read):
  that `HttpClient` issues exactly those requests — the standard ADR-0003 split (interactive TUI
  owned by the green `TestBackend` suite).

Durable learnings captured this cycle (each to the smallest right home, all on `main`):

- **rust-standards + tester agent — the worker-analogue synchronous test executor is the
  sanctioned way to test a pure `handle_event`/`apply_response` seam without async.** When the
  effectful shell is a worker thread + channels (ADR-0006 Model A), the test harness mirrors it
  with a small synchronous executor that maps each emitted `ClientRequest` through the injected
  fake `Client` and feeds the `ClientResponse` back into `apply_response`, looping on chained
  follow-ups. This drives the two-step seam end-to-end with the only mock being the sanctioned
  external-service trait — no internal collaborator, no async runtime. Recorded as the general
  pattern in `rust-standards` and as front-of-mind tester guidance.

Deliberately **skipped** (did not earn a durable edit): **no `CLAUDE.md` gotcha** — this cycle
hit no new recurring miss. The three-home model, the contract-frozen boundary (#2), and
statelessness (#1) all held cleanly, and the pure-core/effectful-shell rule (the executor's
foundation) already lives in `rust-standards`. No `docs-standards`/`bash-standards`/
`coding-standards`/`git-standards` change — nothing new surfaced there. **No new crate** → no
new dev agent; `tui-dev` already owns `crates/tui`. No new ADR — inside ADR-0006/ADR-0003.

Next cycle should know:

- **The poll-loop redraw path is a new candidate trigger for `docs/manual-smoke.md`.** Spinner
  repaint and terminal raw-mode teardown are invisible to `TestBackend` (accepted residual risk
  per ADR-0003 §3); when the manual-smoke checklist is authored, add a "request in flight →
  spinner animates, Esc cancels, terminal restores cleanly on quit" item.
  **✓ Resolved `4318d65`** — the checklist already existed; the in-flight item + a poll-loop-path
  trigger were added to `docs/manual-smoke.md` directly (docs-only, main-side, no cycle).
- The **`main.rs:4` stale doc comment** (pre-existing health-probe nit) is a free pickup for the
  next `tui-dev` touch.
  **✓ Resolved** — filed and run as Board chore `0006` (the inaugural `chore`); the comment now
  describes the ADR-0006 worker/pure-`App` entrypoint. Reviewed (chore invariant attested) at
  `awaiting-merge`; see the 0006 handoff entry above.
- Still pending from earlier cycles (not lost): the operator-sanctioned reported-only `./ok.sh
  coverage` verb over `cargo-llvm-cov` (no hard threshold) — `architect` to plan as a `main`-side
  item; and the deferred TUI backlog (profile-switch UX, task edit/delete, Notes, Pomodoro gated
  on ADR-0002, TUI-side OTel).

**Homes.** Cross-cutting edits on `main` (homes #1/#3): this `docs/handoff.md` entry (+ the
"What works right now" snapshot refreshed), the `rust-standards`/`tester` learning,
`docs/build-plan.md`, and the regenerated `board/README.md`. **Feature-local on the branch
(home #2):** the item's `## Summary`. The orchestrator advances the branch status to
`awaiting-merge` after this; **`main`'s frozen copy of
`board/features/0005-tui-responsive-event-loop.md` is left untouched** at the claim snapshot
until the human's merge.

[adr-0006]: ./adr/0006-tui-concurrency-and-responsiveness.md

---

## Handoff — 2026-06-18 (0004 — TUI: register/login + profile + task add/list/close; slice 0001 closes)

Branch: `feature/0004-tui-foundational` (last code sha `8fb0505`). Slice 3 of 3 of the 0001
umbrella — the TUI side of the tracer bullet, closing the loop TUI ↔ `contract` ↔ server ↔
Postgres. The cycle ran build → review → verify and stopped at the AI-terminal `awaiting-merge`
on the branch.

What shipped (on the branch):

- **`crates/tui`** (binary `organized-koala`, lib+bin split) — `ratatui` 0.29, `crossterm`
  0.28, blocking `reqwest` 0.12 (rustls). The crate was **auto-discovered** by the existing
  `members = ["crates/*"]` glob; **no root `Cargo.toml` edit** was needed.
- **`src/client/`** — a `Client` trait over health/register/login/list-profiles/list-tasks/
  create-task/close-task, every method typed on `contract` DTOs (no local wire types —
  hard-constraint #2). The `reqwest` impl is `HttpClient`; the standard `ErrorBody`
  (code + message) maps to a typed `ClientError` (`Api` preserving the `ErrorCode` for
  branching; `Offline` for any transport failure or unintelligible body).
- **`src/app/`** — a **pure** screen state machine (`Auth` → `TaskList`, plus a blocking
  `Offline` screen) advanced by `App::handle_event` over a transport-agnostic `Event` enum,
  with the `Client` injected. Auth: login (identifier + password) and register (username,
  email, password, profile-name); on success fetches `GET /api/profiles`, auto-selects the
  first profile (per the plan's single-profile Assumption), loads its task list. Task list:
  newest-first with done/undone markers, add-task sub-flow (Title + Description), mark-done
  sends `…/close` and replaces the row from the server response, refresh re-fetches.
  Error-code branching per ADR-0005: `unauthenticated` drops the in-memory session → login;
  `validation_failed`/other `Api` errors surface inline; transport failure → blocking offline
  screen with a manual retry. **JWT + active profile id live in process memory only**
  (hard-constraint #1; no on-disk/cross-run state).
- **`src/ui/`** pure draw fns; **`src/terminal/`** the crossterm driver with a pure `map_key`
  and a raw-mode guard restoring the terminal on drop.
- **Keybindings (now pinned by tests):** `Esc`/`Ctrl+C` quit (`Esc` = cancel in the add-task
  sub-flow); `Enter` submit; `Tab`/`Down` next, `Shift+Tab`/`Up` prev; `Backspace`; auth `F2`
  toggles login/register; task-list `a` add / `c` mark-done / `r` refresh / `q` quit; offline
  `r` retry; printable keys typed literally in text-entry contexts.
- **Tests (tester):** 35 `TestBackend` tests under `crates/tui/tests/` (the only mock a held,
  recording fake `Client` — ADR-0003 layer 2, no binary, no live DB): `keybindings.rs` (11)
  pinning the whole `map_key` contract incl. context-sensitivity, `rendering.rs` (7)
  buffer-snapshotting auth/task-list/add-task/offline (masked password — plaintext never
  rendered), `error_branches.rs` (9) driving the ADR-0005 `code` branches, `flows.rs` (8)
  the login/register→profile→list sequence, add-task, mark-done, and statelessness.

Verdicts:

- **Reviewer: REVIEW-STATUS approved `8fb0505`** — all four gates green at HEAD; hard-constraints
  #1/#2 held (no local DTOs; no persistence/file-IO; offline path fabricates no cached data),
  the ADR-0005 error contract wired+tested, the layer-2 `TestBackend` suite present and green,
  no contract/migration/shared-state drift, `#[allow]` audit clean. **No fix-now findings.** One
  non-blocking nit: the orchestrator's board-claim commit `846ba2a` used a
  `noreply@anthropic.com` co-author trailer instead of the project form (board-only, outside
  reviewed code) — now closed durably in `git-standards` (see learnings below).
- **Verifier: VERIFY-STATUS verified `8fb0505`** — capabilities present (Docker 29.5.3, Compose
  v5.1.4), **no gap**. Booted `./ok.sh up` in the worktree and exercised the live reqwest client
  path (ADR-0003 layer 1): every endpoint the `Client` consumes round-tripped with `contract`-
  matching shapes (`register` 201, `login` 200, `GET /api/profiles` 200, task list/create/close
  open→done with `closed_at` set); error contract verified live with exact wire strings
  (`unauthenticated`/`invalid_credentials` 401, `username_taken`/`email_taken` 409,
  `validation_failed` 400, `not_found` 404); profile-scoping (#4) with a second account → 404 no
  leak; persistence across a server restart; OTel spans received end-to-end by the collector. The
  layer-2 `TestBackend` suite confirmed green under `./ok.sh test`. Only un-driven items
  (neither a blocker): interactive crossterm on a real TTY (routed to the ungated
  `docs/manual-smoke.md` check per ADR-0003 §3) and the out-of-scope timer endpoint.

No contract change, no migration, no new ADR (TUI-only, inside the frozen ADR-0005 wire format
and ADR-0003 verification routing). **No new crate-dev agent** — `tui-dev` already owns
`crates/tui`.

Durable learnings captured this cycle (each to the smallest right home, all on `main`):

- **rust-standards + tui-dev agent — separate the pure core from the effectful shell to make an
  IO/interactive surface testable.** The whole TUI surface was `TestBackend`-driveable with no
  live server and no TTY because the crate is a pure update fn (`App::handle_event`), pure draw
  fns, and a pure `map_key`, with the one external service (the server) behind an injected
  `Client` trait. That is the ADR-0003 layer-2 enabler; recorded as the general rule in
  `rust-standards` and as a front-of-mind constraint on the `tui-dev` agent.
- **git-standards — the orchestrator's co-author trailer is `claude <claude@organized-koala.local>`,
  and that applies to Board-only commits too.** The 0004 board-claim commit used
  `<noreply@anthropic.com>` (the reviewer's nit). Tightened the existing footer rule to pin the
  orchestrator's domain form explicitly and state `<noreply@anthropic.com>` is never correct here.

Deliberately **skipped** (did not earn a durable edit): no `CLAUDE.md` gotcha — this cycle hit
no new recurring miss (the three-home model and #6 held cleanly; the auto-discovery of the crate
via `members = ["crates/*"]` and the lib+bin split are already captured). No `docs-standards`,
`bash-standards`, or `coding-standards` change — nothing new surfaced there. No new ADR — TUI-only.

Backlog deferred per the plan's Assumptions (next cycles, not lost): profile picker / multiple-
profiles switch UX; task edit/delete; Notes; Pomodoro (still gated on ADR-0002 timer authority);
TUI-side tracing/OTel; and the `docs/manual-smoke.md` TTY checklist for raw-mode/teardown
behaviour invisible to `TestBackend`. Also still pending from 0003: the operator-sanctioned
reported-only `./ok.sh coverage` verb over `cargo-llvm-cov` (no hard threshold) — `architect` to
plan as a new `main`-side Board item.

**Sequencing — the foundational slice closes with this merge.** Merging
`feature/0004-tui-foundational` puts all three children (0002/0003/0004) on `main`, so parent
0001's end-to-end acceptance (register/login → profile → task add/list/close, TUI ↔ contract ↔
server ↔ Postgres) becomes closeable. `0001` is the only foundational item left open after this.

**Homes.** Cross-cutting edits on `main` (homes #1/#3): this `docs/handoff.md` entry (+ the "What
works right now" snapshot refreshed), the `rust-standards`/`git-standards`/`tui-dev` learnings,
`docs/build-plan.md`, and the regenerated `board/README.md`. **Feature-local on the branch
(home #2):** the item's `## Summary`. The orchestrator advances the branch status to
`awaiting-merge` after this; **`main`'s frozen copy of `board/features/0004-tui-foundational.md`
is left untouched** at the claim snapshot until the human's merge.

---

## Handoff — 2026-06-12 (0003 feedback re-entry — four human items resolved, re-verified, `awaiting-merge`)

**Feedback re-entry, not a fresh feature.** 0003 was at `awaiting-merge` (verified `f67a883`)
when the operator authored four `[human]` items in its Log. `architect` triaged them; the cycle
ran forward (triage → fixes → review → verify) and the item is back at `awaiting-merge` on its
branch. The four resolutions:

- **#1 (suggestion) — compose server healthcheck.** `7833b15` (platform-dev): a `healthcheck:`
  on the compose `server` service hitting pure-liveness `GET /healthz` on the in-container port
  8080, plus `curl` added to the slim runtime image. The verifier observed the container reach
  Docker `healthy` for real (probe ExitCode 0 in-container).
- **#2 (question) — no unit tests / coverage DoD.** Answered + a real gap closed. Zero server
  unit tests is policy-consistent (the public API is HTTP; coding-standards favours public-API
  coverage — 28 such tests exist). But `4c679bd` (tester) closed a **genuine** gap:
  expired-token→401 was untested at *every* layer, while a prior slice-5 Log entry had falsely
  claimed "source-owned jwt unit tests" that never existed. Closed at the HTTP layer by
  hand-signing an hour-past-`exp` token (outside jsonwebtoken's 60 s leeway) → 401
  `unauthenticated`, with a fresh-token control. **The coverage-DoD-in-CI part is a separate
  `main`-side decision the operator SANCTIONED:** add `cargo-llvm-cov` behind a new `./ok.sh
  coverage` verb for a **REPORTED** coverage metric with **NO hard threshold** — to be planned as
  a new Board item (see follow-up below). Not created here (that is `architect` planning).
- **#3 (nitpick) — redundant custom `Debug`.** `353026f` (server-dev): dropped the hand-written
  `Debug` on `Jwt`/`JwtConfig` for `#[derive(Debug)]` (`SecretString` already redacts);
  load-bearing custom impls (`Password`/`AppState`/`TelemetryGuard`) left intact.
- **#4 (question, DoS) — DB hit on every authenticated request?** Clarified, no change. Auth is
  stateless JWT verification with **zero** DB queries (`session.rs:37` → `jwt.rs:63-68`, no
  session table; the user id is the token `sub` claim). The premise did not hold; the only DB
  work on an authed request is the business query itself.

**Verdicts (feedback delta `fca5f53..HEAD`).** reviewer **`REVIEW-STATUS: approved 4c679bd`**;
verifier **`VERIFY-STATUS: verified 4c679bd`** — re-verified live via the sanctioned `./ok.sh
up`/`down` (Docker 29.5.3 / Compose v5.1.4): the `server` container went `starting` → `healthy`
(curl present in the slim image), migrate one-shot exited 0 before server start, regression
spot-check of register/login/task CRUD + error contract green, OTLP export re-confirmed.

**Follow-up the next cycle picks up — operator-sanctioned coverage verb.** A new Board item is
to be planned on `main` (`architect`): an `./ok.sh coverage` verb wrapping `cargo-llvm-cov` that
**reports** a coverage metric with **no hard threshold** (not a DoD gate). `cargo-llvm-cov` is
operator-sanctioned for this; `platform-dev` owns the verb, `eng-manager` documents it. This is
deliberately **not** created here — recorded so it is not lost.

**Learnings captured (each to the smallest right home):**

- **git-standards** — the co-author footer identity is owned by `git-standards`, **never copied
  from a dispatch prompt**. `353026f` committed with `<noreply@anthropic.com>` because the
  orchestrator's dispatch prompt hardcoded that trailer; the `<agent>@organized-koala.local` form
  is the only authority.
- **docs-standards** — two notes: (a) never let a wrapped Board prose line begin with `#` or a
  list-like token — `rumdl fmt`'s auto-fix splits the paragraph with an inserted blank line
  (MD032); reword (e.g. "constraints 1–6"); never blindly accept `rumdl fmt` on prose. (b) A
  successful commit does **not** prove markdown is lint-clean — `.githooks/pre-commit` is a
  secret-scan only; markdown linting is the PostToolUse `.claude/lint.sh` hook and does not gate
  commits, so run `rumdl check --config .claude/rumdl.toml <file>` explicitly.
- **coding-standards** + **reviewer agent** — a "covered by …" claim must name a test that
  actually exists. The slice-5 phantom-test claim let an untested `exp` path reach
  `awaiting-merge`; the reviewer now spot-checks that cited coverage is real (a phantom claim is
  changes-requested).

**Homes.** Cross-cutting edits on `main` (homes #1/#3): this `docs/handoff.md` entry (+ the
"What works right now" snapshot refreshed), the four standards/agent edits above, and the
regenerated `board/README.md`. **Feature-local on the branch (home #2):** the item's `## Summary`
and the four `[x]`-checked feedback items live on `feature/0003-server-auth-profile-tasks` and
return to `main` atomically at the human's merge. `main`'s frozen copy of the item is left
untouched at the claim snapshot.

---

## Handoff — 2026-06-12 (0003 re-verified under the sanctioned mechanism — block cleared, `awaiting-merge`)

**The capability-gap block on 0003 is cleared.** Docker was provisioned by the operator (Engine
29.5.3, Compose v5.1.4), and 0003 was re-verified **under the sanctioned mechanism only** — the
real docker-compose stack via `./ok.sh`, **no external binary acquired, no improvised DB**. This
closes the loop opened by the policy-correction entry below: a `blocked` capability gap is
recoverable.

**Re-entry mechanics.** drive re-entered 0003 at the **verify** phase — there was **zero code
change** (last code sha is still `f67a883`; only Board-only commits follow it), so the reviewer's
**`REVIEW-STATUS: approved f67a883` was preserved** (a board-only commit does not invalidate the
approval; the orchestrator confirmed no code commit follows the approved sha). No re-review was
needed; only the previously-void verifier verdict had to be re-earned live.

**Verifier verdict: `VERIFY-STATUS: verified f67a883`** (on the branch), closing both prior
environmental gaps live under docker:

- `./ok.sh test` — **28/28 green** on the compose Postgres.
- `./ok.sh up` full stack — the ADR-0004 migrate→run `service_completed_successfully` gating
  **proven via `docker inspect`**: the one-shot `migrate` `exited(0)`, and the `run` service
  started ~0.49 s later — never before. (Prior gap-1, the un-booted compose gating, is closed.)
- Full ADR-0005 HTTP surface live with exact codes/bodies; two-user profile isolation → 404
  `not_found`; idempotent re-close (byte-identical `closed_at`).
- **OTLP export observed live** — 31 spans landed in the collector `debug` exporter. (Prior
  gap-2, log-only degraded mode, is closed.)
- Secrets clean in logs/Board; clean teardown (`./ok.sh down`); read-only throughout.

drive then flipped the branch item **`blocked` → `review` → `awaiting-merge`**. 0003 is now at
the AI-terminal state, awaiting the human merge.

**The validated process learning — the recovery loop works end-to-end.** The
**block → escalate → human provisions the capability → re-verify under the sanctioned mechanism**
loop ran to completion and is now demonstrated, not just asserted. The takeaways worth keeping:

- **A `blocked` item is recoverable with zero code churn.** Because the block was purely
  environmental (a missing capability, not a code defect), provisioning docker was sufficient;
  nothing in `crates/server` or `deploy/` changed. The hard-constraint-#6 discipline (block +
  escalate, never engineer around) cost **no** rework — it deferred the *runtime confirmation*,
  not the *code*.
- **Re-entry was at verify, and reviewer approval at the last code sha survived.** The three-home
  Board model held: the cycle's intervening status changes were board-only commits on the branch,
  so `approved f67a883` still names the last code sha. The orchestrator's "no code commit follows
  the approved sha" check is what makes a verify-only re-entry safe — re-review is unnecessary.
- The existing **verifier agent instructions needed no edit** — it already encodes the
  block-and-escalate-on-missing-capability rule and the sanctioned-mechanism-only constraint
  (added in the policy-correction cycle). This re-entry *exercised* those instructions exactly as
  written; no refinement was manufactured.

**Sequencing — merge of 0003 unblocks 0004.** 0004 (TUI, the third and final slice of the 0001
umbrella) depends-on 0003 and becomes claimable once the human merges
`feature/0003-server-auth-profile-tasks`. No new crate dev agent — `server-dev` already owns
`crates/server`.

Docs updated (all on `main` — derived/cross-cutting, homes #1/#3): `docs/handoff.md` (this entry,
plus the "What works right now" snapshot refreshed); `board/README.md` regenerated (home #3,
derived).
**`main`'s frozen copy of `board/features/0003-*.md` is left untouched** at the claim snapshot
(`ready` + pointer) — the branch copy carries the live `awaiting-merge` status and verdicts and
returns to `main` atomically with the code at the human's merge (home #2). No `CLAUDE.md` or
agent/skill edit — the #6 policy and verifier discipline are already in place and were validated,
not changed.

---

## Handoff — 2026-06-12 (policy correction — no unsanctioned binaries; 0003 reverted to `blocked`)

**Operator policy correction, encoded on `main`.** Supersedes the docker-fallback framing in the
0003 entry below. Two linked, load-bearing rules now binding on every agent in every phase
(CLAUDE.md hard constraint **#6** + tightened "Ambiguity policy"):

1. **No agent downloads, installs, or runs an external binary without the operator's explicit
   approval** — including anything written into a dispatch prompt.
2. **A missing capability the Definition of Done needs (docker, a live DB, any required tool)
   sets the item to `blocked` with a precise question and STOPS for human intervention — it is
   never engineered around.** `verified-with-gaps` is for genuinely-minor *inferred* sub-items,
   never for "couldn't run it because a required tool was missing."

**Origin.** In the 0003 cycle docker was absent in the sandbox. The orchestrator authorized the
tester/verifier to "bootstrap a throwaway local Postgres"; they downloaded/ran an embedded
Postgres 16.2 and the verifier reused a leftover `/tmp/pgextract` binary. The operator has
**disavowed** this. The "binary + live-Postgres fallback" verification of 0003 is therefore
**void for sign-off**.

**Status change.** 0003 was moved **`awaiting-merge` → `blocked`** (on its branch — the
orchestrator committed the block + Log entry there; `main`'s snapshot stays frozen at the claim).
It is **not** heading to merge.

**Re-entry plan.** Operator sets up docker → 0003 is re-verified under the **sanctioned mechanism
only** (`./ok.sh up` / the real compose stack, no improvised DB, no downloaded binary) → back to
`awaiting-merge`. The reviewer's **`REVIEW-STATUS: approved f67a883` stands** (cold code review is
unaffected by the runtime gap); only the **verifier verdict is void** until the sanctioned live
pass is done.

**Docs corrected on `main`:** CLAUDE.md (hard constraint #6 + tightened Ambiguity policy);
`.claude/agents/verifier.md` (the 3ac2a46 "sanctioned binary/live-Postgres fallback + merge-time
ask" language **removed** and replaced with report-not-verified + block-and-escalate);
`.claude/agents/tester.md`, `server-dev.md`, `platform-dev.md` (each now carries the
no-unsanctioned-binaries / block-on-missing-capability rule); `bash-standards` (scripts fail loud
and escalate, never fetch+run); `board/README.md` regenerated (0003 → `blocked`). **Kept intact**
(correct learnings from 3ac2a46): the lib+bin rule in `rust-standards`/`new-crate`, and the
net-new-infra carve-out in CLAUDE.md "The Board" home #1.

---

## Handoff — 2026-06-12 (0003 — server: auth + default profile + tasks + migrations + docker stack)

> **SUPERSEDED in part by the policy-correction entry above (2026-06-12).** The "docker
> unavailable → sanctioned binary + live-Postgres fallback → verified-with-gaps → human boots
> `./ok.sh up` at merge" framing in this entry is **disavowed**. 0003 is **`blocked`**, not
> heading to merge; its verifier verdict is **void for sign-off** pending a sanctioned live pass
> on a docker host. The reviewer's `approved f67a883` stands. Read the entry below as the cycle's
> historical record, not as current policy or status.

Branch: `feature/0003-server-auth-profile-tasks` (last code sha `f67a883`). Slice 2 of 3 of the
foundational slice 0001 — the server side of the tracer bullet, verifiable live over HTTP before
the TUI exists. The cycle ran build → review → verify and stopped at the AI-terminal
`awaiting-merge` on the branch.

What shipped (on the branch):

- **`crates/server`** (binary `organized-koalad`) with the ADR-0004 admin CLI: `run` (default
  no-arg, **never** mutates schema), `migrate` (idempotent), `rollback` (one step default,
  bounded by `--steps`, never auto-invoked). Reversible paired `*.up.sql`/`*.down.sql`
  migrations for `users`/`profiles`/`tasks` (FKs profile→user, task→profile; flat task domain),
  embedded via `sqlx::migrate!`; committed `.sqlx/` offline cache.
- **Auth:** argon2id PHC hashing (constant-time decoy verify for absent users), JWT HS256
  (sub/iat/exp, expiry enforced; secret held as `SecretString`, redacted everywhere), the
  `AuthUser` Bearer extractor. Endpoints per ADR-0005: `register` (user + named default profile
  in one transaction → 201), `login` (username-or-email → 200), `GET /api/profiles`, profile-
  scoped `GET|POST .../tasks` + `POST .../tasks/{tid}/close`, `GET /healthz`.
- **Profile isolation** via ownership-joined queries → unowned/nonexistent profile is **404
  `not_found`** (never 403, no existence leak); title trimmed+non-empty (else 400
  `validation_failed`); close idempotent (preserves original `closed_at`). The thiserror
  boundary maps each case to HTTP status + `contract::ErrorBody { code?, message }` (internal
  causes logged, never sent). `tracing` spans + INFO mutation events on every endpoint; OTLP
  export gated on `OK_OTLP_ENDPOINT`, degrading to log-only when the collector is absent.
- **`deploy/` docker stack** (platform-dev): multi-stage Dockerfile (release build off the
  committed `.sqlx/`, slim runtime as an unprivileged user), `docker-compose.yml` with the
  ADR-0004 graph — Postgres (healthcheck) → one-shot `migrate` (gated `service_healthy`) →
  `run` (gated `service_completed_successfully`) → minimal OTel collector (OTLP/gRPC receiver +
  `debug` exporter). `ok.sh` wired: `up`/`down`, dev-only `migrate`/`rollback` delegating to the
  binary, `run-server`, and a `test` verb that boots a throwaway tmpfs Postgres for the
  `#[sqlx::test]` suite. The committed stack carries **no** credential literal (a gitignored
  `deploy/.env` with DEV-ONLY placeholders is generated by `up`; secret-scan clean).
- **Tests** (tester): 28 integration tests over the public HTTP surface (`auth` 14,
  `tasks` 9, `profile_isolation` 5) driving the real `axum` router in-process via
  `tower::ServiceExt::oneshot` over a per-test `#[sqlx::test]` DB. Every error asserts the exact
  ADR-0005 `code`, not just status.

Verdicts:

- **Reviewer: REVIEW-STATUS approved `f67a883`** — mechanical gate green (`fmt --check`/`lint`/
  `build`/`sqlx prepare --check`/`secret-scan`); no contract drift (server defines no DTO, maps
  at the boundary); endpoints/CLI/compose match ADR-0004/0005; hard constraints #2–#5 held;
  secrets redacted. Two non-blocking nits (unused `app_with_ttl` expired-token helper;
  `cmd_run_server` harmlessly forwards `"$@"` to argless `run`).
- **Verifier: VERIFY-STATUS verified-with-gaps `f67a883`** — docker unavailable in the sandbox,
  so used the sanctioned binary + live-Postgres fallback (real HTTP round-trips, nothing faked).
  Verified live: `./ok.sh test` **28/28 GREEN**, CLI run/migrate/rollback, the **migrate-before-
  serve seam** proven (fresh unmigrated DB: `register`→500 since serve never creates schema;
  after `organized-koalad migrate`, the same running server served `register`→201 with no
  restart), the full ADR-0005 surface with exact codes/bodies, **profile isolation across two
  users** → 404 `not_found`, idempotent re-close, tracing spans/INFO events, and **secrets
  absent from logs**.

Two verifier gaps — environmental, docker-only, NOT code defects:

1. `./ok.sh up` full compose stack + its `service_completed_successfully` migrate→run gating was
   not booted.
2. OTLP span export to the OTel collector was not observed (ran log-only degraded mode).

> **Merge-time action for the human:** boot `./ok.sh up` once on a docker host to close 0003's
> two gaps — confirm the migrate one-shot gates the `run` service and that spans reach the OTel
> collector. The semantics are already proven via the binary + live-Postgres fallback; this is
> the live-stack confirmation the sandbox could not perform.

Process learnings captured this cycle (all on `main`):

- **Net-new infra born with a new crate rides that crate's branch (carve-out to home #1).** This
  cycle deliberately put the `deploy/` stack + the `ok.sh` `up`/`run-server`/`migrate` verbs ON
  the branch — they are net-new and only meaningful because the `server` crate doesn't exist on
  `main` yet, and the verifier needs `./ok.sh up` to work inside the worktree. Landing them on
  `main` early would be an out-of-sync bug in the *other* direction (referencing a non-existent
  crate). This is distinct from the 0002 bug class (*modifying existing* shared infra, which
  stays `main`-only). Added as a narrow, explicitly-bounded carve-out to CLAUDE.md "The Board"
  home #1 with a decision test (when unsure → main-only).
- **A binary crate that will be integration-tested needs a `[lib]` target — scaffold it lib+bin
  from the start.** `tests/` links the crate's library, not its binary; the binary-only `server`
  crate couldn't expose `app::router`/`AppState`/config for in-process tests, blocking
  `./ok.sh test` until `server-dev` added a `[lib] name = "server"` + thin `src/lib.rs`
  (re-exporting the seams) with `main.rs` reduced to a CLI shell (`f67a883`). Recorded in
  `rust-standards` (the rule) and `new-crate` (the scaffold-time action); the `new-crate`
  reference example was also refreshed off the removed `organized-koala` placeholder onto
  `contract` (library) + `server` (lib+bin) as the live exemplars.
- **Docker-unavailable sandbox is a standing verifier limitation.** Every cycle shipping
  compose/OTel infra leaves the `service_completed_successfully` gating and OTLP-export sub-items
  verified-by-reading only; the sanctioned mitigation is the binary + live-Postgres fallback
  (proves semantics) plus the human booting the full stack once at merge. Recorded in the
  `verifier` agent so future verify passes apply it consistently.

Be aware:

- 0003 is **branch-owned** on `feature/0003-server-auth-profile-tasks`; the cycle advanced the
  branch copy of the item (status, reviewer/verifier verdicts, `## Summary`). `main`'s copy stays
  frozen at the claim snapshot (`ready`, with a pointer note) until the human's merge brings it
  back atomically with the code. No new crate dev agent — `server-dev` already owns
  `crates/server`.
- With 0003 heading to merge, **0004 (TUI) becomes unblocked** (it depends-on 0003). 0004 is the
  third and final slice of the foundational 0001 umbrella.

Docs updated (all on `main` — shared/cross-cutting, home #1): `docs/handoff.md` (this entry);
`CLAUDE.md` "The Board" home #1 (the net-new-infra carve-out); the `rust-standards` +
`new-crate` skills (the lib+bin rule + refreshed reference example); the `verifier`
agent (docker-unavailable fallback); `board/README.md` regenerated (home #3, derived). The 0003
item's `## Summary` was filled **on the branch** (home #2).

---

## Handoff — 2026-06-12 (0002 re-entry — human feedback: chrono timestamps + test-layout)

Two `[human]` feedback items on the already-verified, `awaiting-merge` 0002 re-opened the cycle.
`architect` triaged both; the cycle ran forward on `feature/0002-contract-crate` and stopped at
the AI-terminal `awaiting-merge` again. Both feedback boxes are now `[x]`.

What shipped (on the branch):

- **Feedback-1 (chrono):** contract timestamps are now `chrono::DateTime<Utc>`
  (`Task.created_at`/`closed_at`, `Profile.created_at`) instead of opaque strings — consumers
  get a typed timestamp and malformed dates now fail to parse. `chrono` added pure-DTO
  (`default-features = false, features = ["std","serde"]` — no clock/IO surface). **Wire bytes
  are unchanged** (RFC 3339 `…Z`, `closed_at: null` still emitted), so it sits **inside**
  ADR-0005's frozen wire format — **no wire change, no ADR.** Commits `bc61626` (contract),
  `98d1a85` (tests); reviewer approved `98d1a85`, verifier VERIFIED — 41 integration + 12
  doctests = 53 green.
- **Feedback-2 (test layout):** resolved as a **clarification, no code change**. The
  `contract` crate is pure-DTO — its whole surface is public — so the crate-root `tests/`
  public-API suite plus doctests is the correct, complete layout; there is no private logic for
  `module/tests.rs` to cover. Captured as a durable rule in `rust-standards` on `main`
  (`8b56ed2`).

Process point worth keeping (the durable learning of this re-entry):

- **A pure-Rust-representation change on an `awaiting-merge` item, with identical wire bytes,
  does NOT need an ADR.** ADR-0005 froze the *wire format*; it explicitly delegates the Rust
  representation (chrono vs string, enum-with-catch-all, etc.) to `contract-owner`. Swapping the
  in-crate type while the serialized bytes are byte-identical stays inside that delegation.
  **Contrast:** a change to the wire shape itself (a renamed/added/removed field, a changed
  encoding the other side observes) IS an ADR event and ripples to both consumers (CLAUDE.md
  hard-constraint #2). The reviewer guarded the boundary by holding the exact-byte assertions
  (`…Z` suffix, `closed_at: null` emitted) unweakened.
- The re-entry mechanics held: the **unchecked box was the only re-entry signal**;
  `architect` triaged to the smallest re-entry point (behaviour tweak, not a redesign); the
  owning agent checked the box `[x]` only after on-branch resolution + re-review. Zero blast
  radius because 0003/0004 are not built yet.

Be aware:

- 0002 remains **branch-owned** on `feature/0002-contract-crate`; the chrono delta advanced the
  branch copy of the item (status, re-review/re-verify verdicts, Summary) — `main`'s snapshot
  stays frozen at the claim until the human's merge. 0003 (server) is still `ready` and
  unblocked once 0002 merges; 0004 (TUI) follows 0003.
- No new crate dev agent — `contract-owner` still owns `crates/contract`.

Docs updated (all on `main` — shared/cross-cutting, home #1): `docs/handoff.md` (this entry);
`.claude/skills/rust-standards/SKILL.md` (the pure-DTO test-layout rule, `8b56ed2`);
`board/README.md` regenerated (home #3, derived). The 0002 item's `## Summary` was updated for
the chrono change **on the branch** (home #2).

---

## Handoff — 2026-06-11 (0002 — contract crate + workspace restructure)

Branch: `feature/0002-contract-crate` (head `638eef1`, last code `56833a6`, linear atop `main`
`ed9510e`, fast-forward — frozen for the human to merge). Slice 1 of 3 of the foundational
slice 0001.

What shipped:

- Removed the `crates/organized-koala` placeholder; the workspace now matches the target
  `contract`/(`server`)/(`tui`) layout. `crates/contract` authored as the single source of
  truth for the foundational wire shapes per ADR-0005.
- DTOs: `RegisterRequest`, `LoginRequest`, `SessionResponse`, `Profile`, `Task`, `TaskStatus`,
  `CreateTaskRequest`, `ErrorBody { code?, message }` + the 7 stable error codes with a lossless
  `Unknown` catch-all; a `Password` newtype (transparent serialize, `[REDACTED]` Debug).
- 37 serde/wire-format integration tests + 12 doctests green; build/lint/fmt clean. Reviewer
  approved at code head `56833a6` (re-attested after the rebase); verifier confirmed the
  pure-DTO seam (live-stack E2E deferred to 0003/0004 per ADR-0003).
- Planning artifacts (ADR-0005 + the 0002/0003/0004 plan) were committed to `main` as
  `1a2540c` before the worktree was finalized.

Process learnings captured this cycle (these will bite 0003/0004 if ignored):

- **State has three homes, by which side of the `main`↔branch line it belongs on.** This is THE
  process learning of the cycle, and it supersedes the earlier (wrong)
  "Board-authoritative-on-`main`, branches code-only" framing, which added a transcription step
  and still stranded cross-cutting state on the wrong side of the line — the root cause of BOTH
  out-of-sync incidents this cycle. The corrected model (now in CLAUDE.md "The Board"):
  1. **Shared / cross-cutting → `main` only, never on a feature branch.** ADRs + the decisions
     index, infrastructure (`ok.sh`, `.githooks/`, docker/compose, OTel config), `CLAUDE.md`,
     the standards skills, and `.claude/` agent/skill defs. A change to any of these riding a
     feature branch IS the out-of-sync bug class.
  2. **Feature-local → on the feature branch, in the worktree.** The
     `board/features/NNNN-<slug>.md` item travels with the code: status flips, per-slice Log,
     reviewer/verifier verdicts, and the `## Summary` are all committed on the branch. A clean
     revert is just dropping the worktree + branch; concurrent worktrees never contend on a
     shared Board file; a verdict on the branch is immutable evidence tied to its sha.
  3. **Derived → regenerated on `main`.** `board/README.md` from item frontmatter + branch heads.
  Lifecycle: born on `main` during planning, **branch-owned on claim** (the branch copy advances,
  `main`'s copy freezes at the claim snapshot until the human's merge brings it back atomically
  with the code). reviewer/verifier are **read-only on everything** (code AND Board) and report
  verdicts back; the orchestrator commits them on the branch. A Board-only commit does not
  trigger re-review — only a new code/test commit does. Codified in `drive`/`plan`/`review` and
  the `architect`/`reviewer`/`verifier` agents.
- **The secret-scan hook fix was relocated from the 0002 branch to `main`.** This cycle
  `platform-dev`'s `.githooks/secret-scan.sh` fix was wrongly committed on the 0002 feature
  branch, leaving `main`'s scanner stale — a textbook instance of cross-cutting state (home #1)
  riding a feature branch. It has been moved to `main`; the three-home rule above exists to
  prevent the recurrence.
- **Plan/ADR must be committed to `main` before the worktree is cut.** This cycle the ADR-0005
  artifacts were left uncommitted, the worktree was cut from the pre-ADR commit, and the code's
  `(see ADR-0005)` citations dangled — contract-owner flagged it as a blocker; recovered by
  committing to `main` and rebasing. Now a corollary of the three-home model (an ADR is home #1,
  and a worktree cut from a commit that lacks it cannot see it). Codified in `plan` + `drive`,
  the `architect` agent, and CLAUDE.md.
- **secret-scan matches credential VALUES, not bare identifiers** (now `d34570c` on `main`; the
  branch's original `37b78c4` was dropped when the fix was relocated): a bare Rust field
  declaration (keyword + bare type + comma, no separator/literal) no longer false-positives;
  assigned literals still trip. One known non-blocking gap recorded for future platform-dev (the
  JSON-object quoted-key/quoted-value form is not caught). Documented in `bash-standards`
  structurally (no matchable literals, so the doc does not trip its own scanner).
- **Markdown MD004:** a wrapped prose line starting with `+`/`*`/`-` is read as a list marker;
  reflow so a symbol is never line-leading. Documented in `docs-standards`.

Be aware:

- No new crate dev agent registered — `contract-owner` already owns `crates/contract`.
- 0002 is **in-flight and branch-owned** on `feature/0002-contract-crate`; its live status lives
  on the branch (where the cycle advanced it), and `main`'s snapshot is frozen at the claim until
  the human's merge. 0003 (server) is `ready` and unblocked (depends-on 0002); 0004 (TUI) is
  `ready` but depends-on 0003. 0001 is the umbrella (`planned`), tracking its three children.
- 0003 handles real credentials/JWTs — wrap secrets so they never reach `Debug`/`Display`/logs;
  do not rely on the secret-scan as the safety net.

Docs updated (all on `main` — shared/cross-cutting state, home #1): `docs/handoff.md` (this
entry, re-corrected to the three-home model); CLAUDE.md "The Board"; `docs/build-plan.md`;
`board/README.md` regenerated; the `plan`/`drive`/`review` skills; the
`architect`/`reviewer`/`verifier` agents; the `bash-standards`/`docs-standards` skills. The
secret-scan hook fix was relocated from the 0002 branch to `main`. The 0002 item's
`## Summary` + Log live on the branch (home #2).

---

## Handoff — 2026-06-10 (Bootstrap — workflow scaffold)

Branch: `main`.
Stood up the AI development workflow per BOOTSTRAP.md: the agent team, skills, Board, and docs
system for organized-koala. No application code yet — this cycle established *how* work runs,
not *what* it does.

What shipped:

- `CLAUDE.md` constitution (purpose, stack, `ok.sh` ops, 5 hard constraints, error contract,
  ambiguity policy, Definition of done, trigger tables).
- 9 agents in `.claude/agents/` (architect, contract-owner, server-dev, tui-dev, platform-dev,
  tester, reviewer, verifier, eng-manager); read-only roles omit Write/Edit.
- Skills in `.claude/skills/`: drive, plan, grill, review, coding-/rust-/docs-/bash-standards,
  repo-map, autowork, autoreview.
- `ok.sh` operations entrypoint; `.githooks/` pre-commit secret scan (hooksPath enabled).
- `docs/adr/0001-foundational-architecture.md` + decisions index; this handoff; build-plan.
- `board/` with the dashboard and feature `0001` (foundational vertical slice) in `inbox`.

Be aware:

- `.claude/settings.json` (the permission allowlist) was **not** written by the bootstrap — the
  harness auto-mode classifier blocks an agent authoring permission rules. The human must add it
  (content is in the bootstrap conversation / README of this cycle).
- The `crates/organized-koala` placeholder still exists; feature 0001 restructures it into
  `contract` / `server` / `tui`.
- ADR-0002 (timer authority) is pending and gates Pomodoro work.

Docs updated: ADR-0001 created; CLAUDE.md authored.

---

### What works right now

- The **workflow** is in place: run `/drive` to advance the Board one item to `awaiting-merge`.
- **The `contract` crate is merged on `main`** (0002): a compile-only, pure-DTO seam carrying
  the foundational wire shapes (auth/profile/task DTOs, `ErrorBody`, error codes, the redacting
  `Password` newtype) per ADR-0005, with `chrono::DateTime<Utc>` timestamps (wire bytes
  unchanged — RFC 3339 `…Z`). The workspace matches the target layout (placeholder crate gone).
- **The server is merged on `main`** (0003): `organized-koalad` implements the full ADR-0005
  HTTP API against Postgres — argon2 + JWT auth, the atomically-created default profile,
  profile-scoped add/list/close tasks, the `{ code?, message }` error contract, the ADR-0004
  `run`/`migrate`/`rollback` CLI, reversible migrations, `tracing`/OTLP instrumentation, and the
  `deploy/` docker stack (compose `server` healthcheck on `/healthz`). Merged after a four-item
  human-feedback re-entry; reviewed + live-verified under the sanctioned docker mechanism.
- **The TUI is merged on `main`** (0004): `organized-koala` (ratatui/crossterm/reqwest) completes
  the loop — register/login (auto-selecting the single default profile), task list (newest-first,
  done/undone markers, add Title+Description, mark-done), ADR-0005 error-code branching
  (`unauthenticated`→login, `validation_failed`→inline, offline→blocking+retry), and statelessness
  (JWT + active profile id in process memory only). Built as a pure core (update fn + draw fns +
  `map_key`) behind an injected `Client` trait, so the whole interactive surface is `TestBackend`-
  tested (ADR-0003 layer 2). Reviewed + live-verified over the full reqwest path.
- **The foundational slice 0001 is CLOSED.** With 0002/0003/0004 all on `main`, the umbrella
  0001 merged too — the end-to-end tracer bullet TUI ↔ contract ↔ server ↔ Postgres is complete.
- **The TUI responsive event loop is MERGED on `main`** (0005): the TUI no longer freezes
  during an HTTP request — it keeps rendering, animates a spinner with a "working… (Esc to
  cancel)" hint, and stays interactive in flight. Per [ADR-0006][adr-0006] Model A: a synchronous
  `Client` on a worker thread, `std::sync::mpsc` request/response, a polled (`event::poll`) render
  loop — **no async runtime**. The `App` core is now client-free with two pure seams
  (`handle_event`/`apply_response`); one request in flight at a time (transient `pending:
  Option<RequestId>`), cancel is user-perceived (stale-`RequestId` response dropped). `tui::app`
  was reorganized into `auth`/`task_add`/`task_list` submodules + `protocol.rs`. TUI-only —
  `contract`/`server` unchanged. Reviewed + live-verified (code-hash
  `bc89672d4be5cdecd0bb54b340a24a5b8741cf21`); fast-forwarded to `main` at `6f9a80a`, worktree +
  branch removed.
- **The `chore` Board item type now exists** (governance, learned-0005 follow-up): a lightweight
  lane for scope-limited maintenance (refactors, doc fixes, test-only, dep bumps) with no
  behaviour/`contract`/domain change — orchestrator-mintable, on a lighter DoD (gates + an
  invariant-attesting cold review; live verifier skipped). See CLAUDE.md "Definition of done" +
  "The Board". **First trip through the pipeline complete and MERGED — `0006`** (the
  `tui/src/main.rs` stale-doc-comment fix) ran mint → claim → build → invariant-attesting cold
  review → verify skipped → `awaiting-merge`, then fast-forwarded to `main` (code-hash
  `401ad3de59c4cc7e33c3ebf8308c171d80659e4e`); the chore lane needed zero process correction.
- **The account-global Pomodoro focus timer is MERGED on `main`** (0008, the
  first Focus-phase feature; live-verified): a new `contract` `timer`
  module (`TimerConfig`, `UpdateTimerConfigRequest`, the tagged `TimerSession` enum carrying
  `ends_at` + `server_now`), five account-global `/api/timer/...` server endpoints keyed on
  `user_id` (config get/update, session get/start/stop) with a reversible migration creating
  `timer_configs` + `timer_sessions` (`ends_at` derived, not stored), and a TUI presentation whose
  live `MM:SS` countdown is **render-only** — recomputed each ~80 ms draw from the server's
  absolute `ends_at` + `server_now` + a monotonic `Instant`, never a stored counter (#1-safe;
  inside [ADR-0006][adr-0006], no per-second polling). Account-global (#4 / ADR-0002 §5), flat (#3,
  duration the only knob); the contract/domain surface carries no new/amended ADR
  ([ADR-0002][adr-0002] governs). **After the 0008-R1 feedback re-entry (TUI-only):** the timer is
  an **always-visible global widget** in the bottom-right of every post-auth screen (no dedicated
  page), toggled by a global **`p`** (start/stop) that is listed in the bottom-left help caption;
  the in-flight indicator **appends a trailing spinner** to the stable caption instead of replacing
  it (flicker fix), and the coarse session refresh loosened ~5 s → ~1 min — all governed by the
  [ADR-0006][adr-0006] **§8 amendment** (TUI presentation only; ADR-0002 authority/render model
  unchanged; no `contract`/server/migration change). Reviewed **approved** and live-**verified** at
  the 0008-R1 end state, both pinned to code-hash `3fa0adefce8cd6d67ae716dae7a24ce6dbf9defd` on
  `feature/0008-pomodoro-timer` (the original 0008 build was approved + verified at
  `708ee8d0085ce9b3af68eb7e1b76dbe56a6185da`, voided when the re-entry moved the tree).
  Fast-forwarded to `main` at `c32f0ad`; worktree + branch removed.
- **The report-only `./ok.sh coverage` verb is MERGED on `main`** (0007, a
  `chore`): `cargo llvm-cov --workspace --summary-only`, reusing
  `cmd_test`'s live-DB wiring (throwaway test Postgres booted + torn down on a `RETURN` trap), in
  the no-arg usage/help. **No threshold, not a DoD gate** — purely reported (operator-sanctioned
  shape: coverage visible, not a brittle bar). Baseline at implementation: ~66% line / ~66%
  function / ~61% region. Tooling-only (no crate source/behaviour/`contract`/domain change), so it
  ran the lighter chore DoD: gates green + a cold reviewer **approved** attesting the chore
  invariant, pinned to code-hash `3fa0adefce8cd6d67ae716dae7a24ce6dbf9defd` on
  `feature/0007-ok-coverage-verb`; the live verifier pass was correctly **skipped**. The 0003
  "sanctioned follow-up" is now consumed. Fast-forwarded to `main` at `6860b28`; worktree + branch
  removed.
- **Coverage is now captured in the cycle and recorded in each item's Summary** (0009, a
  `main`-only governance `chore`, at `awaiting-merge` on `main` after this step): `drive` step 6
  runs `./ok.sh coverage`, parses the headline workspace coverage %, and writes a `coverage: NN.N%`
  line (or `coverage: unavailable (docker)`) into the item's `## Summary` on **every** cycle
  (feature and chore). **Report-only — never a gate** (no threshold, not a DoD clause, never blocks
  `awaiting-merge`); consistent with the "How to run" `coverage` row and 0007. Three governance
  edits (drive SKILL, CLAUDE.md DoD note, eng-manager charter), applied directly on `main` with **no
  worktree**; cold reviewer **approved** with the chore invariant attested (code-hash
  `3fa0adefce8cd6d67ae716dae7a24ce6dbf9defd`), live verifier **skipped**. 0009's own Summary is the
  first to carry a coverage line: 66.36% line / 61.48% region / 66.67% function.
- **Notes — the final domain feature — is `merged` on `main`** (0010, a
  `feature`, live-verified; the operator performed the final merge): the last missing flat feature,
  a near-exact structural clone of the
  task surface governed by [ADR-0007][adr-0007]. A new `contract` `note` module
  (`Note { id, title, content, created_at }`, `CreateNoteRequest`, `UpdateNoteRequest`, no new
  `ErrorCode`, no `updated_at` — flat #3); five profile-scoped server CRUD routes under
  `/api/profiles/{id}/notes` (create 201 / list 200 newest-first / get 200 / update 200 in-place /
  delete 204), every query ownership-joined so an unowned/missing profile or note id is `404`
  (never 403, #4), with a reversible migration `20260612163049_notes` (`ON DELETE CASCADE`,
  `(profile_id, created_at DESC)` index); and a TUI `Screen::Notes` view (list + create/edit/delete)
  opened by `n` from the task list, stateless (#1), reqwest client maps one-for-one to the wire.
  Tests in all three crates (`contract` 11, `server` 28, `tui` `TestBackend` 13). Reviewer
  **approved** + verifier **verified**, both pinned to code-hash
  `46c1c60f1eb3865eb127a72502982827ebb09d65`; coverage 68.24% line. With Notes merged, all four flat
  features (TODO, Pomodoro, Notes, Profiles) exist except Profiles CRUD (0012, still `ready`).
- **Task update/delete/reopen is MERGED on `main`** (0011, a `feature`, live-verified): the
  one-way task `close` generalized into full edit / toggle-done / reopen / delete — a
  **breaking** change ([ADR-0008][adr-0008-0011]) that **removes** the
  `POST .../tasks/{id}/close` route (clean removal, single in-repo consumer, ADR-0005 §8). A new
  `contract` `UpdateTaskRequest { title?, description?, status? }` (all-optional partial, no
  `updated_at`, #3); `PATCH …/tasks/{id}` via one static `UPDATE … RETURNING` (`COALESCE`/`CASE`:
  done→`closed_at` set, open→cleared, empty patch a 200 no-op, blank title → 400) + `DELETE …/tasks/{id}`
  (204 / 404), both ownership-joined → 404 never 403 (#4), **no migration**; the TUI gains edit/
  toggle/delete keys (`e`/`c`/`x` with two-step confirm), stateless (#1). It was re-rebased onto
  post-0010 `main`, which pulled the merged Notes feature into its `crates/` tree, **changing its
  code-hash** to `ee5047c9abf1e4196ed1933655a61fcf41148bcb` and forcing a re-review/re-verify (both
  re-passed); an operator-authorized doc-only README fix then moved the hash to
  `97cbc025523bdff1907e9552fd3636d3a874b589` (verdicts carried forward by authorization).
  Fast-forwarded to `main` at `9635608`; worktree + branch removed.
- **Profiles create/update/delete + TUI switcher is MERGED on `main`**
  (0012, a `feature`, live-verified — **the final
  domain feature; organized-koala is now functionally complete**): the only profile surface was
  list + register-time bootstrap; 0012 adds `POST /api/profiles` (201), `PATCH /api/profiles/{id}`
  (200), `DELETE /api/profiles/{id}` (204) under [ADR-0009][adr-0009], plus a client-side TUI
  switcher. New `contract` `CreateProfileRequest`/`UpdateProfileRequest` + two **append-only** error
  codes `ProfileNameTaken`/`LastProfile`. Server: race-safe DB unique-violation → `409
  profile_name_taken` (no TOCTOU); atomic last-profile guard → `409 last_profile` (account keeps ≥1
  namespace); delete **cascades** the profile's tasks **and** notes via FK `ON DELETE CASCADE` (#4);
  reversible `UNIQUE (user_id, name)` migration ordered after 0010. TUI `Screen::Profiles` switcher
  (`s`; `a`/`e`/`x` create/rename/delete): **switch is client-side only** — rebinds the in-memory
  `active_profile_id`, no server endpoint, no persistence (#1); deleting the active profile
  re-points to the first remaining. Reviewer **approved** + verifier **verified** (live cascade
  DB-confirmed `tasks=0, notes=0, profile=0` + 404), both pinned to code-hash
  `71fb7ecf327fbd42a14cb19456207885c782fe49`; coverage 66.91% line. The cycle's load-bearing
  learning — `./ok.sh prepare` is now self-contained (`3e0094b` on `main`), completing the
  "every DB-needing `ok.sh` verb self-boots the shared test PG" pattern (`test`/`coverage`/`prepare`).
  Fast-forwarded to `main` at `685b4de`; worktree + branch removed. The reviewer's pre-existing
  `Session.token` JWT-`Debug`-leak nit was promoted to **0013** (high `chore`).
- **The session JWT `Debug` leak in the `tui` is at `awaiting-merge` on
  `feature/0013-session-token-debug-leak`** (0013, a high `chore`, cold-review-approved; live
  verifier correctly skipped): the bearer JWT, previously a bare `String` reachable from a derived
  `Debug` on `Session` + all 17 `ClientRequest::*` variants + `Outcome::ListProfiles`, is now held in
  a `SessionToken(String)` newtype (`crates/tui/src/app/token.rs`) with a hand-written `Debug` →
  `[REDACTED]` and an `expose()` accessor used only at the point the `Authorization: Bearer` header
  is attached. Mirrors the in-repo `contract::Password` template (chosen over `secrecy` to avoid a new
  dependency for one field); the wire bearer string is byte-identical. `tui`-only — no `contract`/wire
  (#2), no domain (#3), no behaviour change beyond `Debug` rendering. Reviewer **approved** with the
  chore invariant attested (code-hash `e5925c5139e52846d8593c4be3ab2d0516d49fa0`); coverage 66.90%
  line. This cycle sharpened `rust-standards` with a callout on the
  `missing_debug_implementations`-lint-vs-secret-redaction tension (the root cause that let this leak
  survive from 0004 through 0011 under diff-scoped cold review). Merged on `main`.
- **The TUI layout shell is MERGED on `main`** (0014, Phase 1 of the three-part TUI overhaul, a
  `feature`, live-verified): a **`tui`-crate-only** reshape of the structural shell with **no**
  `contract`/server/domain change ([ADR-0010][adr-0010-0014-snap] §5 boundary). `Screen::TaskList`/
  `Notes`/`Profiles` collapsed into one `Screen::Main(Box<MainState>)` holding the active
  `Tab{Tasks,Notes,Profiles}` + all three live panes (new `crates/tui/src/app/main_view.rs`);
  `Tab`/`Shift+Tab` cycle tabs (arrows move list selection), each switch re-derives the pane from a
  fresh server load for the active profile (#1, #4) preserving the row; removed `n`/`s`/idle-`Esc`-back
  and the old cross-screen events; `t` left unbound for 0016. `Session`/`AuthState` gained a
  client-captured `account: String` (no new wire); centred bounded auth form, centred verbatim title
  `organized koala - <user> @ [<profile>]`, footer flushed to the bottom. Reviewer **approved** +
  verifier **verified**, both pinned to code-hash `bf65aa9612bf1633bf75e64f66a3dfddcfb4aa10`; coverage
  72.96% line. ADR-0010 binds 0015/0016. Fast-forward merged into `main`.
- **The TUI dialog system is at `awaiting-merge` on `feature/0015-tui-dialog-system`**
  (0015, Phase 2 of the TUI overhaul, a `feature`, live-verified): a **`tui`-crate-only** modal
  framework with **no** `contract`/server/domain change ([ADR-0010][adr-0010-0014-snap] §5 boundary,
  confirmed byte-identical). A deep `draw_dialog` helper (one `Dialog` fed by all six dialog kinds +
  the help overlay) floats centred over the tabbed view via `Clear` + `centered_rect`; task/note/
  profile add+edit+delete-confirm and the timer duration edit all moved off the 2-row message band
  into dialogs (state machines/error routing untouched — `last_profile` refusal preserved); a `?`
  help modal (transient `App.help_open`, `Event::ToggleHelp`) lists the full hotkey reference and the
  three long `*_CAPTION` constants collapse into one short `FOOTER_CAPTION`; `draw_field` renders a
  focused field's border in `Color::Magenta` (auth + all dialog fields). A single
  `App::overlay_capturing_input()` predicate unifies the scattered text-entry/sub-flow gates: globals
  (`q`/`r`/`?`/`p`/`d`/tab-switch) suppressed while any overlay captures input, two-tiered `Esc`
  (cancels an open overlay, still quits idle post-auth, still cancels in-flight). A tester-flagged
  fix-now made `?` close the help overlay end-to-end (distinct `help_open` param in the 5-arg
  `map_key`) so the advertised `?/Esc: close` affordance works. Tests `tests/dialogs.rs` 21/0 + 380
  total pass; reviewer **approved** + verifier **VERIFIED**, both pinned to code-hash
  `b9884943f36f3ac6c9d56fd2be46e31057a9060a`; coverage 73.80% line. Awaiting the human's merge; 0016
  is unblocked once 0015 merges.

[adr-0010-0014-snap]: ./adr/0010-tui-navigation-and-interaction-model.md
