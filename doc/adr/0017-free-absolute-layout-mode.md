# ADR-0017 — `free` (absolute) layout mode, and the default for un-laid-out cards

- Status: **Accepted** — implemented (slice 5); **amended** so `free` is the *default* for a card
  with no `layout` overlay (was: flat column), so native mirrors classic.
- Date: 2026-06-17
- Related: [ADR-0014](0014-layout-model-group-containers.md)/[ADR-0016](0016-grid-layout-and-card-layout-scripting.md)
  (the layout overlay this extends), [ADR-0008](0008-native-view-rendering.md) (whose
  "no geometry crosses outward" rule this *intentionally* relaxes — as an opt-in escape hatch).

## Context

The native target deliberately omits geometry (ADR-0008): the host owns layout, and slices 2/4 gave
it declarative `row`/`column`/`grid`. But existing stacks were authored with **absolute rects** and a
fixed letterboxed card; rendering them natively currently stacks everything in a column, which looks
wrong. The dialect names **`free`** as the "keep the classic feel" escape hatch. This slice ships it.

## Amendment (default for un-laid-out cards)

Originally `free` was an opt-in escape hatch and a card with **no** `layout` overlay rendered as a
flat column. In practice that made native mode diverge sharply from classic — an
absolutely-authored 2-column card became a 1-column stack — which read as broken. So the default is
flipped: **a card with no `layout` overlay now renders as `free`** (every object at its authored
rect), so native looks like classic *with real Material widgets*. Authors **opt into** responsive
layout (`column`/`row`/`grid`) by adding a `layout` overlay. Consequences: ADR-0008's "no geometry
crosses outward" guardrail now holds only for the **declarative** modes (column/row/grid — verified
by `non_free_layout_still_omits_geometry`); the default and `free` intentionally emit card-unit
geometry (the host still owns the unit→dp mapping).

## Decision

Add a card-level **`free`** layout mode. When a card's layout root has `mode == "free"`,
`render_view_tree` emits each object node **with its authored card-unit geometry** (`x`/`y`/`w`/`h`
props), and `ViewTree` carries the card `width`/`height`. The host renders a `BoxWithConstraints`,
computes a fit scale (`min(availW/cardW, availH/cardH)`, like `CardTransform`), and places each
object by `offset(x·s, y·s)` at `size(w·s, h·s)` dp.

- **Geometry is intentional only in `free` mode.** The slice-1 "omits geometry" guardrail still holds
  for `column`/`row`/`grid` (tested by `non_free_layout_still_omits_geometry`); `free` is the single
  documented exception, gated on the mode string. The core still emits abstract *card units*, not
  device pixels/dp — the host owns the unit→dp mapping, so the platform-agnostic boundary holds in
  spirit (the host could map card units to anything).
- Reachable from script via `set the layout of this card to "free"` (ADR-0016).

## Consequences

- **Positive:** existing absolutely-authored stacks render in native mode looking like the Canvas
  player — real Material widgets at their authored positions. A smooth migration path: flip a card to
  `free` to get Material affordances without re-authoring layout, then move to declarative later.
- **Positive:** small — geometry is appended in one place (`object_node` when `geometry` is set), and
  the host adds one `BoxWithConstraints` branch.
- **Negative / by design:** `free` does **not** reflow — it's fixed-aspect absolute placement, the
  thing responsive layout exists to replace. It's an escape hatch, not the recommended mode; the
  dialect's north star is still declarative. Nested groups under `free` aren't meaningful (free is
  flat); the scripted builder produces a flat child list.

## Non-goals

Per-object responsive behavior within `free`; mixing `free` subtrees inside declarative ones;
emitting device pixels/dp from the core (the host keeps owning the unit mapping).
