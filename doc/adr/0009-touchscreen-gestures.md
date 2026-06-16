# ADR-0009 — Touchscreen gestures as bubbling HyperTalk messages

- Status: Accepted
- Date: 2026-06-16
- Related: [ADR-0002](0002-json-string-jni-bridge.md) (the bridge this adds one call to),
  [ADR-0010](0010-modern-ui-considerations.md) (the post-WIMP principles this realizes), and the
  [Android-native dialect vision](../design/android-hypertalk-dialect.md).

## Context

HyperCard's input vocabulary is mouse-shaped: `mouseUp`, `mouseDown`, `mouseStillDown`. On a
phone the dominant input device is a finger, and its idioms — long-press for context actions,
swipe for navigation, double-tap — have no mouse equivalent. HyperCatalog dispatched exactly one
message, `mouseUp`, from `dispatch_touch`. To feel native on a touchscreen the player needs
first-class **gestures**, without abandoning the existing tap semantics that current stacks and
the field editor rely on.

Two facts made this cheap rather than a rewrite:
- The parser already accepts **any** word as a handler name, so `on longPress` / `on swipeLeft`
  parse today with no grammar change.
- `dispatch_message` already **bubbles** a message object → card → background → stack.

## Decision

Add one core entry point, **`Session::dispatch_gesture(x, y, gesture)`**, that sends `gesture`
(`tap`, `doubleTap`, `longPress`, `swipeLeft`/`swipeRight`/`swipeUp`/`swipeDown`) as a HyperTalk
message:

- It targets the **object under the gesture's start point** (a `Me`, regardless of lock state)
  and then **bubbles** the same path as every other message — so a stack-level `on swipeLeft`
  catches a swipe made anywhere, while an object can intercept its own. Matching is
  case-insensitive; an **unhandled gesture is a no-op** (no error, no redraw effect).
- A gesture **never opens the field editor.** Long-pressing or swiping an unlocked field runs
  script; only a plain tap (still `dispatch_touch` → `mouseUp`) returns `focus_field` to focus it.
- `mouseUp` stays the canonical "click/tap" for backward compatibility; gestures are **additive**.

Across the bridge (per [ADR-0002](0002-json-string-jni-bridge.md)): one new method
`nativeDispatchGesture` returning the same `DispatchResult` JSON. The **host owns gesture
recognition** — `CardView` feeds touches to an Android `GestureDetector` that fires long-press,
double-tap, and fling (classified into a swipe by dominant axis past a small threshold), maps the
point to card space via `CardTransform`, and calls the bridge. A `gestureConsumed` flag suppresses
the trailing tap when a richer gesture already handled the sequence.

### Choices and their reasons

- **Gesture as a message string, not a typed enum.** Keeps the core dialect open (a host can send
  any gesture token), needs no parser/AST change, and matches HyperTalk's stringly nature.
- **Recognition on the host, not the core.** Fling thresholds, double-tap timeouts, and touch-slop
  are platform UX concerns; the platform-agnostic core only receives the *classified* gesture and
  a card-space point. The desktop host can synthesize gestures for testing without an Android
  detector.
- **Bubble from the start-point object.** Mirrors HyperCard's path so authoring is consistent
  (`on longPress` on a button; `on swipeLeft` on the stack); no separate routing rules to learn.
- **Tap ≠ a second message.** Firing both `mouseUp` and `tap` for one tap would double-dispatch;
  `mouseUp` remains the tap. `tap` is accepted by `dispatch_gesture` for hosts that prefer it, but
  the reference host does not emit it.

## Consequences

- **Positive:** stacks become touch-native — swipe-to-navigate, long-press menus — purely in
  HyperTalk, with no new grammar. Fully backward compatible: existing `mouseUp` stacks are
  unchanged, and gestures they don't handle are silent no-ops.
- **Positive:** the core stays platform-agnostic and headless-testable; gesture dispatch is
  covered by `hypercore` unit tests (long-press runs an object handler; swipe bubbles to the
  stack and navigates; unhandled gesture is a no-op; a gesture never focuses a field).
- **Negative / limits:** only discrete gestures, no continuous ones (pan/drag deltas, pinch-zoom,
  multi-touch) — those would need a richer, possibly streaming event channel, which strains
  ADR-0002's event-driven (not per-frame) assumption. `mouseDown`/`mouseStillDown` are still not
  modeled. Gesture recognition lives only in the Android host; another host must supply its own.
- **Gotcha:** a double-tap also fires one `mouseUp` (from the first tap) before `doubleTap`; the
  `gestureConsumed` flag only suppresses the *second* tap. Documented for script authors.
