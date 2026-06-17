# ADR-0021 — Native component palette (checkbox/radio/slider/progress/image/chip/divider)

- Status: **Accepted** — implemented (Phase 6).
- Date: 2026-06-17
- Related: [ADR-0015](0015-switch-object-kind.md) (the `switch` precedent this generalizes),
  [ADR-0008](0008-native-view-rendering.md), and the dialect's "richer object taxonomy than
  button/field".

## Context

HyperCard had two widgets; the dialect wants a palette. `switch` (ADR-0015) shipped as "a `Button`
with `checked`". This ADR extends the taxonomy to the rest of the common Material controls —
**checkbox, radio, slider, progress, image, chip, divider** — under one discriminator, keeping the
additive Design-B pattern (variant fields on `Button`, never a model explosion of distinct objects).

## Decision

Add a single **`Button.control: String`** discriminator (`#[serde(default)]`): `""` (plain button,
or legacy `switch` when `checked` is set) | `checkbox` | `radio` | `slider` | `progress` | `image` |
`chip` | `divider`. Plus two state fields: **`value: Option<f32>`** (0..=1, for slider/progress) and
**`source: String`** (image). Boolean controls reuse the shipped **`checked: Option<bool>`**.

- **Projection** (`session.rs` `button_node`): node `kind` = `control` when set, else `"switch"`
  when `checked.is_some()` (legacy, unchanged), else `"button"`. Emits `checked`/`value`/`source`
  props as relevant. The Canvas target (`button_cmd`) shows a terse textual stand-in (`☐`, `───`,
  `[40%]`, `[img: …]`) — retro, no `CardView` change.
- **Dispatch**: boolean controls reuse `toggle_if_switch` (it keys on `checked.is_some()`, so
  checkbox/radio auto-toggle exactly like switch). Sliders push their dragged value to the core via
  a new `set_value(id, v)` bridge call (clamped 0..=1, mirroring `set_field_text`) and dispatch
  `mouseUp` on release.
- **Host** (`NativeCardScreen.kt`): one `when (node.kind)` branch each → `Checkbox`/`RadioButton`/
  `Slider`/`LinearProgressIndicator`/`Image`/`FilterChip`|`AssistChip`/`HorizontalDivider`, reusing
  `nodeTextStyle`/the dispatch pattern. `image` loads a **bundled asset** by name (remote URLs are
  Phase 10); a missing asset degrades to a labelled placeholder.
- **Scriptable** (`interp.rs`): `the value of`, `the control of`, `the source of` (get/set), beside
  the existing `the checked of`.

## Consequences

- **Positive:** the full common palette for ~one model field + small per-kind host branches; the
  `control` discriminator scales to future kinds without touching the dispatch/hit-test/iteration
  machinery (it's still a `Button`). Existing switches and plain buttons are unchanged (legacy
  `checked`-without-`control` still means switch).
- **Caveat — radio groups:** mutual exclusion is the author's script responsibility for now (each
  radio's handler clears its siblings); a first-class `group` for radios is deferred.
- **Caveat — image:** local assets only; remote URLs need the async facilities (Phase 10). No
  bundled image ships yet, so the demo shows the placeholder path until one is added.
- **Caveat — slider range:** fixed 0..=1 for now; custom `min`/`max` deferred.

## Non-goals (later)

Radio groups, remote/async images (Phase 10), chip sub-type taxonomy beyond filter/assist, slider
ranges/steps, and app-bar/list/dialog kinds.
