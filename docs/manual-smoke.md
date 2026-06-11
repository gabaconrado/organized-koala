# Manual terminal smoke checklist

A **named, repeatable** manual check for the sliver of TUI behaviour that no automated layer
covers: genuine terminal integration of `organized-koala` (`ratatui` + `crossterm`) on a real
TTY. Per [ADR-0003][adr-0003], this is **documented and ungated** — it is **not** a Definition
of Done criterion and is **not** run per feature. The verifier validates the live server API
and reqwest client path; the tester owns interactive-TUI view/update, keybindings, and
error-code branching via `ratatui`'s `TestBackend`. What remains here is only the real-terminal
integration those layers cannot observe.

## When to run (triggers)

Run this checklist when **any** of the following happens — not on routine feature work:

- A `crossterm` or `ratatui` **dependency version bump**.
- First boot on a **new target platform** (a new OS, terminal emulator, or architecture).
- Any change to **terminal setup/teardown** — raw-mode toggling, alternate-screen
  enter/leave, panic-hook restore, or the resize path.

If a real-terminal regression ever escapes to the human between runs, that is the signal to
**add a new trigger to this list** — not to build a PTY-based gate.

## Checklist

Run against a live stack (`./ok.sh up`, `./ok.sh migrate`, `./ok.sh run-server`) in a real
terminal, then launch the TUI with `./ok.sh run-tui`.

- [ ] **Raw-mode + alternate screen init** — launching the TUI enters the alternate screen and
  raw mode cleanly; the host shell's scrollback is untouched.
- [ ] **Glyph paint** — the initial view renders correctly: borders, layout, and text glyphs
  paint without artifacts or misalignment at the default terminal size.
- [ ] **Resize** — resizing the terminal reflows the layout without panicking or corrupting the
  display.
- [ ] **Keybinding round-trip on a real TTY** — at least one navigation key and one action key
  produce the expected on-screen result (deterministic mapping is already covered by tester;
  here we only confirm real `crossterm` key delivery works).
- [ ] **Clean teardown on quit** — quitting leaves raw mode, leaves the alternate screen, and
  restores the cursor; the shell prompt returns normal and the terminal is usable.
- [ ] **Teardown on panic** — if the app panics, the panic hook still restores the terminal
  (no stuck raw mode / hidden cursor).

## Recording a run

Note the run in the relevant Board item's `## Log / comments` (trigger, platform, terminal,
`crossterm`/`ratatui` versions, and pass/fail per item). **Never write secrets** into the
Board — describe behaviour, not credentials or payloads.

[adr-0003]: ./adr/0003-verification-layering.md
