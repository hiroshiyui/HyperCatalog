# ADR-0004 — In-app HyperTalk script editor

- Status: Accepted
- Date: 2026-06-16

## Context

Roadmap Phase 1: let a user edit the HyperTalk on **existing** objects from the device, the
first step of on-device authoring ([ADR-0003](0003-player-first-json-authored-stacks.md)). Two
problems to solve:

1. **Tap conflict.** In browse mode a tap *runs* an object's script (or opens a field editor).
   For authoring, a tap must instead *select* the object to edit. The same gesture cannot mean
   both.
2. **Bad scripts.** A user can type HyperTalk that does not parse. Saving an unparseable script
   would make the object silently dead (its handler never matches), with no feedback.

Because scripts are stored as source and parsed lazily per dispatch
([ADR-0001](0001-rust-native-hypertalk.md)), writing a new script string is sufficient — there
is no recompile step, and the next tap runs the new code.

## Decision

Add a **dedicated edit-mode toggle** (a host control), rather than overloading a long-press or
auto-opening the editor on tap. Rationale: an explicit mode keeps browse behavior unchanged and
unsurprising, makes "I am authoring now" visible, and leaves the long-press gesture free for
future object operations (Phase 2). When edit mode is on, a tap selects the topmost object at
that point and opens a **multi-line script editor** pre-filled with its current source.

Extend the bridge with the minimum surface, mirroring the existing `set_field_text` path:

- Core (`hypercore::session::Session`):
  - `object_at(x, y) -> Option<u32>` — topmost object id at a card-space point (reuses the
    existing hit-test traversal; ignores lock state, unlike `dispatch_touch`).
  - `get_object_script(id) -> Option<String>` / `set_object_script(id, &str) -> bool` —
    read/write an object's `script` by id, searching card layer then background layer.
  - `check_script(&str) -> Option<String>` — `Some(error)` if the source fails to parse, else
    `None`. Thin wrapper over `script::parse_script`.
- JNI (`hyperffi/src/android.rs`) + `NativeBridge.kt`: `nativeObjectAt`, `nativeGetObjectScript`,
  `nativeSetObjectScript`, `nativeCheckScript`.
- Host (`MainActivity` / `CardView`): an "Edit/Done" toggle; in edit mode `CardView` resolves
  the tapped object via `nativeObjectAt` and raises `onEditScript(id)`; `MainActivity` shows an
  `AlertDialog` with a multi-line `EditText`. **Save validates via `nativeCheckScript` first**:
  on a parse error it keeps the dialog open and toasts the message; only valid scripts are
  written and the card is refreshed. Persistence is unchanged — `onPause` already saves scripts
  via `to_json`.

Scope is **buttons and fields** (the tappable, id-bearing objects). Editing card/background/
stack scripts, and creating/deleting/moving objects, are deferred to Phase 2.

## Consequences

- **Positive:** smallest useful authoring step; browse mode is untouched; invalid scripts can't
  be saved silently; no new JSON contract structs (the calls pass an id and/or a string).
- **Positive:** the validate-before-save check surfaces the parser's own error string, giving
  real feedback for free.
- **Negative / limits:** `object_at` returns only an id; if a button and a field on one card
  shared an id the search order (buttons before fields) decides — our authored stacks keep ids
  unique, so this is acceptable but worth noting. No syntax highlighting; the editor is a plain
  multi-line `EditText`. The edit toggle is a floating control, not yet a real toolbar.
- **Follow-on:** the same `object_at` + by-id mutation pattern generalizes to Phase 2 property
  edits (name/title/rect), so this bridge shape is forward-compatible.
