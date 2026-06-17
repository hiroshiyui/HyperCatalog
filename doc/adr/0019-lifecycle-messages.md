# ADR-0019 — Activity-lifecycle messages

- Status: **Accepted** — implemented (slice 7).
- Date: 2026-06-17
- Related: [ADR-0009](0009-touchscreen-gestures.md) (gestures-as-messages, the same "host fires a
  named message that bubbles the path" pattern), [ADR-0010](0010-modern-ui-considerations.md)
  ("no busy-loop; lifecycle drives the UI"), and the dialect's `on resume`/`suspend`/`backPressed`/`rotate`.

## Context

HyperCard's `idle` busy-loop is a battery anti-pattern on mobile. The dialect replaces it with
**explicit Activity-lifecycle messages** routed through the existing message path: the host fires a
named message at each lifecycle transition, and scripts handle it like any other message.

## Decision

Add `Session::dispatch_lifecycle(message)` (bridge `dispatchLifecycle`) — it sends `message` with
**no object origin**, so it bubbles card → background → stack (a stack-level `on resume` catches it),
reusing `dispatch_message`. `DispatchResult` gains a **`handled`** flag (whether a handler matched
and ran), so the host can decide whether to consume an event.

The host (`MainActivity`) fires:
- `onResume` → **`resume`**, `onPause` → **`suspend`** (before the existing save),
- `onConfigurationChanged` → **`rotate`** (the activity declares `android:configChanges` so it isn't
  recreated), and
- a system-back `OnBackPressedCallback` → **`backPressed`**: if a handler ran (`handled`), the back
  is consumed; otherwise it falls through to the platform default (finish/back-stack).

`idle` is intentionally **not** fired. The desktop REPL gains a `fire <message>` command to drive
lifecycle (and arbitrary) messages headlessly.

## Consequences

- **Positive:** stacks can react to lifecycle (`on resume` to refresh, `on suspend` to persist,
  `on backPressed` to intercept back / confirm exit, `on rotate` to reflow) — entirely additive and
  reusing the message path; no new runtime, no `idle`. The `handled` flag is generally useful
  (e.g. future "did anything consume this gesture?").
- **Positive:** verified that a stack *without* an `on backPressed` keeps default back behavior
  (handled = false → fall through), so existing stacks are unaffected.
- **Caveat:** defining `on backPressed` makes the stack own back entirely (any matching handler
  consumes it) — HyperCard-ish interception, but authors must remember to provide an exit path.
  `rotate` doesn't carry the new size yet (the dialect's `on rotate w, h` params are a follow-on;
  handler args aren't wired through `dispatch_message` yet).

## Non-goals

`on rotate w, h` parameters (needs typed message args), `permissionResult`/`saveState`/`restoreState`,
and any `idle`/timer message.
