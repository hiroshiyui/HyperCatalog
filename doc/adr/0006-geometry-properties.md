# ADR-0006 — HyperTalk geometry properties (loc / rect / width / …)

- Status: Accepted
- Date: 2026-06-16

## Context

Roadmap Phase 3 fills documented interpreter gaps. Before this change, `get`/`set the <prop>
of <object>` covered only `name`, `title`/`text`, `visible`, and `locked` — a script could read
and write an object's *contents* but not its *geometry*. With on-device object authoring now
shipped ([ADR-0005](0005-object-authoring.md)), the obvious next gap is letting **scripts** do
what the authoring UI does: move, resize, and show/hide objects. The model already stores a
`Rect { x, y, w, h }` per object, so geometry needs no new data — only interpreter plumbing.

The Phase 3 wishlist also names `textStyle`/`textSize`/`textFont`. Those have **no backing model
fields** and the renderer draws a fixed 16px style, so supporting them means new model fields
plus render work — a separate change, deferred here.

## Decision

Add geometry properties to `get_property`/`set_property` for **buttons and fields**, plus a
read-only `id`. HyperTalk semantics:

| Property            | Get returns                | Set effect                                   |
|---------------------|----------------------------|----------------------------------------------|
| `loc` / `location`  | center point `"h,v"`       | re-centers, keeping size                     |
| `rect`/`rectangle`  | `"left,top,right,bottom"`  | sets all four edges                          |
| `width` / `height`  | number                     | resizes, keeping the top-left corner         |
| `top` / `left`      | number                     | moves that edge (object keeps its size)      |
| `bottom` / `right`  | number                     | moves so that edge lands on the value        |
| `id`                | number                     | read-only                                    |

Implementation notes:

- Two free helpers in `interp.rs`: `geom_get(prop, rect) -> Option<Value>` and
  `geom_set(prop, &mut rect, &value) -> bool`. The property `match` arms fall through to these;
  `geom_set` returning `false` means "not a geometry property", so the caller still raises
  *unknown property* for genuine typos. Get falls through to `Value::Empty` (HyperTalk's missing
  value), matching the prior behavior for unknown gets.
- `set` of `rect`/`width`/`height` clamps to a 1px minimum (`MIN_GEOM_SIZE`) so a script can't
  zero or invert a rect and break hit-testing/rendering.
- **`width`/`height` keep the top-left corner** rather than re-centering. HyperCard re-centers;
  we chose corner-stable because it's more predictable against the top-left `Rect` model and
  matches the authoring drag-resize handle. Documented so it's a choice, not a surprise.
- Malformed coordinate strings (`set the rect ... to "oops"`) are ignored as a no-op, matching
  HyperTalk's lenient `set`.
- Numbers format through `Value::Number` (integers print without a trailing `.0`), so `the loc`
  reads back as `"60,65"`, not `"60.0,65.0"`.

This is interpreter-only: no model, bridge, or host changes. The `field_props` helper, now
redundant, was removed and the field-get path reads the field inline like the button path.

## Consequences

- **Positive:** scripts can now move/resize/show objects (`set the loc of button "x" to the
  loc of me`, animation-by-steps, responsive layout in a handler), and read geometry for
  calculations. Synergizes with authoring — a script can finish what a drag started.
- **Positive:** no new bridge calls or JSON contract changes; persistence already covers rects.
- **Negative / limits:** no `textStyle`/`textSize`/`textFont` yet (needs model + render fields);
  no per-edge composite like `the topLeft`; card/stack have no geometry (unchanged). `width`/
  `height` corner-stable behavior differs from classic HyperCard's centered resize.
- **Follow-on:** text styling is the next Phase 3 property step; custom-message dispatch up the
  path and `visual effect` remain separate Phase 3 items.
