---
name: finalize
description: The human-run merge step. Run /finalize at the end of a drive cycle when a feature
  sits at awaiting-merge — it audits authorship + commit format, re-freshens the branch onto main,
  re-checks the DoD, fast-forward-merges, flips the item to merged + regenerates the dashboard, and
  tears down the worktree/branch. Never pushes; the operator's run IS the merge authorization.
audience: human
---

# finalize

## When to invoke

- The human runs `/finalize` **from the main checkout** (repo root) at the end of a `drive`
  cycle, when exactly one work item sits at `status: awaiting-merge` on its feature branch and
  they want it merged into `main`.
- This is the **human's merge step** — the one action the AI cycle is forbidden to take on its
  own (`drive` is terminal at `awaiting-merge`; see that skill's step 8). Running `/finalize`
  **is** the operator's authorization for the merge, the `merged` status flip, and the
  worktree/branch teardown below. Nothing about the authorization model changes: still **no
  `git push`, no remote-mutating command** of any kind — those remain the operator's to run by
  hand afterward.

## What it guarantees before merging

1. **Authorship + format are correct** on every branch commit (the audit below) — the human is
   the author, the committing agent is a well-formed co-author trailer, and subjects are
   Conventional Commits.
2. **The branch is current on `main`** (re-rebased if `main` moved), so the merge is a clean
   fast-forward and the human merges exactly what was reviewed.
3. **The Definition of done still holds** at the merged tree — verdicts still pin to the live
   `./ok.sh code-hash`, and `test | lint | fmt --check` are green.

If any guarantee fails and cannot be repaired deterministically, `/finalize` **STOPS and
surfaces** rather than merging.

## Procedure

> Run from the **main checkout** at the repo root, with `main` checked out and a clean working
> tree. Recompute all state from the Board frontmatter + `git` — never trust a cached value.

### 1. Locate the item + branch

Find the `board/features/NNNN-<slug>.md` whose **branch copy** is `status: awaiting-merge`, and
read its `branch:` / `worktree:` frontmatter. Cross-check against `git worktree list`.

- **Zero** items at `awaiting-merge` → nothing to finalize; report and stop.
- **More than one** → ask the human which item to finalize (do not guess).

Confirm the main working tree is clean (`git status --porcelain` empty). If dirty, stop and
surface — `/finalize` must not merge over uncommitted main-side changes.

### 2. Audit authorship + commit format (the gate)

Enumerate the branch's own commits — `git log --format=... main..<branch>` (those not yet on
`main`). Read **raw** history (the `rtk` proxy truncates long `git log` output — verify counts
with `git rev-list --count` and bypass with `rtk proxy git log …` when a full enumeration
matters). For **each** commit check:

- **Author + committer are the human.** The canonical human identity is this checkout's
  `git config user.name` / `user.email`. Any commit whose author **or** committer email ends in
  `@organized-koala.local` (an agent identity) is a defect.
- **A well-formed agent co-author trailer is present.**
  `Co-authored-by: <agent> <agent@organized-koala.local>` (identity owned by `git-standards`).
  A `noreply@anthropic.com` co-author trailer is **never correct in this repo** (learned
  0003/0004/0009) and is a defect.
- **The subject is a Conventional Commit** — `<type>[scope]: <description>`, type in
  `feat|fix|docs|refactor|test|chore|build|ci|perf|style` (`git-standards`).

**Deterministic fix (safe — this history is pre-merge and never pushed).** When a commit's
**author/committer is an agent**, the agent identity *is* the author field, so the repair is
unambiguous and mirrors the known-good pattern: rewrite author **and** committer to the human,
and append `Co-authored-by: <that-agent> <that-agent@organized-koala.local>` if no valid trailer
is already present. Apply over the branch range with `git filter-branch` scoped to
`<base>..<branch>` (env-filter to rewrite the agent emails → human; msg-filter to append the
trailer only on the commits that lack it), exactly as a branch-local rewrite — **no force-push**,
because the branch was never pushed. Rewriting commit metadata/messages does **not** touch
`crates/`/manifests, so `./ok.sh code-hash` is unchanged and the review/verify verdicts stay
valid (they pin to the code-tree hash, not the sha — the verdicts' sha pointers simply go stale,
which is fine). Always back up first: `git branch backup/finalize-NNNN <branch>` before any
rewrite.

**STOP and surface (do not guess)** when the defect is *not* deterministically repairable:

- A **human-authored** commit carrying a `noreply@anthropic.com` (or otherwise wrong) trailer —
  the intended agent is not derivable from the author field. Hand back to the orchestrator /
  human to correct.
- A non-Conventional-Commit subject — do not silently rewrite someone's message; report it.

A purely human commit with **no** agent trailer is **fine** (not every commit is agent work);
only require a trailer where the author was an agent.

### 3. Re-freshen against `main` (re-run drive step 7)

`main` may have advanced since the branch was last freshened (eng-manager's learnings, a parallel
merge). Re-apply `drive` step 7 so the merge is a fast-forward and verdicts are re-validated:

- `git merge-base --is-ancestor main <branch>` → **already current**: skip to step 4.
- **Behind** → record `OLD_HASH = ./ok.sh code-hash` at the branch head (run the **worktree's
  own** `ok.sh`, or pass the explicit branch sha). Then `git rebase main` **in the worktree**.
  The expected conflict is only the feature-local Board file (`main`'s frozen-pointer note vs.
  the branch's authoritative copy) — resolve in favour of the **branch** (drop the pointer note).
  Then gate on code identity:
  - **`code-hash` (rebased head) == `OLD_HASH`** — `main` moved only where the branch doesn't
    (`docs/`, `.claude/`, the Board file). Verdicts **carry forward untouched**; append a
    one-line freshen note to the Log. Continue.
  - **`code-hash` differs** — the rebase changed compiled code, so `approved`/`verified` are
    **void**. **STOP.** Do **not** merge: the item must re-enter `drive` at step 4 (review) →
    step 5 (verify) on the rebased head. Report this clearly; `/finalize` cannot merge a branch
    whose verdicts no longer attest the live code.

### 4. Re-check the Definition of done

Confirm the item's `type` gate holds at the (possibly rebased) head, pinned to the **current**
`./ok.sh code-hash`:

- Re-run `./ok.sh test`, `./ok.sh lint`, `./ok.sh fmt --check` — all green.
- `REVIEW-STATUS: approved` is recorded and pins to the current code-hash (for a `chore`, the
  approval must additionally attest the no-change invariant).
- For a **`feature`**: a `verifier` verdict (verified) pins to the current code-hash. For a
  **`chore`**: no verifier pass is required (it is skipped by design).

Any gate red → STOP and surface; do not merge.

### 5. Fast-forward merge (linear history)

From the main checkout, with `main` checked out:

```sh
git switch main
git merge --ff-only <branch>
```

The rebase in step 3 guarantees this is a true fast-forward — **no merge commit, no squash**
(`git-standards`: linear history). This brings the branch's commits (including the item file at
`awaiting-merge`) onto `main`.

### 6. Flip to `merged` + regenerate the dashboard

On `main`, edit `board/features/NNNN-<slug>.md` `status: awaiting-merge` → `merged`, and
regenerate the derived `board/README.md` dashboard from item frontmatter + active branch heads.
Commit on `main` as the orchestrator (`claude`):

```sh
git add board/
git commit -m "docs(board): NNNN awaiting-merge→merged (operator-authorised ff-merge); regen dashboard

Co-authored-by: claude <claude@organized-koala.local>"
```

### 7. Tear down the worktree + branch

```sh
git worktree remove .claude/worktrees/NNNN-<slug>
git branch -d <branch>
```

Use `-d` (not `-D`): after the ff-merge the branch is fully contained in `main`, so `-d`
succeeds and doubles as a safety check that nothing was left unmerged. If `-d` refuses, **stop
and investigate** — do not force-delete. Remove any `backup/finalize-NNNN` branch from step 2
only once the merge is confirmed.

### 8. Report — and hand the push back to the human

Summarize: the item merged, the new `main` head, the audit result (commits clean, or what was
repaired), the freshen outcome (current / re-rebased code-identical), and confirmation that the
worktree + branch are gone. **Do not push.** Remind the operator that `git push` (the only step
that touches the remote) is theirs to run.

## Hard rules

- **Never `git push` or run any remote-mutating command** — enforced by the permission
  deny-list and reasserted here. Reading the remote (`git fetch`, `git log origin/...`) is fine.
- **Fast-forward only** — never create a merge commit or squash; rebase first so the merge is a
  fast-forward (`git-standards`).
- **Never merge on a stale verdict** — if step 3's rebase changed the code-hash, or step 4 finds
  a red gate, STOP. The verdict must pin to the live code-tree hash (CLAUDE.md "Verdict
  pinning").
- **Never guess an agent identity** when repairing a trailer — fix only the deterministic case
  (author *is* an agent); otherwise surface it.
- **`-d`, not `-D`, for branch deletion** — let git refuse if the branch is not fully merged.

## Extending this skill

Living document — `eng-manager` appends durable finalize/merge learnings + human feedback here,
the same as the other workflow skills.
