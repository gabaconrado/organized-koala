# Ideas backlog

A **calm parking lot for out-of-scope follow-ups** — observations, nice-to-haves, suspected
tech-debt, or "worth thinking about later" items that surface during a cycle but are **not** part of
the work item being driven.

This folder is **deliberately outside the Board state machine.** An idea is **not** a work item: it
carries no Definition of done, blocks nothing, and never advances itself. It exists so a follow-up
can be captured **without disrupting the drive loop**, then triaged by the human on their own
schedule. When an idea is accepted, it graduates into a real Board item under `board/features/`.

> The Board is committed and potentially public, and so is this folder. **Never write secrets,
> tokens, or sensitive payloads into an idea** — describe behaviour and shape. The pre-commit secret
> scan covers `board/ideas/` like everything else.

## Where it lives (home #1 — `main` only)

An idea is **shared / cross-cutting future-work state**, so it follows CLAUDE.md three-homes rule #1
(shared → `main` only): it is committed to **`main`** and **never rides a feature branch**. The
orchestrator captures an idea from the main checkout, not inside a worktree — putting one on a branch
is the out-of-sync bug class.

## Lifecycle

```text
open ──(human triages)──▶ accepted ──(drive step 1 promotes)──▶ Board item (board/features/NNNN)
   └────────────────────▶ closed   (kept as a record, with a reason)
```

- **`open`** — captured by an agent/orchestrator during a cycle. Waits for the human.
- The **human** is the only one who flips an idea to **`accepted`** or **`closed`**, writing the
  one-line decision in `## Disposition`. The AI cycle never advances an idea.
- An **`accepted`** idea is promoted into a Board `inbox` item at the next `drive` step 1 (a
  `feature` → `architect`/`plan`; a clearly scope-limited `chore` → minted directly). The idea is
  then stamped `promoted-to: NNNN`.
- **`closed`** and **`accepted`** idea files are **kept** as a calm log — not deleted.

## Capture policy (idea-first)

When any agent flags a follow-up out of scope of the current item, the default disposition is to
**file an idea here** for the human to triage — **not** to mint a Board item. Direct minting of a
`chore`/`feature` mid-cycle is reserved for the **genuinely urgent** (e.g. a security leak like
0013's JWT); even then, record an idea alongside so the trail is complete.

## Naming

`NNNN-<slug>.md`, with its **own sequence** (0001, 0002, …) independent of `board/features/`. Refer
to one as "idea NNNN".

## Frontmatter

```yaml
---
id: 0001
title: <short imperative title>
status: open          # open | accepted | closed   (only the human flips open → accepted/closed)
priority: low         # high | medium | low — a triage hint, never a gate
created: 2026-06-26   # absolute date
source: 0012          # Board item / cycle that surfaced it, or "adhoc"
raised-by: reviewer   # reviewer | verifier | eng-manager | architect | orchestrator | human
promoted-to: null     # NNNN of the resulting Board item, set when accepted & promoted
---
```

## Content template

Copy [`TEMPLATE.md`](./TEMPLATE.md). Sections:

- **`## What`** — one paragraph: the observation / suggestion.
- **`## Why it matters`** — impact, risk, or value — and why it was out of scope of the surfacing
  task.
- **`## Possible approach`** (optional, non-binding) — a sketch, not a plan. It does **not** pre-empt
  the `architect`; if accepted as a `feature`, the real plan is written then.
- **`## Disposition`** — a single `- [ ] <ts> [human] decision: …` line the human checks when they
  accept (→ promote) or close (with a reason). This mirrors the Board's `[human]` re-entry signal.
