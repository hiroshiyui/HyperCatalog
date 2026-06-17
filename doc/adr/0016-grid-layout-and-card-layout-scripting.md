# ADR-0016 — `grid` layout mode and card-level layout scripting

- Status: **Accepted** — implemented (slice 4 of the native render target).
- Date: 2026-06-17
- Related: [ADR-0014](0014-layout-model-group-containers.md) (the layout overlay this extends),
  [ADR-0008](0008-native-view-rendering.md), and the dialect's
  `set the layout of this card to "… | grid | …"`.

## Context

ADR-0014 gave the native target nested `row`/`column` groups, authored in YAML. Two gaps remained
from the dialect's layout vision: a **`grid`** mode, and the ability to set layout **from HyperTalk**
(`set the layout of this card to "column"`), not just YAML.

## Decision

- **`grid` mode:** `LayoutGroup` gains `columns: u32` (`#[serde(default)]`); `ViewTree` gains
  `columns` for the root. A `mode:"grid"` container emits a `columns` prop; the **host chunks** its
  children into rows of `columns` equal-width cells (a `Column` of `Row`s, a short final row padded
  with `Spacer`s) — no `LazyVerticalGrid` (which is scroll/viewport-oriented and overkill for a
  static card grid).
- **Card-level layout scripting:** `the layout of this card` / `the padding of this card` are
  get/set in the interpreter's `ResolvedObj::Card` arm. `set the layout of this card to
  "row"|"column"|"grid"` builds (or replaces) a **single-level** root `LayoutGroup` over *all* the
  card's objects in render order (background then card), via `Runtime::set_card_layout`; `grid`
  defaults to 2 columns. The getter returns the root mode (or `""`). Nested authoring stays
  YAML-only — scripting a whole tree isn't worth the surface yet.

## Consequences

- **Positive:** authors get responsive grids both in YAML and at runtime; `set the layout of this
  card` is the dialect-faithful entry point and works on existing stacks (it wraps their objects).
- **Positive:** grid via chunking is ~15 lines in the host and composes with the existing
  weight/padding machinery; no new Compose dependency.
- **Caveat:** `set the layout of this card` flattens to a single level (all objects in render
  order) — it can't reproduce a hand-authored nested grid, and a card-level grid uses one global
  `columns`. Fine for the common "lay these out in N columns" case; richer structure is YAML.
- **Caveat:** like all overlays (ADR-0014), only referenced objects appear in native mode — but the
  scripted builder references *every* object, so nothing disappears when set from script.

## Non-goals

`constraints` mode and anchors (a later slice); per-cell grid spans/alignment; scripting nested
group trees; `the columns of this card` as a separate scriptable property (folded into the grid
default for now).
