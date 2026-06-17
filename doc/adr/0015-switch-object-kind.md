# ADR-0015 — The `switch` object kind (a button with toggle state)

- Status: **Accepted** — implemented (slice 3 of the native render target).
- Date: 2026-06-17
- Related: [ADR-0008](0008-native-view-rendering.md) (native rendering — switches show as Material
  `Switch`es), [ADR-0010](0010-modern-ui-considerations.md) (the richer object taxonomy), and the
  [dialect vision](../design/android-hypertalk-dialect.md) (`the checked of`, `switch`/`checkbox`/`radio`).

## Context

HyperCard had two widgets (button, field); the Android dialect wants a palette
(switch/slider/chip/checkbox/radio/image/…). This ADR adds the first new kind, **`switch`** — a
Material toggle holding a boolean — and establishes the pattern for the rest.

A new object kind could be modelled two ways. Exploration of the blast radius was decisive:

- **Distinct kind** (`card.switches: Vec<Switch>`, a `Me::Switch` variant): forces new branches in
  **~14–16 `session.rs` functions** (render, hit-test, collect_path, me_for_id, add/delete/move,
  object_props, next_id, …) plus new `Me`/`ResolvedObj` variants — ~200–300 LOC.
- **Button variant** (`Button.checked: Option<bool>`): touches **~5 files / ~80 LOC**; iteration,
  dispatch, hit-test, and id namespace are all reused unchanged.

A switch *is* a button with toggle state: same id space, same `mouseUp`, same script dispatch, same
text styling. The only real differences are how it renders and that it carries a boolean.

## Decision

Model a switch as a **button with `checked: Option<bool>`** (`#[serde(default)]`): `None` = a plain
button, `Some(b)` = a switch. Additive — existing stacks are unaffected.

- **Auto-toggle in the core:** on a switch's `mouseUp` (both `dispatch_touch` and `dispatch_by_id`),
  flip `checked` **before** running the handler, via `toggle_if_switch(id)`. So a script-less switch
  still toggles, and a handler reading `the checked of me` sees the new state. Toggle state lives in
  the model (the source of truth); the host re-reads it on the next render — no host-side toggle
  state to drift.
- **`the checked of`** is a scriptable button property (get/set), mirroring the existing `textsize`
  etc. arms — one line each, no new enum variants.
- **Render:**
  - Native (view tree): `button_node` emits `kind:"switch"` + a `checked` prop when `checked.is_some()`;
    the Compose host renders a Material 3 `Switch` (label + toggle in a `Row`), whose `onCheckedChange`
    dispatches `mouseUp` by id (the core does the actual toggle).
  - Canvas: `button_cmd` prefixes the label with ☑/☐ — no `CardView`/`DrawCmd` change.
- **Inspector:** `ObjectProps` gains `checked`; the inspector shows a "Checked" box for switches.
  `apply_button` only writes `checked` when the object is already a switch (the inspector can't turn
  a plain button into a switch — that's an authoring concern for later).

## Consequences

- **Positive:** a real, accessible Material toggle in native mode for ~80 LOC; reuses all of the
  button machinery (dispatch, hit-test, id namespace, text styling). The auto-toggle-in-core keeps a
  single source of truth and works identically for Canvas taps and native dispatch.
- **Positive — the pattern generalizes:** the next kinds (checkbox/radio are the same boolean shape;
  slider = a button with a numeric `value`; image = a `source`) follow this "variant field on an
  existing object, projected to a distinct view-tree `kind`" recipe, avoiding the model explosion of
  distinct kinds.
- **Negative / caveat:** overloading `Button` means "button" is now a small union; a far-future,
  genuinely different kind (e.g. a canvas/drawing widget) may still warrant a distinct object. The
  inspector can't yet *create* a switch (only edit an authored one).

## Non-goals (later slices)

`checkbox`/`radio` (same shape, trivial follow-ons), `slider` (numeric `value`), `image` (`source`),
and the rest of the taxonomy; creating switches via the authoring palette; tri-state/indeterminate.
