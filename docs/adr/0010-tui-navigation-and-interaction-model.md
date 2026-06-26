# ADR-0010: TUI navigation and interaction model (tabs, dialogs, detail views)

**Status:** Accepted · 2026-06-26

## Context

The TUI overhaul Board items [0014][feat-0014] → [0015][feat-0015] → [0016][feat-0016]
reshape the `organized-koala` client's whole **interaction model** over three sequential
phases:

- **0014 (shell)** replaces the three-separate-screens navigation (`Screen::TaskList` /
  `Notes` / `Profiles`, reached with `n`/`s`/`Esc`) with a **single post-auth view carrying a
  `Tasks | Notes | Profiles` tab bar**, cycled by `Tab` / `Shift+Tab`; re-centres the auth
  form and title; tightens the footer.
- **0015 (dialogs)** introduces a **reusable modal/overlay framework** and moves every
  add/delete/timer-config sub-flow off the inline "message band" into centred dialogs, adds a
  `?` help modal + trimmed footer caption, and a purple focus border.
- **0016 (detail views + keymap)** adds **per-field task/note detail views** (each field a
  pane, `Tab`-cycled inside the view, `e`/`Enter`/`Esc` edit lifecycle) and locks in the
  **final hotkey scheme** (`Space` done, `d` delete, `t`/`T` timer, `a`/`e`/`Enter`
  context-scoped to the active tab).

Today the `tui` crate models navigation as four mutually-exclusive `Screen` variants advanced
by a transport-agnostic `Event` enum, with the keymap pinned by the pure `map_key`
(`crates/tui/src/terminal/mod.rs`) and the pure draw functions in `crates/tui/src/ui/mod.rs`
(ADR-0003 layer-2 seam; ADR-0006 two-step update core). The arc does not just restyle that
surface — it **reshapes the `Screen`/`Event` state-machine contract**: the three list screens
must coexist behind one tabbed view with per-tab selection preserved (0014); a modal layer
must overlay any tab and suppress global hotkeys while it captures input (0015); a detail view
must overload `Tab`/`Esc` context-sensitively (0016). These are the same class of
cross-cutting client-architecture decisions that **ADR-0006 recorded for the concurrency
model** — decided once, before code, so all three phases inherit consistent invariants rather
than re-deriving them per branch.

**This ADR changes no wire shape and no server.** Like ADR-0003 and ADR-0006, it is a
`tui`-crate-only decision: it touches neither the `contract` crate, any endpoint, the server,
nor the domain shape (hard-constraints #2/#3). Every tab, dialog, and detail-view pane renders
**existing** DTOs (`Task`/`Note`/`Profile`/`TimerConfig`/`TimerSession`) over the **existing**
client methods; no new field, no per-profile timer config, no new route. The feature requests'
"presentation-only, no contract/server/domain change" assertion is therefore **confirmed**,
and this ADR makes that boundary **binding** for all three phases (see §5).

### Forces

- **Flatness / simplicity** (coding-standards priority order). The smallest model that is
  correct wins; new state machinery must earn its place. The tabbed view should reuse the
  existing per-tab list states, not introduce a parallel store (hard-constraint #1).
- **The ADR-0003 layer-2 testability seam.** The whole interactive surface is driven through
  `ratatui`'s `TestBackend` with no live server and no real terminal, because the pure `App`
  state machine reaches its one external service through the injected synchronous `Client`
  trait, and `map_key` / `handle_event` / the draw fns are pure. Every change here **must keep
  this seam**: tabs, modals, and detail views are pure state + pure render, drivable by the
  `common` harness with the fake `Client` as the only mock.
- **The ADR-0006 two-step update core.** `handle_event(Event) -> Option<Dispatch>` and
  `apply_response(ClientResponse) -> Option<Dispatch>` stay the seam; new interactions add
  `Event` variants and reshape `Screen`, but no interaction performs I/O inline.
- **Hard-constraint #1 — the TUI is stateless.** Per-tab list selection, the open modal, and a
  detail-view edit buffer are **transient process-lifetime UI state** (same category as the
  in-memory JWT and the current `AuthState`/`AddTaskState` buffers) — never persisted, every
  list still derived from a server response. The tab the user is on is a *view selector*, not
  cached server data.
- **Profiles are namespaces (#4).** A tab switch to a list re-derives that list for the active
  profile from the server (or from already-loaded in-memory state for the active profile); no
  tab ever shows another profile's data.
- **The error-code branching contract** (ADR-0005 codes; the `handle_*_error` routing). Tabs,
  dialogs, and detail-view commits route their responses through the **same** branching,
  unchanged: `unauthenticated` → login, `validation_failed` → inline (now in the dialog/pane),
  offline → blocking retry screen.

## Decision

### 1. One post-auth tabbed view; the three lists become tab panes (0014)

The post-auth surface is a **single view with a tab bar** (`Tasks | Notes | Profiles`),
**Tasks selected by default**. The three existing list states (`TaskListState`,
`NotesState`, `ProfilesState`) become the **panes of that view**, not mutually-exclusive
`Screen` variants. The concrete `Screen`/`App` reshape (whether a single
`Screen::Main { active_tab, … }` variant holding all three pane states, or an equivalent
active-tab discriminant) is a
**`tui-dev` implementation choice** bounded by these invariants — it is not pinned here:

- **Tab switching is `Tab` / `Shift+Tab` only**, cycling `Tasks → Notes → Profiles → Tasks`
  (and reverse). There are **no `t`/`n`/`p`/`s` tab-letter hotkeys** (this also keeps `t` free
  for the timer in 0016). The old `n` (open notes) / `s` (open profiles) / `Esc` (back)
  cross-screen navigation is removed.
- **Per-tab list selection is preserved across switches** — leaving and returning to a tab
  restores its selected row.
- **Within a list, arrows move the selection** (`Tab` is now owned by tab-switching, not list
  navigation).
- A tab switch keeps the TUI stateless: the destination pane shows data **derived from a
  server response** for the active profile (#1, #4) — either an already-loaded in-memory list
  for the active profile or a fresh load.

### 2. Auth form, title, footer (0014, presentation)

- The auth (login/register) screen renders as a **small centred box** (bounded, centred both
  axes), preserving the Login⇄Register toggle, all current fields, and the inline error band.
- The post-auth title is **centred** and reads exactly `organized koala - <user> @ [<profile>]`
  (literal square brackets), `<user>` = the logged-in account identifier, `<profile>` = the
  active profile name. The auth screen keeps a simple centred title.
- The footer (caption + timer) **hugs the bottom row** (the large `BOTTOM_BAND_ROWS` bottom
  margin is removed; the band is pulled flush to the last row).

> **Account identifier for the title.** The post-auth title needs the logged-in account
> identifier (`<user>`). The in-memory `Session` today holds the token + active profile but
> **not** the account identifier. Sourcing `<user>` is a `tui-dev` choice **bounded to
> client-side, no-new-wire**: capture the identifier the user typed on the login/register form
> into the in-memory `Session` at auth time. **It must not require a new endpoint or a new DTO
> field** — if the only correct way to obtain it were a contract/wire change, that is an ADR
> event and the work stops to amend this ADR (it is not; the identifier is already entered
> locally at auth).

### 3. Modal/overlay framework + global-hotkey suppression (0015)

A **reusable centred floating modal** overlays the active tabbed view. The binding invariants
(detailed design is 0015's plan):

- **While a modal/dialog (or a detail-view field edit, §4) is capturing input, global hotkeys
  are suppressed** — typing a field never fires `q`/`t`/`r`/`?`/tab-switch. This generalises
  today's `is_text_entry` gate to a single "input-capturing overlay" rule.
- **`Esc` closes/cancels the active modal** (and, in a detail view, cancels an in-progress
  edit — §4); a two-tiered `Esc` is the rule, never a hard quit while an overlay is open.
- Add (task/note/profile), delete-confirm (task/note/profile), and timer-config sub-flows move
  **from the inline message band into modals**. The footer caption trims to essentials
  (movement, tab-switch, `q`, `?`, the in-flight spinner); the full reference lives in the `?`
  help modal. Focused fields show a **purple border** (replacing the bold-border cue).

### 4. Detail views + final keymap (0016)

- `Enter` on a selected task/note opens a **detail view** whose fields are individual panes;
  inside it **`Tab`/`Shift+Tab` cycle panes** (not top-level tabs), `e` enters edit on the
  focused pane, `Enter` commits that field, and **`Esc` is two-tiered** (cancels an in-progress
  field edit, else exits the detail view to the list). The focused pane shows the purple focus
  border.
- The **final hotkey scheme** (the table in 0016) is canonical: per-entity action keys
  (`a`/`e`/`Enter`/`Space`/`d`) are **context-scoped to the active tab**; global keys
  (`t`/`T`/`r`/`q`/`?`/arrows/`Tab`/`Shift+Tab`) work anywhere an overlay/edit is **not**
  capturing input. The remap is: `c`→`Space` (done), `x`→`d` (delete), `p`→`t` (timer
  start/stop), duration-edit→`T`.

### 5. Binding scope boundary — presentation only, across all three phases

This is the load-bearing decision. For the entire 0014–0016 arc:

- **No `contract`/wire change (#2).** Every tab/dialog/detail pane renders existing DTOs over
  existing client methods. Detail views expose **only existing fields**
  (task: Title/Description/Status/Created/Closed; note: Title/Content/Created).
- **No server change.** No new route, no changed response shape, no changed status code.
- **No new domain structure (#3).** No subtasks/tags/categories, no per-profile timer config,
  no profile detail view. Profiles keep switch/add/rename/delete only.
- **Profiles stay namespaces (#4)** and the **stateless** invariant (#1) holds — every list is
  server-derived; tab/modal/detail state is transient process-lifetime UI state.

If any phase discovers it **cannot** meet its acceptance without crossing one of these
boundaries (e.g. the detail view needs a field the DTO does not carry, or `<user>` cannot be
obtained client-side), that is an **ADR event**: the work stops, this ADR is amended (or a new
one written) before re-implementation — it is never engineered around on a feature branch.

## Consequences

- **Positive.** The three phases share one decided interaction contract: tab semantics, the
  global-suppression rule, the two-tiered `Esc`, and the canonical keymap are settled once, so
  0015/0016 plans inherit them rather than re-deriving (and a reviewer checks each phase against
  this ADR). The ADR-0003/0006 testability seam is explicitly preserved, so the whole reshaped
  surface stays `TestBackend`-drivable with the fake `Client` as the only mock. The
  presentation-only boundary is now **enforceable** — a reviewer can block any wire/server/domain
  creep against §5.
- **Negative / cost.** The `Screen` reshape (three list screens → one tabbed view) is a
  non-trivial internal refactor that touches `app/mod.rs`, `terminal/map_key`, `ui/mod.rs`, and
  every `tui` test that constructs a `Screen` directly (the `common` builders + suites). It is
  contained to the `tui` crate and carries no wire risk, but the blast radius inside the crate
  is real and is sequenced across three Board items to keep each reviewable.
- **Risk.** The keymap remap (0016) changes muscle-memory keys (`c`→`Space`, `x`→`d`,
  `p`→`t`); this is the intended product change, pinned by the `map_key` keybinding tests so no
  binding silently regresses.
- **Reversibility.** Entirely reversible by `tui`-crate revert; nothing leaves the client, and
  `main` carries no wire/server change to unwind.

[feat-0014]: ../../board/features/0014-tui-layout-shell.md
[feat-0015]: ../../board/features/0015-tui-dialog-system.md
[feat-0016]: ../../board/features/0016-tui-detail-views-and-hotkeys.md
