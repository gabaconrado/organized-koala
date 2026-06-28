---
id: 0016
title: TUI detail views + final hotkey scheme — per-field task/note panes, full keymap
type: feature      # feature | chore
status: working         # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
priority: medium    # high | medium | low
parent: null
depends-on: [0015]
branch: feature/0016-tui-detail-views-and-hotkeys
worktree: .claude/worktrees/0016-tui-detail-views-and-hotkeys
created: 2026-06-26
updated: 2026-06-28
---

## Feature request

**Goal:** Phase 3 (final) of the three-part TUI overhaul (0014 → 0015 → **0016**). Add
**task/note detail views** with one pane per field (each editable in place) and lock in the
**complete hotkey scheme**. Builds on the tab shell (0014) and dialog framework (0015).

**Context (current behaviour to change):**

- **Tasks have no detail view**; the description is entered on add/edit but never displayed.
  **Notes** have a read-only `Viewing` detail (`app/notes.rs`, `crates/tui/src/ui/mod.rs`) —
  this becomes an **editable per-field** view.
- The keymap predates this overhaul (`map_key`, `crates/tui/src/terminal/mod.rs`): `c` toggles
  done, `x` deletes, `p` toggles timer, `d` edits duration, etc. This phase replaces it with
  the scheme below.

**Surface to build (TUI only — no `contract`/server change):**

- **Detail views with per-field panes (point 9, points 5.1/5.3).** Selecting a task or note
  with **`Enter`** opens a **detail view** whose individual fields are each their own **pane**
  (task: Title, Description — plus read-only Status / Created / Closed as appropriate; note:
  Title, Content — plus read-only Created):
  - Panes are **cycled with `Tab` / `Shift+Tab`** (inside a detail view, Tab cycles *panes*,
    not top-level tabs).
  - **Edit lifecycle:** pressing **`e`** enters edit mode on the focused pane; **`Enter`
    commits the change to that field**; **`Esc` cancels the edit** (reverts the pane to its
    pre-edit value, staying in the detail view).
  - **`Esc` is two-tiered:** while a field edit is in progress it cancels *that edit* (per
    above); while no field is being edited it **exits the detail view back to the list**.
  - The focused pane shows the **purple focus border** from 0015.
- **Final hotkey scheme (point 5).** Replace the current keymap with the table below. Per-entity
  action keys (`a`/`e`/`d`/`Enter`/`Space`) are **context-scoped to the active tab**; global
  keys work anywhere a dialog/edit field is **not** capturing input.

  | Scope | Key | Action |
  | --- | --- | --- |
  | **Tasks tab** | `a` | Add task (dialog, 0015) |
  | | `e` | Edit **title only** (list); in the task detail view, enter edit on the focused pane |
  | | `Enter` | Open task detail view; **when editing a pane, commit that field** |
  | | `Space` | Mark done / undone (**replaces the old `c` toggle**) |
  | | `d` | Delete task (confirm dialog, 0015) |
  | **Notes tab** | `a` | Add note (dialog) |
  | | `e` | Edit **title only** (list); in the note detail view, enter edit on the focused pane |
  | | `Enter` | Open note detail view; **when editing a pane, commit that field** |
  | | `d` | Delete note (confirm dialog) |
  | **Profiles tab** | `Enter` | Switch active profile |
  | | `a` | Add profile (dialog) |
  | | `e` | Edit (rename) profile |
  | | `d` | Delete profile (confirm dialog; keeps 0012 last-profile guard) |
  | **Global** | `T` | Configure timer (dialog, 0015) |
  | | `t` | Start / stop timer |
  | | `r` | Refresh |
  | | `q` | Quit |
  | | arrows | Move (list selection) |
  | | `?` | Help modal (0015) |
  | | `Tab` | Next field/pane (in a dialog/detail view) **or** next top-level tab (in a list) |
  | | `Shift+Tab` | Previous field/pane, or previous top-level tab (in a list) |

  Notes on the remap: `c` (done) → **`Space`**; old delete `x` → **`d`**; old timer toggle `p`
  → **`t`**; old duration-edit `d` → **`T`** (configure). `Esc` closes a dialog (0015); in a
  detail view it cancels an in-progress field edit, else exits the view to the list (two-tiered,
  per the edit lifecycle above).

**Acceptance criteria:**

- [ ] `Enter` on a selected task opens a detail view with per-field panes; `Tab`/`Shift+Tab`
      cycle the panes; `e` enters edit on the focused pane; `Enter` commits that field; `Esc`
      cancels an in-progress edit (reverting the value), and `Esc` with no edit in progress
      returns to the list.
- [ ] `Enter` on a selected note opens the equivalent editable detail view (Title + Content
      editable; Created read-only).
- [ ] The focused pane in a detail view shows the purple focus border (0015).
- [ ] All keys behave per the table: `Space` toggles task done/undone (no `c`); `d` deletes
      (no `x`); `t` starts/stops the timer and `T` opens the timer-config dialog (no `p`/`d`
      duration); `a`/`e`/`Enter` per the active tab; `r`/`q`/`?`/arrows/Tab/Shift+Tab global.
- [ ] Per-entity action keys are scoped to the active tab and never fire while a dialog or an
      edit field is capturing input (global-suppression rule from 0015 holds).
- [ ] Full `feature` Definition of done: `./ok.sh test | lint | fmt --check` green; `reviewer`
      approved (pinned to `./ok.sh code-hash`); the detail-view + keymap behaviour covered by the
      `TestBackend` suite ([ADR-0003][adr-0003]); `verifier` confirms that suite is green and
      boots the stack to confirm reqwest paths still function (no server/contract delta).

**Out of scope (would need an ADR — #3):** new fields on tasks/notes beyond today's flat shape
(the detail view only exposes existing fields — Title/Description/Status/Created/Closed for
tasks, Title/Content/Created for notes); per-profile timer config; any profile detail view
(profiles keep switch/add/rename/delete only). No `contract`/server change.

## Plan(s)

> **Planned by `architect` 2026-06-28.** Phase 3 (final) of the TUI overhaul
> (0014 shell → 0015 dialogs → **0016**). The full interaction contract for this phase —
> per-field detail views, the two-tiered `Esc`, and the canonical hotkey remap — is **already
> decided in [ADR-0010][adr-0010] §4–§5**; 0016 *implements* it and **cites** it. **No new ADR
> is required:** ADR-0010 §5 binds the whole arc to presentation-only (no `contract`/wire (#2),
> no server, no domain (#3)), and this phase exposes **only existing DTO fields**
> (`Task`: id/title/description/status/created_at/closed_at; `Note`: id/title/content/created_at)
> over **existing** client methods (`UpdateTask`/`UpdateNote`/`GetNote`). Confirmed against the
> DTOs: `UpdateTaskRequest` carries `Option<…>` fields (a per-field task commit sends only the
> changed field), and `UpdateNoteRequest` carries `title`+`content` (a per-field note commit
> re-sends the unchanged field from current pane state) — both satisfiable on the existing wire.

### Scope and boundary (binding)

- **TUI crate only.** `crates/tui/**` and its tests. No `crates/contract/**`, no
  `crates/server/**`. If implementation surfaces a genuine need for a new field, route, or
  response-shape change, that is an **ADR-0010 §5 event**: STOP, set the item `blocked`, and
  bounce to `architect` to amend ADR-0010 — never engineer it around on the branch.
- **Inherits the 0015 framework:** the unified `overlay_capturing_input` global-suppression
  rule, the two-tiered `Esc` (already live for dialogs), the centred `Dialog`/`draw_field`
  purple-focus-border helper, and the `?` help modal. 0016 extends these to a **new in-`Main`
  sub-mode** (the detail view), it does not rebuild them.

### Design shape (the implementation contract for `tui-dev`)

The detail view is a **new transient sub-mode of `Screen::Main`**, not a new `Screen` variant
(stays within the tabbed view; the tab bar/title/footer keep rendering). Model it per-tab,
mirroring the proven `NotesMode` state-machine pattern. Recommended concrete shape (a
`tui-dev` choice, bounded by the invariants below):

- A `DetailView` sub-state on the Tasks and Notes panes (e.g. `tasks.detail: Option<TaskDetail>`,
  `notes.mode` gains/replaces `Viewing`), holding: the entity snapshot (derived from a fresh
  `GetNote` / from the selected `Task`), the **focused pane index**, and an optional
  **in-progress edit buffer** for the focused field (the only editable surface; absent ⇒ not
  editing).
- **Panes per entity** (each its own bordered box; focused one drawn with the purple border via
  the existing `draw_field`/dialog border style):
  - Task: `Title` (editable), `Description` (editable), `Status` (read-only), `Created`
    (read-only), `Closed` (read-only, shown only when present / Done).
  - Note: `Title` (editable), `Content` (editable), `Created` (read-only).
- **Edit lifecycle (per ADR-0010 §4):** `e` on a focused **editable** pane opens the edit
  buffer (seeded from the current value); printable chars / `Backspace` mutate the buffer;
  `Enter` **commits that one field** → an `UpdateTask`/`UpdateNote` request (task: only the
  edited field via `Option`; note: edited field + the other field from current snapshot), then
  on success re-derives the detail from the server response (#1) and clears the buffer; `Esc`
  **cancels** the edit (drops the buffer, restores the pre-edit value, stays in the detail view).
  `e` on a read-only pane is inert.
- **`Tab`/`Shift+Tab` inside a detail view cycle panes** (wrapping), **not** top-level tabs —
  exactly as they cycle dialog fields today; the existing `active_pane_in_sub_flow` /
  `overlay_capturing_input` gating that diverts `Tab` away from tab-switching must treat an open
  detail view as input-capturing.
- **Two-tiered `Esc`:** edit-in-progress ⇒ cancel the edit; no edit ⇒ exit the detail view back
  to the list. In-flight commit ⇒ `Esc` cancels the request (existing cancel path).
- **Global-suppression (#0015 rule):** while a field edit buffer is open the detail view is an
  input-capturing overlay — `overlay_capturing_input()` returns `true`, so `q`/`t`/`T`/`r`/`?`
  and tab-switch never fire and printable chars go to the buffer. While the detail view is open
  but **no** field is being edited, pane-cycling (`Tab`) and `e`/`Enter`/`Esc` are the detail
  view's keys; whether other globals (`t`/`T`/`r`) stay live in that non-editing state is a
  `tui-dev` choice — **recommended:** treat the open detail view as capturing for action keys
  but keep `?` help available, mirroring how `Viewing` behaves today. Record the chosen rule in
  the slice Log as an assumption so the reviewer and `tester` check the same contract.

### Final hotkey remap (canonical, per the item table + ADR-0010 §4)

Rewrite `map_key` (`crates/tui/src/terminal/mod.rs`) and the `Event` alphabet:

- `c` (toggle done) → **`Space`**; old delete `x` → **`d`**; old timer toggle `p` → **`t`**;
  old duration-edit `d` → **`T`** (configure timer).
- `Enter`: open detail view (Tasks/Notes idle list) / commit field (in a detail edit) /
  switch profile (Profiles) — routed through the existing `Submit`-folding seam plus the new
  detail-view handlers.
- `e`: edit title-only (list) / enter edit on focused pane (detail view).
- Per-entity keys (`a`/`e`/`d`/`Enter`/`Space`) stay **context-scoped to the active tab**;
  globals (`t`/`T`/`r`/`q`/`?`/arrows/`Tab`/`Shift+Tab`) live only when no overlay/edit captures
  input (the 0015 `globals_live` gate, extended to count an open detail-edit as capturing).
- **Delete affordance:** `d` replaces `x` as the delete key. Keep the existing confirm-dialog
  delete (0015) for all three tabs; the old task two-step `x`-again affordance is superseded by
  the `d` confirm dialog (the table specifies "Delete task (confirm dialog, 0015)").
- Update the `?` **help modal** body (`draw_help`) and the footer caption text to the new keys
  (`Space`/`d`/`t`/`T`), and refresh the doc comments on `map_key`/`is_text_entry` that still
  describe the old `c`/`x`/`p`/`d` bindings.

### Task breakdown & agent assignments

**Slice 1 — `tui-dev`: Event alphabet + keymap remap.**

- Extend `Event` (`crates/tui/src/app/mod.rs`) with the detail-view variants needed
  (e.g. `OpenDetail`/`BeginFieldEdit`/`CommitField`/`CycleField`(+rev)/`CloseDetail` or a reuse
  of `Submit`/`Next`/`Prev`/`Cancel` where the existing seam already carries the meaning — prefer
  reuse to keep the alphabet minimal).
- Rewrite `map_key` + `is_text_entry` for the canonical scheme (`Space`/`d`/`t`/`T`, context
  scoping, detail-edit suppression). Update all doc comments to the new bindings.
- Files: `crates/tui/src/terminal/mod.rs`, `crates/tui/src/app/mod.rs` (Event enum + global
  routing for `ToggleTimer`/`BeginEditDuration` rebind to `t`/`T`).

**Slice 2 — `tui-dev`: Task detail view (state + handlers).**

- Add the task detail sub-state + pure handlers on `TaskListState`/`App` (open from `Enter`,
  pane cycle, per-field edit buffer, commit → `UpdateTask` with only the edited `Option` field,
  cancel, exit). Wire `apply_update` to re-derive the open detail from the refreshed list / a
  re-`GetTask`-equivalent (use the existing `ListTasks` refresh + re-select, mirroring current
  task mutation flow — **no new client method**).
- Files: `crates/tui/src/app/task_list.rs`, `crates/tui/src/app/task_add.rs` (or a new
  `task_detail.rs` sibling), `crates/tui/src/app/mod.rs`.

**Slice 3 — `tui-dev`: Note detail view (state + handlers).**

- Convert the read-only `NotesMode::Viewing(Note)` into the editable per-field detail
  (`Title`/`Content` editable, `Created` read-only); `Enter` still issues `GetNote` first so the
  view derives from a server response (#1); per-field commit → `UpdateNote` (edited field +
  other field from snapshot); cancel/exit two-tiered `Esc`.
- Files: `crates/tui/src/app/notes.rs`, `crates/tui/src/app/mod.rs`.

**Slice 4 — `tui-dev`: Rendering.**

- Draw both detail views as per-field bordered panes with the purple focus border on the focused
  pane (reuse `draw_field`/the dialog border style; the detail view renders in the main content
  area, **not** as a floating dialog). Update `draw_help` body + `FOOTER_CAPTION` to the new
  keys. Update `draw_notes_pane`'s `Viewing` branch.
- Files: `crates/tui/src/ui/mod.rs`.

**Slice 5 — `tester`: `TestBackend` coverage (ADR-0003 layer 2).**

- Keymap: extend `crates/tui/tests/keybindings.rs` so every remapped/new binding is pinned —
  `Space` toggles done (and `c` no longer does), `d` deletes (and `x` no longer does), `t`
  start/stop, `T` config, `a`/`e`/`Enter` context-scoped per tab, and the **global-suppression**
  assertion: no action key fires while a dialog **or** a detail-view field edit captures input.
- Detail views: new/extended `crates/tui/tests/` coverage (e.g. extend `flows.rs`/`navigation.rs`
  or add `detail.rs`) for: `Enter` opens task & note detail; `Tab`/`Shift+Tab` cycle panes (and
  do **not** switch top-level tabs while open); `e` enters edit on the focused pane; `Enter`
  commits one field (assert the request payload carries only the edited field for tasks); `Esc`
  cancels an in-progress edit reverting the value; `Esc` with no edit exits to the list; the
  focused pane shows the purple border (buffer-snapshot assertion via the `common` harness +
  synchronous worker-analogue executor for the commit round-trip).
- Files: `crates/tui/tests/keybindings.rs`, `crates/tui/tests/navigation.rs`,
  `crates/tui/tests/flows.rs`, `crates/tui/tests/rendering.rs`, `crates/tui/tests/common/` (new
  builders if needed), and any per-module unit tests under `crates/tui/src/app/*/tests.rs`.

**Slice 6 — `verifier`:** confirm the `TestBackend` suite exists and is green; boot the stack
and exercise the reqwest task/note **update** paths to confirm no server/contract delta (the
per-field commits go over the existing `UpdateTask`/`UpdateNote` routes). Per ADR-0003 the
interactive-TUI behaviour is `tester`-owned; the verifier's live pass is the server-API +
reqwest-client confirmation only.

### File ownership

| Path | Owner |
| --- | --- |
| `crates/tui/src/terminal/mod.rs` (map_key, is_text_entry) | `tui-dev` |
| `crates/tui/src/app/mod.rs` (Event, App routing, detail folding) | `tui-dev` |
| `crates/tui/src/app/task_list.rs`, `task_add.rs` / new `task_detail.rs` | `tui-dev` |
| `crates/tui/src/app/notes.rs` (Viewing → editable detail) | `tui-dev` |
| `crates/tui/src/ui/mod.rs` (detail-view render, help body, caption) | `tui-dev` |
| `crates/tui/tests/**`, `crates/tui/src/app/**/tests.rs` | `tester` |
| `crates/contract/**`, `crates/server/**` | **untouched** (boundary; ADR-0010 §5) |

### Risks

- **R1 — The two-tiered `Esc` / per-pane edit state machine (the subtle correctness point).**
  Three nested levels: (a) field-edit-in-progress, (b) detail view open / no edit, (c) the list.
  `Esc` must unwind exactly one level at a time; an off-by-one (e.g. `Esc` from a field edit
  jumping straight to the list, or quitting the app) is the classic bug. Mitigation: model the
  edit buffer as `Option` on the detail sub-state (its presence *is* the tier discriminant);
  pin every transition with a dedicated `tester` case (slice 5). This mirrors the already-proven
  dialog two-tier `Esc` (0015), so it extends a known-good pattern rather than inventing one.
- **R2 — Global-suppression while a field edit captures input.** A printable char or a `t`/`r`
  must not fire its global while typing into a pane buffer. Mitigation: route the detail-edit
  state into the **existing** `overlay_capturing_input()` / `active_pane_in_sub_flow()` predicates
  so the one unified gate (ADR-0010 §3) covers it — do **not** add a parallel gate. Slice 5
  asserts no action key fires while a field edit is open.
- **R3 — `Tab` overloading.** Inside a detail view `Tab` must cycle *panes*; on an idle list it
  must cycle *tabs*. Same predicate as R2 governs the fork. Pin both directions in `tester`.
- **R4 — Keymap remap blast radius.** `c`→`Space`, `x`→`d`, `p`→`t`, `d`→`T` touches muscle-memory
  keys and every keybinding test plus the `?` help body and footer caption. Mitigation: the
  `keybindings.rs` suite is the regression net — assert both the new binding fires **and** the old
  key no longer does. ADR-0010 §4 flags this as the intended product change.
- **R5 — Note per-field commit re-sends the unchanged field.** `UpdateNoteRequest` has no
  `Option` fields, so committing `Title` must re-send the current `Content` (and vice versa) from
  the pane snapshot, or it would blank the other field. Mitigation: commit from the full detail
  snapshot with the one edited field overlaid; `tester` asserts the payload preserves the
  untouched field. (No contract change — `UpdateTask` *does* have `Option` fields and needs no
  such workaround.)
- **R6 — Stateless invariant (#1) under in-place editing.** The edit buffer is transient
  process-lifetime UI state; the committed value must always re-derive from the server response
  (re-fetch / refresh), never be trusted from the local buffer. Mitigation: on commit success,
  re-derive the detail from the server result exactly as the existing mutation flows do.

### Assumptions

- **A1 — No new ADR.** ADR-0010 §4–§5 already decides this phase and binds it presentation-only;
  0016 cites ADR-0010. (Confirmed against the DTOs — see the planner note above.)
- **A2 — Detail view is a `Screen::Main` sub-mode**, not a new `Screen` variant; the tab
  bar/title/footer keep rendering beneath/around it. (Smallest change that satisfies the
  criteria; #1/#3.)
- **A3 — `Enter` opening a note detail keeps the existing `GetNote`-first behaviour** so the view
  derives from a fresh server response (#1); the task detail may open from the already-selected
  in-memory `Task` (the list is itself server-derived) — re-deriving on commit via the existing
  refresh.
- **A4 — Per-field commit uses existing client methods only:** task via `UpdateTask` with a single
  `Option` field set; note via `UpdateNote` with the edited field + the snapshot's other field. No
  new `ClientRequest` variant and no new route.
- **A5 — `d` delete uses the 0015 confirm dialog** for all three tabs (per the item table); the
  pre-0016 task two-step `x`-again affordance is retired in favour of the dialog.
- **A6 — Read-only panes (`Status`/`Created`/`Closed`)** are rendered but never enter edit mode;
  `e` on them is inert.
- **A7 — While a detail view is open but no field is being edited**, the view captures the
  per-tab action keys and `Tab` (pane cycle); `?` help stays reachable. The precise set of
  globals that remain live in that non-editing state is finalised by `tui-dev` and recorded in
  the slice Log so `tester`/`reviewer` check the same contract (the AFK smallest-change default:
  treat the open detail view as capturing, matching today's `Viewing`).
- **A8 — `Space` is added to the `Event`/`map_key` alphabet as the done-toggle key** only on the
  Tasks tab idle list (not a global); it does not capture as a printable char outside text entry.

<!-- feature: planned by `architect` (`plan` skill); `## Plan(s)` block above. -->

## Log / comments

- [x] 2026-06-28 [orchestrator] Claimed `ready` → `working`. Worktree
  `.claude/worktrees/0016-tui-detail-views-and-hotkeys` cut from `main@7cae76a` (carries the
  plan citing ADR-0010; no new ADR required — confirmed presentation-only). Session: drive
  cycle for 0016. `tui-dev` builds slices 1–4, `tester` slice 5.
- [x] 2026-06-28 [tui-dev] Slice 1 — Event alphabet + keymap remap (`crates/tui/src/terminal/mod.rs`).
  Rewrote `map_key` to the canonical scheme: `c`(done)→`Space`, `x`(delete)→`d` (all three tabs),
  `p`(timer)→`t`, old duration-edit `d`→`T` (configure). Per-entity keys (`a`/`e`/`d`/Space) stay
  context-scoped to the active tab; globals (`t`/`T`/`r`/`q`/`?`/arrows/Tab/Shift+Tab) live only on
  an idle post-auth screen. **No new `Event` variants** — the remap reuses the existing alphabet
  (`Space`→`ToggleDone`, `d`→`DeleteSelected`/`BeginDeleteNote`/`BeginDeleteProfile`,
  `t`→`ToggleTimer`, `T`→`BeginEditDuration`), per the plan's reuse preference. Updated all
  `map_key`/`is_text_entry` doc comments that described the old `c`/`x`/`p`/`d` bindings.
  `./ok.sh fmt` clean; `cargo clippy -p tui --lib --bins` clean; `cargo build -p tui` green. (The
  detail-view `Tab`/`e`/`Esc` routing + `is_text_entry` detail-edit branch land in slices 2–3 with
  the detail state they depend on.)
- [x] 2026-06-28 [tui-dev] Slice 2 — Task detail view (`crates/tui/src/app/task_detail.rs` new,
  `task_list.rs`, `app/mod.rs`, `terminal/mod.rs`, `ui/mod.rs`). New `TaskDetail` sub-state
  (`tasks.detail: Option<TaskDetail>`) — a `Screen::Main` sub-mode, **not** a new `Screen` variant.
  Panes: Title/Description editable, Status/Created read-only, Closed read-only when done. `Enter`
  opens the detail from the selected task (list is server-derived, A3); `Tab`/`Shift+Tab`/arrows
  cycle panes (wrapping); `e` opens the edit buffer on a focused **editable** pane (inert on
  read-only, A6); `Enter` commits that one field via `UpdateTask` with **only** the edited `Option`
  set, then `apply_update` re-derives the detail from the returned task and chains a list refresh
  (#1); `Esc` is **two-tiered** — cancels an in-progress edit (reverting), else exits to the list.
  The edit buffer is modelled as `Option<String>` on the sub-state (its presence is the tier
  discriminant, plan R1). Task delete converted from the `x`-again two-step to the 0015 confirm
  dialog (arm via `d`, confirm via `Enter`, A5). **Routing folded into the existing unified gate**
  (R2/R3): `overlay_capturing_input()` + `active_pane_in_sub_flow()` now count an open detail view,
  so globals/tab-switch suppress and `Tab` cycles panes; `is_text_entry` counts a detail field edit;
  no parallel gate added. **A7 decision (recorded):** an open detail view is input-capturing for the
  per-tab action keys and other globals (`t`/`T`/`r`/`q`/tab-switch all suppressed) **but `?` help
  stays reachable while no field edit is in progress** (the plan's recommended option); while a field
  edit *is* in progress everything including `?` is captured as text. Encoded in `App::can_open_help`
  and the `detail_idle` arm in `map_key`. No new `Event` variants (reused `Submit`, `Next`, `Prev`,
  `BeginEditTask`, `Cancel`, `ToggleHelp`). `./ok.sh fmt` clean; `cargo clippy -p tui --lib --bins`
  clean; `cargo build -p tui` green.
- [x] 2026-06-28 [tui-dev] Slice 3 — Note detail view (`crates/tui/src/app/notes.rs`, `app/mod.rs`,
  `terminal/mod.rs`). Converted the read-only `NotesMode::Viewing(Note)` into an editable per-field
  `NotesMode::Detail(NoteDetail)`: Title/Content editable, Created read-only. `Enter` still issues
  `GetNote` first (view derives from the server response, #1); `apply_get_note` now folds into
  `Detail`. `e` opens the edit buffer on the focused editable pane; per-field commit →
  `UpdateNote` re-sending the **unchanged** field from the snapshot (the request has no `Option`
  fields, plan R5); `apply_update_note` re-derives the open detail from the returned note and chains
  a list refresh (#1). Two-tiered `Esc` and the `Option<String>` edit-buffer tier discriminant
  mirror the task detail. `in_sub_flow` excludes `Detail` (so `?` stays reachable, A7); a separate
  `detail_open`/`detail_editing` pair drives the unified gate (`overlay_capturing_input`,
  `active_pane_in_sub_flow`, `is_text_entry`) and `App::detail_view_open`/`detail_field_editing`.
  `apply_notes` preserves an open `Detail` across the list refresh. No new `Event` variants (reused
  `BeginEditNote`/`Submit`/`Next`/`Prev`/`Cancel`). `./ok.sh fmt` clean; `cargo clippy -p tui
  --lib --bins` clean; `cargo build -p tui` green.
- [x] 2026-06-28 [tui-dev] Slice 4 — Rendering (`crates/tui/src/ui/mod.rs`). Both detail views draw
  as per-field bordered panes in the main content area (**not** a floating dialog): a shared
  `draw_detail_panes` stacks 3-row bordered boxes, the focused **editable** pane carrying the purple
  focus border (reusing the dialog/`draw_field` `Color::Magenta` cue); a focused read-only pane is
  bordered but not purple (signalling `e` is inert, A6). `draw_task_pane`/`draw_notes_pane` render
  the detail when open. Updated `draw_help` body to the final scheme (`t`/`T` timer, `Space` done,
  `d` delete, `Enter` detail, plus a Detail row) and confirmed the longest help line is **62 display
  columns = the dialog inner width** (`DIALOG_WIDTH` 64 − 2 border), so no line wraps/clips at the
  80×24 viewport (ADR-0006 §8.3 caption/band coupling honoured — `FOOTER_CAPTION` and
  `BOTTOM_BAND_ROWS` unchanged, no action key added to the caption). `./ok.sh fmt` clean; `cargo
  clippy -p tui --lib --bins` clean; `cargo build -p tui` green. `./ok.sh test` run: the **library**
  builds, but `crates/tui/tests/**` fails to compile against the new `TaskListState.detail` field and
  the `NotesMode::Viewing`→`Detail` rename — those test files are `tester`-owned (slice 5) and are
  intentionally left untouched per the file-ownership boundary; the suite goes green in slice 5.
- [x] 2026-06-28 [tester] Slice 5 — `TestBackend` coverage (ADR-0003 layer 2;
  `crates/tui/tests/**` only, no non-test source touched). **Compile fixes:** added the new
  `TaskListState.detail` field to the two `common/mod.rs` task-pane builders, and updated the one
  `notes.rs` reference from `NotesMode::Viewing(v)`/`v.content` → `NotesMode::Detail(v)`/`v.note.content`.
  **Keymap remap re-pinned (`keybindings.rs`, `navigation.rs`, `tasks.rs`):** `Space` toggles done
  **and `c` no longer fires**; `d` deletes on all three tabs **and `x` no longer fires**; `t`
  start/stops the timer and `T` opens the timer-config dialog **and old `p`/duration-`d` no longer
  fire**; `a`/`e`/`Enter` context-scoped per tab; the task delete is the 0015 confirm dialog (armed
  via `d`, confirmed via `Enter`, `Esc` cancels — the old `x`-again two-step is retired, A5); the
  global-suppression assertions now cover `t`/`T`/`d` in every dialog kind. **New `detail.rs` (21
  tests)** grouped by acceptance: detail-view lifecycle (`Enter` opens task & note detail, deriving
  the note from a fresh `GetNote` #1; done-task Closed pane); tab overloading R3 (`Tab`/`Shift+Tab`
  cycle panes, wrap, and do **not** switch top-level tabs); edit lifecycle (`e` enters edit on the
  focused pane; `Enter` commits one field — **task payload carries only the edited `Option`**, **note
  payload preserves the untouched field from the snapshot, R5**; commit re-derives the detail + chains
  a list refresh; `e` on a read-only pane inert, A6); two-tiered `Esc` R1 (cancel edit reverting; exit
  to list with no edit; unwinds one level at a time); global-suppression R2/A7 (open detail captures
  `a`/`d`/`Space`/`t`/`T`/`r`/`q`/tab-switch; `?` reachable while no field edit, captured-as-text once
  editing); purple focus border (buffer-snapshot via `row_fg_count`, mirroring 0015 — focused editable
  pane magenta, follows focus, focused read-only pane not magenta); help-body Detail bindings. The
  commit round-trips run through the harness's synchronous worker-analogue executor (`submit`/`drive`),
  the only mock the sanctioned `Client` trait. `./ok.sh fmt --check` clean; `./ok.sh lint` clean;
  `./ok.sh test` green (tui suite **189** tests: 168 carried + 21 new; whole workspace green).

[adr-0003]: ../../docs/adr/0003-verification-layering.md
[adr-0010]: ../../docs/adr/0010-tui-navigation-and-interaction-model.md
