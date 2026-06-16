# ADR-0003 — Player-first; stacks authored as JSON

- Status: Accepted
- Date: 2026-06-16 (records a decision made at project inception)

## Context

HyperCard is both a player (browse mode) and an authoring environment (create objects, draw,
edit scripts). Building both at once is a large surface. We needed an MVP that demonstrates the
engine end-to-end without committing to an on-device authoring UI first.

## Decision

Ship a **player first**. Stacks are authored **outside the app as JSON files** (`model.rs`
structs are serde-(de)serializable) and loaded by the host. On Android, `MainActivity` loads a
saved stack from `filesDir/stack.json` if present, otherwise a bundled asset
(`productivity.json`), and writes `Session::to_json()` back on pause.

The render-list / host-command bridge ([ADR-0002](0002-json-string-jni-bridge.md)) was designed
so authoring can be layered on later **without reworking it**.

## Consequences

- **Positive:** a small, shippable surface that exercises the model, the interpreter, and the
  bridge; content can be created and tested entirely via JSON + the `hyper-desktop` REPL.
- **Positive:** authoring becomes an additive series of steps (see the roadmap), starting with
  script editing ([ADR-0004](0004-in-app-script-editor.md)) rather than a big-bang editor.
- **Negative:** until authoring lands, "writing a script" or "adding a button" means editing
  JSON by hand — not friendly to non-developers.
- **Constraint:** the current card index is **not** persisted (a stack reopens at card 1), and
  fields render single-line. These are tracked as roadmap Phase 4, not bugs.
