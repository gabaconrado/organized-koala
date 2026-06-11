# Handoff — engineering journal

Reverse-chronological. `eng-manager` appends one entry per completed cycle at the **top** and
keeps the "What works right now" snapshot at the bottom current.

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
- **No application functionality yet.** The first cycle (feature 0001) builds the walking
  skeleton: auth + a profile + minimal TODO end-to-end across server and TUI.
