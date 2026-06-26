---
id: 0014
title: TUI layout shell — top-level tabs, centered title, centered auth form, tight footer
type: feature      # feature | chore
status: working         # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
# ^ FROZEN at claim snapshot — authoritative copy is on branch feature/0014-tui-layout-shell
#   (cut from main@6511941). The human's merge brings the finished item back to main.
priority: medium    # high | medium | low
parent: null
depends-on: []
branch: feature/0014-tui-layout-shell
worktree: .claude/worktrees/0014-tui-layout-shell
created: 2026-06-26
updated: 2026-06-26
---

## Feature request

**Goal:** Phase 1 of a three-part TUI overhaul (0014 → 0015 → 0016). This phase reshapes the
**structural shell** only — the navigation model, the title bar, the auth screen, and the
footer position. No dialogs (0015) and no detail views / final hotkey remap (0016) yet.

**Context (current behaviour to change):**

- There are **no tabs**. Tasks, Notes, and Profiles are three separate full screens; Notes is
  reached with `n` and Profiles with `s` from the task list, `Esc` returns. (`map_key`,
  `crates/tui/src/terminal/mod.rs`; `Screen` enum, `crates/tui/src/app/mod.rs`.)
- The **auth screen is full-width and top-aligned** — fields stacked from the top across the
  whole terminal (`crates/tui/src/ui/mod.rs`, auth render).
- The **title** is a plain top header per screen, without the account/profile context.
- The **footer** band (caption + timer) sits with a tall bottom margin
  (`BOTTOM_BAND_ROWS = 3`, `crates/tui/src/ui/mod.rs`).

**Surface to build (TUI only — no `contract`/server change):**

- **Top-level tabs (point 7).** Replace the three-separate-screens model with a single
  post-auth view that has a **tab bar**: `Tasks | Notes | Profiles`. The selected tab's pane is
  the main content. **Default selection is Tasks** on entry.
  - **Tab switching is via `Tab` / `Shift+Tab` only** (cycle Tasks → Notes → Profiles → Tasks
    and back). There are **deliberately no `t`/`n`/`p` tab-letter hotkeys** — this also keeps
    `t` free for the timer in 0016. Remove the old `n` (open notes) and `s` (open profiles)
    cross-screen navigation.
  - Within a list, **arrows move the selection** (Tab is now owned by tab-switching, not list
    navigation). The list selection state per tab is preserved when switching away and back.
- **Centered, compact auth form (point 1).** Replace the full-width top-aligned login/register
  layout with a **small, centered form** (a bounded box centered horizontally and vertically),
  not two lines spanning the full screen. Preserve the existing Login⇄Register toggle and all
  current fields (Login: identifier, password; Register: username, email, password, profile
  name) and the inline error band.
- **Centered title with account + profile (point 8).** The title is **centered** and reads
  exactly `organized koala - <user> @ [<profile>]` (literal square brackets around the active
  profile name). `<user>` is the logged-in account identifier; `<profile>` is the active
  profile. Shown on the post-auth view (the auth screen keeps a simple centered title).
- **Tight footer (point 3).** Pull the footer (hotkey caption + timer label) **very close to
  the bottom** of the screen — remove the large bottom margin so it hugs the last row.

**Acceptance criteria:**

- [ ] Post-auth, the UI shows a `Tasks | Notes | Profiles` tab bar with **Tasks selected by
      default**; `Tab`/`Shift+Tab` cycle the tabs both directions and the selected pane updates.
- [ ] No `t`/`n`/`p`/`s` keys switch tabs or screens; arrows move the selection within the
      active list; per-tab selection survives a tab switch.
- [ ] The login/register screen renders as a **small centered form** (bounded box, centered
      both axes), with the Login⇄Register toggle and all existing fields/error band intact.
- [ ] The title is centered and renders `organized koala - <user> @ [<profile>]` with the live
      account identifier and active profile.
- [ ] The footer (caption + timer) sits flush near the bottom row (no large bottom margin).
- [ ] **No behavioural change to add/edit/delete sub-flows or hotkeys beyond navigation** —
      those are reshaped in 0015/0016; this phase must not regress existing task/note/profile
      CRUD reachable through the new tabs.
- [ ] Full `feature` Definition of done: `./ok.sh test | lint | fmt --check` green; `reviewer`
      approved (pinned to `./ok.sh code-hash`); the TUI change covered by the `ratatui`
      `TestBackend` suite ([ADR-0003][adr-0003]); the `verifier` confirms that suite is green
      and boots the stack to confirm the reqwest paths still function (no server/contract delta
      to exercise).

**Out of scope (later phases / would need an ADR):** any dialog/modal (→ 0015), the `?` help
modal and caption trimming (→ 0015), purple focus styling (→ 0015), task/note detail views and
the full hotkey remap incl. timer/refresh/quit keys (→ 0016). No `contract` or server change;
no new domain structure (#3) — tabs are presentation only.

## Plan(s)

### Plan: TUI layout shell — tabbed post-auth view, centred auth/title, tight footer

**Approach:** A `tui`-crate-only reshape of the navigation model and three presentation
touch-ups, behind the ADR-0003/0006 testability seam. The **tracer-bullet slice** is the
`Screen`/`App` reshape: collapse the three mutually-exclusive list screens
(`Screen::TaskList`/`Notes`/`Profiles`) into one post-auth tabbed view holding all three pane
states, with `Tab`/`Shift+Tab` cycling the active tab and per-tab selection preserved. Once
that core compiles and renders an empty tab bar with the Tasks pane, the slice widens to
re-home the existing list rendering under each tab, then the three independent presentation
changes (centred auth form, centred contextual title, tight footer) layer on. No layer does
I/O inline; every list pane still derives from a server response (#1). No `contract`/server
change (ADR-0010 §5 binding boundary).

**ADR:** [ADR-0010][adr-0010] — *TUI navigation and interaction model (tabs, dialogs, detail
views)*, written and committed to `main` with this plan. Settles the open question: the
0014–0016 arc **does** warrant a TUI-shell ADR (it reshapes the `Screen`/`Event` state-machine
contract and sets cross-phase invariants — the ADR-0006 category), and it **confirms** the
feature request's assertion that the work is **presentation-only** (no `contract`/server/domain
change) by making that boundary binding for all three phases.

**Slices:**

1. **[tui-dev] Reshape the post-auth state machine into a tabbed view (tracer bullet).** Replace
   the three mutually-exclusive post-auth `Screen` variants with one tabbed post-auth view that
   holds the three pane states together and tracks the active tab (concrete shape — a single
   `Screen::Main { active_tab, tasks, notes, profiles }` variant, or an equivalent active-tab
   discriminant — is tui-dev's choice per ADR-0010 §1). Add the tab-cycle events
   (`Tab`→next tab, `Shift+Tab`→prev tab) and route list-navigation onto **arrows only**.
   Preserve each pane's selection across switches. A tab switch yields a list derived from a
   server response for the active profile (already-loaded in-memory list for the active profile,
   or a fresh load) — never another profile's data (#1, #4). Remove the old `OpenNotes`/
   `OpenProfiles`/`Back` cross-screen navigation and the `pick_active_profile` `Esc`-back path
   that depended on the separate-screen model (profile pick-active itself stays). — files:
   `crates/tui/src/app/mod.rs` (the `Screen` enum, `handle_event`/`handle_screen_event`
   routing, `apply_*` response folding that re-points the active screen, `is_post_auth`,
   `screen_pending_id`/`set_screen_pending`/`cancel_in_flight`), and the per-pane state modules
   only where the pane state must expose what the tabbed container needs
   (`crates/tui/src/app/task_list.rs`, `notes.rs`, `profiles.rs`).

2. **[tui-dev] Remap keys to the tab-navigation model.** Update `map_key` so `Tab`/`BackTab`
   drive tab-switching on a post-auth list (not list movement); arrows move the list selection;
   remove the `n` (open notes), `s` (open profiles), and idle-`Esc`-back bindings. **Keep every
   other existing binding unchanged** (`a`/`e`/`c`/`x` per pane, `p`/`d` timer, `r` refresh,
   `q` quit, `Esc`=cancel in a sub-flow / quit when idle on Tasks) — the full keymap remap is
   0016, not this phase (acceptance: "no behavioural change to sub-flows or hotkeys beyond
   navigation"). Note `t` is deliberately left **unbound** as a tab hotkey so 0016 can claim it
   for the timer. — files: `crates/tui/src/terminal/mod.rs` (`map_key`, `is_text_entry`).

3. **[tui-dev] Centred, compact auth form + simple centred auth title.** Replace the
   full-width top-aligned auth layout with a small bounded box centred on both axes; centre the
   auth title. Preserve the Login⇄Register toggle, all fields (Login: identifier, password;
   Register: username, email, password, profile name), and the inline error band. Capture the
   entered account identifier into the in-memory `Session` at auth time so slice 4 can render
   `<user>` — **client-side only, no new wire** (ADR-0010 §2). — files:
   `crates/tui/src/ui/mod.rs` (`draw_auth`, `draw_field`), `crates/tui/src/app/mod.rs`
   (`Session` gains the account identifier; set it where the session is established in
   `apply_profiles`), `crates/tui/src/app/auth.rs` only if the identifier must be surfaced from
   the auth state at session-creation time.

4. **[tui-dev] Centred contextual title + tight footer on the tabbed view.** Render the
   post-auth title centred as exactly `organized koala - <user> @ [<profile>]` (literal square
   brackets), with the tab bar (`Tasks | Notes | Profiles`, Tasks default) below it and the
   selected pane as the main content. Pull the footer (caption + timer) flush to the bottom row
   — remove the large bottom margin (`BOTTOM_BAND_ROWS` shrinks / the layout no longer reserves
   the tall bottom band). Keep the existing caption + timer widget content for this phase (the
   caption trim is 0015). — files: `crates/tui/src/ui/mod.rs` (`draw`, the per-pane draw fns
   merged/refactored under the tabbed view, `draw_bottom_row`, `BOTTOM_BAND_ROWS`).

5. **[tester] `TestBackend` suite for the reshaped shell (ADR-0003 layer 2).** Update the
   `common` builders for the new tabbed `Screen` shape, then cover: tab bar renders with Tasks
   selected by default; `Tab`/`Shift+Tab` cycle Tasks→Notes→Profiles→Tasks and back and the
   pane updates; **no** `t`/`n`/`p`/`s` switches a tab; arrows move the selection; per-tab
   selection survives a switch away and back; the auth form renders as a centred bounded box
   with the toggle + all fields + error band; the title renders
   `organized koala - <user> @ [<profile>]` with a live identifier/profile; the footer sits
   flush near the bottom row; and the existing task/note/profile CRUD + timer flows still reach
   their server calls through the new tabs (no regression). The only mock stays the `Client`
   trait; the synchronous executor analogue is unchanged. — files:
   `crates/tui/tests/common/mod.rs` (builders for the tabbed `Screen`), `crates/tui/tests/keybindings.rs`,
   `crates/tui/tests/rendering.rs`, `crates/tui/tests/flows.rs`, and the per-feature suites
   (`tasks.rs`/`notes.rs`/`profiles.rs`/`timer.rs`/`in_flight.rs`/`error_branches.rs`) wherever
   they construct a `Screen` or assert pre-tab navigation.

**Assumptions** (resolved per the human-AFK ambiguity policy — smallest change that satisfies
the acceptance criteria):

- **A1 — ADR warranted (settled).** The arc reshapes the `Screen`/`Event` contract and sets
  cross-phase invariants, so it gets its own TUI-shell ADR (ADR-0010), mirroring ADR-0006's
  precedent for a wire-neutral TUI runtime reshape. 0015 and 0016 **inherit** ADR-0010 and amend
  it only if a phase needs a wire/server/domain change (none is expected). This resolves the
  HTML-comment open question in the 0014/0015/0016 items.
- **A2 — Presentation-only confirmed.** Tabs/title/auth-form/footer render existing DTOs over
  existing client methods; no new endpoint, DTO field, error code, or domain structure. The
  request's "no contract/server change" assertion is **confirmed**, not refuted.
- **A3 — `Screen` shape is tui-dev's choice.** ADR-0010 §1 fixes the *invariants* (one tabbed
  post-auth view, Tab/Shift+Tab cycle, per-tab selection preserved, server-derived panes) but
  not the exact enum. tui-dev picks the smallest correct representation.
- **A4 — Title `<user>` is the entered account identifier, captured client-side at auth.** The
  literal format is `organized koala - <user> @ [<profile>]` (note: the title text uses
  `organized koala` with a space and no `—` em dash, per the request's exact string — distinct
  from the current `organized-koala — …` headers, which this phase replaces). `<user>` is taken
  from the login/register identifier the user typed and stored in the in-memory `Session`; **no
  new wire** (ADR-0010 §2). If it turned out the identifier could only come from the server, that
  is an ADR event — it does not, so no block.
- **A5 — "Tight footer" keeps the current caption + timer content.** Only the *position*
  (bottom margin) changes this phase; trimming the caption to essentials and the `?` help modal
  are 0015. The band still needs enough rows for the wrapped caption + spinner + cancel
  affordance at 80×24 (ADR-0006 §8.3, learned 0010), so "flush to bottom" means removing the
  *outer bottom margin*, not shrinking the band below what the existing caption needs.
- **A6 — Profile pick-active is retained, re-homed onto the Profiles tab.** Today `Submit` on
  the idle switcher re-scopes the active profile (client-side, ADR-0009 §5). That behaviour
  stays; only its *reachability* changes (it lives under the Profiles tab instead of a separate
  screen). This is navigation, not a CRUD/behaviour change, so it is in scope for 0014.
- **A7 — Auth and Offline stay non-tabbed full screens.** The tab bar is post-auth only; the
  auth screen and the blocking offline screen keep their own single-purpose layouts (the offline
  screen is untouched this phase).

**Risks:**

- **Blast radius inside the `tui` crate is real (contained to the crate, zero wire risk).** The
  `Screen` reshape ripples through `app/mod.rs` routing/response-folding, `map_key`, `ui/mod.rs`,
  and **every test that constructs a `Screen` directly** (the `common` builders + most suites).
  Mitigation: tracer-bullet slice 1 lands the new shape first so the compiler surfaces every call
  site; tester's slice 5 follows the same builders.
- **Per-tab selection + server-derived panes (#1) interplay.** Preserving selection while still
  deriving each pane from a server response must not reintroduce client-side caching of stale
  data or cross-profile leakage (#4). Mitigation: selection index is transient UI state; the pane
  *data* is always a server-response-derived list for the active profile, asserted in slice 5.
- **The exact title string is load-bearing for acceptance.** `organized koala - <user> @
  [<profile>]` (spaces, hyphen, literal brackets) is asserted verbatim — a stray em dash or
  bracket fails the criterion. Pinned by a slice-5 rendering assertion.
- **Scope creep toward 0015/0016.** It is tempting to trim the caption or remap keys now;
  ADR-0010 §5 + the acceptance "no change beyond navigation" forbid it. Reviewer checks this
  phase against ADR-0010's phase boundary.

[adr-0010]: ../../docs/adr/0010-tui-navigation-and-interaction-model.md

<!-- feature: needs an `architect` plan (`plan` skill) writing a `## Plan(s)` block before code. -->
<!-- Open question for the architect: does the new TUI interaction model (tabs + later dialogs + detail views, 0014–0016) warrant its own ADR for the TUI shell, or is it presentation-only and ADR-free? Settle before planning 0015/0016. -->

[adr-0003]: ../../docs/adr/0003-verification-layering.md
