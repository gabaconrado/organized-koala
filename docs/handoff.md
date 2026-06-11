# Handoff — engineering journal

Reverse-chronological. `eng-manager` appends one entry per completed cycle at the **top** and
keeps the "What works right now" snapshot at the bottom current.

---

## Handoff — 2026-06-11 (0002 — contract crate + workspace restructure)

Branch: `feature/0002-contract-crate` (head `d79fc9c`, linear atop `main` `1a2540c`,
fast-forward — frozen for the human to merge). Slice 1 of 3 of the foundational slice 0001.

What shipped:

- Removed the `crates/organized-koala` placeholder; the workspace now matches the target
  `contract`/(`server`)/(`tui`) layout. `crates/contract` authored as the single source of
  truth for the foundational wire shapes per ADR-0005.
- DTOs: `RegisterRequest`, `LoginRequest`, `SessionResponse`, `Profile`, `Task`, `TaskStatus`,
  `CreateTaskRequest`, `ErrorBody { code?, message }` + the 7 stable error codes with a lossless
  `Unknown` catch-all; a `Password` newtype (transparent serialize, `[REDACTED]` Debug).
- 37 serde/wire-format integration tests + 12 doctests green; build/lint/fmt clean. Reviewer
  approved at `d79fc9c`; verifier confirmed the pure-DTO seam (live-stack E2E deferred to
  0003/0004 per ADR-0003).
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
- **secret-scan matches credential VALUES, not bare identifiers** (`37b78c4`): a bare Rust field
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
- **The `contract` crate exists** (0002, in-flight and branch-owned on
  `feature/0002-contract-crate`, awaiting human merge): a compile-only, pure-DTO seam carrying
  the foundational wire shapes (auth/profile/task DTOs, `ErrorBody`, error codes, the redacting
  `Password` newtype) per ADR-0005. The workspace now matches the target layout (placeholder
  crate removed). No HTTP/DB/TUI behaviour yet.
- **No running application yet.** 0003 (server: auth + default profile + tasks + migrations +
  docker stack) is `ready` and unblocked; 0004 (TUI) follows it. Together 0002–0004 complete
  the foundational vertical slice 0001.
