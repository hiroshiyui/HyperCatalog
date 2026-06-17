# HyperCatalog Roadmap

HyperCatalog is a HyperCard-like UI player with a HyperTalk dialect implemented directly in
Rust (lexer → parser → AST → interpreter). The Rust core (`hypercore`) is platform-agnostic;
Android is the reference host, driven through a small, generated **UniFFI** bridge (ADR-0012).

This roadmap states where we are and the order we intend to grow. Decisions that shape the
architecture are recorded as ADRs under [`doc/adr/`](adr/).

## Where we are (shipped)

- **Stack player (browse mode).** Load a stack from JSON, render the current card, hit-test
  taps, run scripts along the HyperCard message path (object → card → background → stack),
  navigate between cards, edit field text through a host overlay, and persist on pause.
- **Touchscreen gestures.** `on tap`/`doubleTap`/`longPress`/`swipeLeft|Right|Up|Down`, dispatched
  as messages that bubble the same path. See [ADR-0009](adr/0009-touchscreen-gestures.md).
- **HyperTalk subset in Rust.** Handlers (`on mouseUp`/`openCard`/…), `put`/`get`/`set`,
  `go`, `answer`/`beep`, `add`/`subtract`/`multiply`/`divide`, `if`/`repeat`, full expression
  precedence, field/button/card/stack property get/set, and `length`/`random`/`trunc`. The
  supported surface is documented in `rust/README.md`.
- **UniFFI typed bridge.** The host drives a generated, typed `HyperStack` object (render,
  dispatch, gestures, authoring) — no hand-written JNI, no JSON on the wire. See
  [ADR-0012](adr/0012-uniffi-bridge.md) (supersedes the original JSON-over-JNI bridge,
  [ADR-0002](adr/0002-json-string-jni-bridge.md)).
- **Sample content** (YAML, ADR-0011). `assets/sample.yaml` (demo), `assets/productivity.yaml`
  (To-Do, Counters, Tip Split, Calculator, Temperature, Length — the default stack), and
  `assets/gestures.yaml` (a 3-card swipe/long-press/double-tap demo for the touch gestures).

## Direction

The arc is **player → script authoring → object authoring → broader HyperTalk**, smallest
useful step first, never breaking the player.

### Phase 1 — In-app script editing *(in progress)*

Let a user edit the HyperTalk on existing objects from the device, not just by hand-editing
JSON. An **edit-mode toggle** switches taps from "run the script" to "select & edit the
script"; a multi-line editor reads the object's current source, validates it through the
parser, and writes it back. No new object creation yet. See
[ADR-0004](adr/0004-in-app-script-editor.md).

*Enables: tweak a button's behavior on-device; immediate parse-error feedback.*

### Phase 2 — Object authoring *(in progress)*

Create / delete / move / resize buttons and fields; a minimal tool palette; set name, title,
style, and lock state. Direct manipulation: tap to select, drag to move, drag a corner to
resize; a property inspector for the rest. Reuses the render-list / host-command bridge that
was designed to accommodate it without a rewrite. See
[ADR-0005](adr/0005-object-authoring.md).

*Enables: build a card from scratch on-device.*

### Phase 3 — Broader HyperTalk coverage *(in progress)*

Fill documented interpreter gaps. **Done:** geometric properties — `loc`/`location`,
`rect`/`rectangle`, `width`/`height`, `top`/`left`/`bottom`/`right`, and read-only `id`
([ADR-0006](adr/0006-geometry-properties.md)); and text styling — `textFont`, `textSize`,
`textStyle` (bold/italic/underline), `textAlign` on buttons and fields, rendered and editable in
the inspector ([ADR-0007](adr/0007-text-styling.md)). **Remaining:** the message box UI,
custom-message dispatch up the path (today `Stmt::Send` of an unknown command is a no-op),
`visual effect`, and fuller `repeat`/`pass`/`return` semantics.

*Enables: scripts that move/resize/show objects and restyle text (done) and, later, send their
own messages.*

### Phase 4 — Persistence & rendering polish

Persist the current card index (today a stack reopens at card 1). Multi-line fields with wrap
and scrolling (today single-line). A stack picker / multiple stacks instead of one default
asset.

### Phase 5 — Android-native dialect *(north star)*

A longer-horizon reference target: a HyperTalk dialect whose primitives are Android's, not 1987
Mac's — Material components via **native-view rendering** (core emits a view tree, host builds
real Material Views/Composables), the Activity lifecycle as system messages, a responsive dp
layout system, and platform reach (permissions, intents, async, accessibility). It steers
decisions rather than describing shipped behavior. Full vision in
[`doc/design/android-hypertalk-dialect.md`](design/android-hypertalk-dialect.md).

**Started:** ADR-0008 has shipped — the core emits a semantic `ViewTree` consumed by a **Jetpack
Compose Material 3** renderer (`NativeCardScreen`) beside the classic Canvas player. *Slice 1* (the
button/field set), *slice 2* (nested **layout** groups — [ADR-0014](adr/0014-layout-model-group-containers.md):
`row`/`column` containers + per-object `weight`, so native mode reflows into a grid), and *slice 3*
(the **`switch`** kind — [ADR-0015](adr/0015-switch-object-kind.md): a button with `checked`,
rendered as a Material `Switch`) are done. In progress: the rest of the layout model
(`grid`/`free`/`constraints`/insets), Material roles/`textRole`/theme, and lifecycle messages.

## Non-goals (for now)

Paint tools, networking/sync, and any return to an embedded scripting VM. HyperTalk stays
Rust-native — see [ADR-0001](adr/0001-rust-native-hypertalk.md).

## Decision records

- [ADR-0001 — Rust-native HyperTalk interpreter](adr/0001-rust-native-hypertalk.md)
- [ADR-0002 — JSON-string JNI bridge](adr/0002-json-string-jni-bridge.md) *(superseded by ADR-0012)*
- [ADR-0003 — Player-first, JSON-authored stacks](adr/0003-player-first-json-authored-stacks.md)
- [ADR-0004 — In-app HyperTalk script editor](adr/0004-in-app-script-editor.md)
- [ADR-0005 — On-device object authoring](adr/0005-object-authoring.md)
- [ADR-0006 — HyperTalk geometry properties](adr/0006-geometry-properties.md)
- [ADR-0007 — Text styling](adr/0007-text-styling.md)
- [ADR-0008 — Native-view rendering](adr/0008-native-view-rendering.md) *(accepted; Compose, slices 1–2)*
- [ADR-0009 — Touchscreen gestures](adr/0009-touchscreen-gestures.md)
- [ADR-0010 — Modern UI considerations](adr/0010-modern-ui-considerations.md)
- [ADR-0011 — YAML stack files](adr/0011-yaml-stack-files.md)
- [ADR-0012 — UniFFI bridge](adr/0012-uniffi-bridge.md) *(supersedes ADR-0002)*
- [ADR-0013 — Persistence layering](adr/0013-persistence-layering.md)
- [ADR-0014 — Layout model: group containers](adr/0014-layout-model-group-containers.md)
- [ADR-0015 — The `switch` object kind](adr/0015-switch-object-kind.md)
- [ADR-0016 — Grid layout + card-level layout scripting](adr/0016-grid-layout-and-card-layout-scripting.md)
- [ADR-0017 — `free` (absolute) layout mode](adr/0017-free-absolute-layout-mode.md)
