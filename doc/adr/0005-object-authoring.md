# ADR-0005 — On-device object authoring (create / delete / move / resize / properties)

- Status: Accepted
- Date: 2026-06-16

## Context

Roadmap Phase 2, building on the script editor ([ADR-0004](0004-in-app-script-editor.md)). The
user can now edit an object's HyperTalk on-device; next they need to **build** cards: add and
remove buttons/fields, position and size them, and set their basic properties (name, title/text,
button style, locked). This is the larger half of authoring and the first feature that *mutates
the card's object set*, not just an object's contents.

Design questions:

1. **How to move/resize.** A numeric inspector (type x/y/w/h) is simplest and reuses the dialog
   pattern, but it is not how HyperCard feels. Direct manipulation (drag the object, drag a
   corner) is tactile and expected for a HyperCard-like tool.
2. **How to carry properties across the bridge.** Adding a typed JNI call per property
   (name/title/style/locked/…) bloats the surface and ossifies it; the set will grow in Phase 3.
3. **Where new objects live.** Card layer vs background layer.
4. **Keeping the bridge event-driven.** A drag emits a continuous stream of positions; the JSON
   bridge ([ADR-0002](0002-json-string-jni-bridge.md)) is for occasional, not per-frame, traffic.

## Decision

- **Direct manipulation on the canvas.** Edit mode gains *selection*: a tap selects the topmost
  object (highlight + a corner resize handle). Dragging the body moves it; dragging the handle
  resizes it. The script editor, previously opened by an edit-mode tap, now opens from the
  inspector instead (tap is reassigned to select).
- **Drag is local; commit on release.** `CardView` tracks a *draft rect* during `ACTION_MOVE`
  and repaints itself — **no bridge calls mid-drag**. Only on `ACTION_UP` does it call
  `set_object_rect` once. This preserves the event-driven boundary.
- **Properties travel as a JSON blob.** One pair of calls — `get_object_props(id)` returns
  `{id,kind,name,title|text,style|locked,x,y,w,h}`; `set_object_props(id, json)` applies whatever
  keys are present (ignoring unknowns). This keeps the JNI surface tiny and forward-compatible
  (Phase 3 properties become new keys, not new calls). Geometry keeps a **dedicated typed
  `set_object_rect`** because it is the drag hot-path and benefits from not building JSON.
- **New objects are created on the current card** (not the shared background), with a default
  rect and a generated unique id (max existing id across all layers + 1). Authoring the
  background layer is possible (you can select/move/delete background objects that hit-test
  first) but creation targets the card to avoid surprising cross-card edits.
- **Core surface** (mirrors the existing by-id mutation style): `add_object(kind) -> Option<id>`,
  `delete_object(id) -> bool`, `set_object_rect(id,x,y,w,h) -> bool`, `get_object_props(id)`,
  `set_object_props(id, json) -> bool`. **Bridge:** `nativeAddObject`, `nativeDeleteObject`,
  `nativeSetObjectRect`, `nativeGetObjectProps`, `nativeSetObjectProps`. **Host:** `CardView`
  selection + drag/resize and a `MainActivity` edit palette (New Button / New Field / Properties /
  Script / Delete) plus a property inspector dialog.

`set_object_rect` enforces a minimum size so an object can't be resized to zero/negative.
Persistence is unchanged — new/edited objects are part of `to_json`.

## Consequences

- **Positive:** authentic HyperCard authoring feel; a card can be built from scratch on-device;
  the bridge grows by five calls but the property channel won't grow again per-property; drag
  stays off the bridge so it's smooth and the event-driven contract holds.
- **Positive:** the by-id locate-across-layers pattern is now shared by script, rect, and props
  mutations — one mental model.
- **Negative / limits:** selection is single-object; no multi-select, grouping, undo, z-order
  reordering, or alignment guides yet. The resize handle is a single bottom-right corner (not all
  eight). New objects always start at one default position (may overlap; user drags them apart).
  Editing a background object silently affects every card sharing it — accepted for authoring,
  flagged in the palette title.
- **Follow-on:** undo/redo and z-order are natural next steps; `set_object_props` can absorb
  `visible`, `loc`/`rect` aliases, and text styling as Phase 3 lands them.
