---
id: 0010
title: Empty-string query params bypass the {code,message} JSON error contract
status: open          # open | accepted | closed   (only the human flips open → accepted/closed)
priority: low         # high | medium | low — a triage hint, never a gate
created: 2026-07-02    # absolute date
source: 0020          # Board item / cycle that surfaced it, or "adhoc"
raised-by: verifier   # reviewer | verifier | eng-manager | architect | orchestrator | human
promoted-to: null     # NNNN of the resulting Board item, set when accepted & promoted
---

## What

On the task-list endpoint added in 0020, an **empty-string** query value —
`GET …/tasks?limit=` or `?limit=&offset=` — returns `400` with a **plain-text body**
(`Failed to deserialize query string: limit: cannot parse integer from empty string`), which is
axum's built-in `Query` extractor rejection. That plain-text response **bypasses the standard
`{ "code": <app-error-code>, "message": <string> }` JSON error contract** the rest of the API
honours. This is distinct from the two cases 0020 does handle to contract: *absent* params (no
query string → `200`, whole list) and an over-ceiling value (`?limit=501` → `400` JSON
`{"code":"validation_failed", …}`).

## Why it matters

It is a small consistency gap in the error contract: a malformed query value produces a
non-conforming error body a TUI cannot match on `code`. Impact in practice is negligible — the
**shipped reqwest client never emits empty-string params** (it sends real integers, `limit=200`
/ `offset=0`), so no real client reaches this path; it is only hit by a hand-crafted URL. Out of
scope for 0020 (which verified `verified`) because closing it is a general axum-extractor error
-mapping concern, not part of the tasks-pane overhaul, and it touches how the server maps
extractor rejections across *all* endpoints, not just this one.

## Possible approach

Non-binding sketch: add a custom `Query` rejection handler (or a wrapper extractor) that maps
axum's `QueryRejection` into the standard `{code: "validation_failed", message}` JSON body, so
malformed query params everywhere return the contract shape. Would want a server-dev slice + a
test pinning the JSON body for a malformed param; no contract/DTO change (the error *shape* is
already the contract).

## Disposition

- [ ] <ts> [human] decision: accept (→ promote to Board) | close (reason)
