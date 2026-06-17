# ADR-0014 — Layout model: group containers for the native render target

- Status: **Accepted** — implemented (slice 2 of the native render target).
- Date: 2026-06-17
- Related: [ADR-0008](0008-native-view-rendering.md) (native-view rendering — this is its layout
  slice), [ADR-0011](0011-yaml-stack-files.md) (YAML authoring), [ADR-0012](0012-uniffi-bridge.md)
  (typed bridge), and the [Android-native dialect vision](../design/android-hypertalk-dialect.md)
  ("Layout — responsive dp, not absolute letterboxed pixels").

## Context

ADR-0008 slice 1 shipped a native (Compose Material 3) render target, but it stacks a card's
objects in a flat full-width column — it ignores how the card is arranged. The dialect vision calls
for **responsive layout** (`set the layout of this card to "column" | "row" | "grid" | …`,
per-object `weight`, group `padding`) replacing absolute letterboxed pixels. This ADR adds the
first, foundational piece: **nested group containers** (column/row) so native mode reflows into a
real grid (e.g. the "Counters" card becomes a positioned column-of-rows).

The hard constraint from ADR-0008 still holds: `hypercore` stays platform-agnostic; **no
geometry/pixels/dp cross the boundary** — layout is expressed declaratively (mode/weight/padding,
unitless), and the host maps it to dp.

## Decision

Add layout as an **additive overlay on the model**, not a restructuring of it:

- Objects keep living in `card.buttons` / `card.fields` by id (unchanged). A card gains an
  **optional** `layout: Option<LayoutGroup>` — a tree that only **references objects by id** and
  defines their nesting/arrangement. `None` ⇒ the native renderer falls back to the slice-1 flat
  column; the Canvas renderer **always** ignores `layout` and uses each object's absolute `rect`.
- `LayoutGroup { mode: "column"|"row", padding: f32, weight: f32, children: Vec<LayoutChild> }`.
- `LayoutChild` is an **untagged** enum: a nested `Group` (a map) or an `Object(u32)` (a bare id).
  The two forms are structurally disjoint (map vs number), so it reads cleanly as
  `children: [10, 20, { mode: row, children: [...] }]` and round-trips in **both** serde_json and
  yaml_serde. (An *externally*-tagged enum was rejected: yaml_serde emits it as a `!group` YAML
  tag, which doesn't match the JSON map form — they wouldn't share one readable representation.)
- Per-object flex lives on the object: `Button.weight` / `Field.weight` (`#[serde(default)] = 0`),
  matching the dialect's `set the weight of field`. `weight` is also a scriptable property
  (`the weight of`); card-level `set the layout/padding of this card` scripting is **deferred**
  (the overlay tree doesn't map to a scalar setter).

Everything is `#[serde(default)]`/additive: existing stacks load and render unchanged in both modes.

### Render contract (ADR-0008 view tree)

`render_view_tree` projects the overlay into the existing flat tree: `ViewTree` gains `layout` +
`padding` (the root container); a `LayoutGroup` becomes a `ViewNode { kind: "group", props:
mode/padding/weight, child_ids }`; an object ref reuses the slice-1 object node plus a `weight`
prop. **Group node ids are synthetic** — allocated from `max(object id on card+bg) + 1`,
monotonically — so they never collide with real ids and, being non-interactive, are inert if ever
dispatched (`dispatch_by_id` only resolves real objects). The desktop `tree` REPL command (already
recursive over `child_ids`) prints the nesting, proving it headlessly with no Android.

### Host realization

The Compose host (`NativeCardScreen`) renders the tree as mutually-recursive `Container`
(Column/Row by `mode`, with `padding`) and `RenderNode`. `Modifier.weight()` is legal only inside a
`Row`/`Column` scope, so the weight modifier is **computed inside each container's lambda** and
threaded into the child; `weight == 0` ⇒ natural size (`fillMaxWidth` for column cells). A
scrollable root column never applies a vertical weight (it would conflict with `verticalScroll`).

## Consequences

- **Positive:** native mode reflows into real responsive grids; the Counters card renders as a
  column of weighted `[label, −1, Reset, +1, count]` rows. Verified on device; the headless `tree`
  shows the nesting in CI.
- **Positive — tiny blast radius:** because layout is an overlay referencing objects by id, the
  Canvas renderer, `hit_test`, `collect_path`, `locate_*`, `set_field_text`, and `dispatch_*` are
  **all untouched**. Only `render_view_tree` and the Compose host read the overlay.
- **Caveat — unreferenced objects:** in grouped native mode, only objects referenced by the layout
  tree are shown; an object the author forgets to reference won't appear (Canvas mode still shows
  everything). A dangling ref to a non-existent id is skipped silently. Documented; authors should
  reference every object they want laid out.
- **Negative:** a card now has two sources of arrangement — absolute `rect`s (Canvas) and the
  `layout` overlay (native). They can drift; keeping both is the cost of supporting both targets.

## Non-goals (later slices/ADRs)

`grid` / `constraints` layout modes; a `free`/absolute compat mode (emitting geometry into the
tree); dp/insets/safe-area anchors; card-level `set the layout/padding of this card` scripting;
Material roles/`textRole`/dynamic-color/theme; new object kinds; lifecycle messages.
