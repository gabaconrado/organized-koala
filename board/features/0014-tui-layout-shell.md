---
id: 0014
title: TUI layout shell — top-level tabs, centered title, centered auth form, tight footer
type: feature      # feature | chore
status: inbox           # inbox → planned → ready → working → review → awaiting-merge → merged | blocked
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

<!-- feature: needs an `architect` plan (`plan` skill) writing a `## Plan(s)` block before code. -->
<!-- Open question for the architect: does the new TUI interaction model (tabs + later dialogs + detail views, 0014–0016) warrant its own ADR for the TUI shell, or is it presentation-only and ADR-free? Settle before planning 0015/0016. -->

[adr-0003]: ../../docs/adr/0003-verification-layering.md
