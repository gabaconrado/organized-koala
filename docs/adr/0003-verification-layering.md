# ADR-0003: Verification layering — who validates the TUI

**Status:** Accepted · 2026-06-11

## Context

Definition of Done #4 (see [CLAUDE.md][claude-md]) requires the [`verifier`][verifier] to
boot the live stack and "exercise the affected flows against a live server," and
`verifier.md` currently tells it to exercise "the TUI/CLI flows the feature touched." The
TUI (`organized-koala`, `ratatui` + `crossterm`) is an **interactive** terminal app:
`crossterm` requires a real TTY, and driving it headlessly would need a fragile PTY harness
feeding raw keystrokes and asserting on escape sequences. The verifier (read-only; `Read`,
`Grep`, `Glob`, `Bash`) **cannot reliably drive the interactive TUI**, so the current
expectation is unmeetable.

We must settle where interactive-TUI validation lives **without dropping it from acceptance
criteria** — narrowing DoD to "API-only" would silently delete confidence in the client.

This is a process/contract decision (it reshapes agent contracts and DoD), so it is recorded
as an ADR before the agent edits land. It does not change any wire shape.

### Forces

- Hard-constraint #1 ([ADR-0001][adr-0001]): the TUI is **stateless** — every view derives
  from a server response. This makes the TUI's *data* observable over HTTP, but it does **not**
  make the TUI's *rendering, keybinding, and error-code branching* observable over HTTP: those
  are pure client logic that never round-trips.
- A real-PTY end-to-end harness is exactly the fragile, non-deterministic mechanism we want to
  avoid; it would re-introduce the problem in CI.
- The residual "does it paint in a real terminal" risk is real but thin and low-frequency: it
  breaks on terminal/dependency changes, not on feature changes.
- "Delegate to tester" must not degrade into "drop": something must assert the delegated
  coverage actually exists and is green.

## Decision

Verification is **routed by layer**, and each layer names the mechanism that owns it.

1. **`verifier` — live, end-to-end: the server API and the reqwest client path.** The verifier
   boots the stack and exercises every server-owned behaviour the feature touched as real HTTP
   round-trips: request/response shapes, status codes, the error contract (`{ code?, message }`),
   profile-scoping, persistence, and OTel spans. Because the TUI is stateless, all
   **server-owned state** the TUI would display is reachable and assertable this way without the
   TUI binary. The verifier does **not** drive the interactive TUI.

2. **`tester` — deterministic, in-memory: TUI view/update logic, keybindings, and error-code
   branching.** The interactive surface is covered with `ratatui`'s `TestBackend` (in-memory
   buffer, no TTY) driven by synthetic `crossterm` `KeyEvent`s, with the server **mocked** (an
   already-permitted external-service mock per [coding-standards][coding-standards] and
   `tester.md`). This owns: keybinding → action mapping, view rendering/layout against a buffer
   snapshot, and the TUI's branching on the error `code`. It is deterministic and CI-safe.

3. **Human — documented manual smoke check, NOT a gated criterion.** The only sliver neither
   layer covers is genuine terminal integration: `crossterm` raw-mode/alternate-screen init and
   actual glyph painting on a real TTY. This is a **named, repeatable** smoke checklist
   (`docs/manual-smoke.md`, owned by `eng-manager`) run at defined triggers — `crossterm`/
   `ratatui` dependency bumps, first boot on a new platform, or any change to terminal
   setup/teardown — **not** on every feature. It is explicitly **not** a DoD gate.

4. **Delegation handshake (so "delegate" is not "drop").** When a feature touches the TUI, the
   verifier must confirm the corresponding `TestBackend` suite **exists and is green** under
   `./ok.sh test` for the touched surface, and quote that result. If it is absent, the verifier
   reports **verified-with-gaps** and routes the gap to `tester` — the live-API pass alone is
   not sufficient sign-off for a TUI-touching feature.

### Interaction with timer authority (ADR-0002, still open)

This boundary is consistent with — and reinforces — the likely [ADR-0002][adr-0002] outcome
that the **server owns the Pomodoro countdown** and the TUI only renders a server-supplied
remaining time (implied by hard-constraint #1). Under that model the countdown *truth*
(remaining/elapsed, completion, stop-resets) is a server response: the **verifier** validates
it live by polling the timer endpoint and observing the value advance against wall-clock and
reset on stop. The TUI's role is to render a server-supplied duration, which the **tester**
covers with a synthetic timer DTO through `TestBackend`. There is **no client-side countdown to
drive through a PTY**. This ADR does not pre-decide ADR-0002; if ADR-0002 instead placed
authority client-side, the countdown logic would move into tester's deterministic suite and
this routing would be revisited.

## Consequences

- **Interactive-TUI behaviour stays in acceptance criteria**; the ADR names which mechanism
  owns which layer rather than narrowing DoD to "API-only."
- DoD #4 is reworded to say the verifier verifies the **live API/client surface** and that
  interactive-TUI behaviour is delegated to tester's `TestBackend` suite (with the handshake);
  `verifier.md` and `tester.md` are amended to match. See "Downstream edits" below.
- A new `docs/manual-smoke.md` checklist exists and is maintained by `eng-manager`; running it
  is a documented manual step, not a gate.
- The thin terminal-integration risk is **accepted** between smoke runs. If a real-terminal
  regression ever escapes to the human, that is the signal to add a trigger to the smoke
  checklist — not to build a PTY gate.
- This decision is process-only: no `contract` change, no source change mandated by the ADR
  itself.

## Downstream edits (mandated; designed here, implemented by the named owners)

1. **`.claude/agents/verifier.md`** (owner: `eng-manager`) — replace the "exercise the TUI/CLI
   flows" framing with live API/client scope plus the delegation handshake:
   - Responsibility bullet, replace *"exercise the TUI/CLI flows the feature touched against the
     live server"* with: *"exercise the server API and reqwest client path the feature touched
     against the live server — request/response shapes, status codes, the error contract,
     profile-scoping, persistence, and OTel spans. Do not drive the interactive TUI; per
     [ADR-0003] its view/update, keybinding, and error-branching behaviour is owned by
     `tester`'s `TestBackend` suite."*
   - Add a Constraints bullet: *"For any TUI-touching feature, confirm the corresponding
     `TestBackend` suite exists and is green under `./ok.sh test` and quote that result. If it is
     absent or red, report **verified-with-gaps** and route the gap to `tester` — a live-API pass
     alone is not sign-off for a TUI feature."*

2. **`CLAUDE.md`** (owner: `eng-manager`) — Definition of Done item #4, replace with:
   *"**`verifier` ran it for real** — booted the stack and exercised the affected **server API
   and reqwest client path** against a live server (shapes, status codes, error contract,
   profile-scoping, OTel spans), quoting what actually ran vs. what was inferred. Interactive-TUI
   behaviour (view/update, keybindings, error-code branching) is owned by `tester`'s `ratatui`
   `TestBackend` suite, not the verifier; for a TUI-touching feature the verifier confirms that
   suite exists and is green (see [ADR-0003])."*

3. **`.claude/agents/tester.md`** (owner: `eng-manager`) — sharpen the existing "mock the server
   for TUI tests" line into an explicit ownership statement. Add a Primary-responsibilities
   bullet: *"Own the interactive-TUI suite: view/update logic, keybindings, and error-code
   branching, exercised via `ratatui`'s `TestBackend` (in-memory buffer) with synthetic
   `crossterm` `KeyEvent`s and the server mocked. This is the gated home of interactive-TUI
   verification per [ADR-0003]; the verifier does not drive the TUI."*

4. **`docs/decisions.md`** (owner: `architect`, done with this ADR) — add the index row and
   resolve the link reference.

5. **`docs/manual-smoke.md`** (owner: `eng-manager`) — new file: the named manual terminal smoke
   checklist (raw-mode/alt-screen init, glyph paint, teardown) with its trigger list. Documented,
   ungated.

[claude-md]: ../../CLAUDE.md
[verifier]: ../../.claude/agents/verifier.md
[coding-standards]: ../../.claude/skills/coding-standards/SKILL.md
[adr-0001]: ./0001-foundational-architecture.md
[adr-0002]: ./decisions.md
[adr-0003]: ./0003-verification-layering.md
