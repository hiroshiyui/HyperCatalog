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
- **Native Material rendering** (Phase 5, below). A second render target: the core emits a semantic
  `ViewTree` realized by a Jetpack Compose Material 3 renderer — layout (column/row/grid/free), the
  `switch` kind, Material roles/theme, lifecycle messages, and safe-area insets
  ([ADR-0008](adr/0008-native-view-rendering.md), 0014–0020).
- **Persistence layering** ([ADR-0013](adr/0013-persistence-layering.md)). Atomic YAML stack writes;
  a Preferences DataStore for session view state (last stack + per-stack card index).
- **Sample content** (YAML, ADR-0011). `assets/sample.yaml` (demo), `assets/productivity.yaml`
  (To-Do, Counters, Tip Split, Calculator, Temperature, Length — the default stack),
  `assets/gestures.yaml` (a 3-card swipe/long-press/double-tap + switch + lifecycle demo), and
  `assets/layout_demo.yaml` (the native-dialect layout/roles/theme showcase, Phase 5).

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

### Phase 4 — Persistence & rendering polish *(mostly done)*

**Done:** a stack picker over multiple bundled stacks + per-stack saved working copies, with
`go to stack`/`show stacks`; **persistence layering** ([ADR-0013](adr/0013-persistence-layering.md))
— atomic YAML stack writes + a Preferences DataStore for session view state, so the **last-viewed
card index per stack is restored** on reopen. **Remaining:** multi-line fields with wrap/scroll
(today single-line; the Compose editable field already wraps, the Canvas one doesn't).

### Phase 5 — Android-native dialect *(largely shipped)*

A HyperTalk dialect whose primitives are Android's, not 1987 Mac's. The gate ([ADR-0008](adr/0008-native-view-rendering.md))
was a **second render target**: the core emits a semantic `ViewTree` (beside the Canvas draw list),
which a **Jetpack Compose Material 3** renderer (`NativeCardScreen`) realizes as real widgets, with
id-addressed dispatch into the same message path. A host toggle switches Classic ⇄ Native. The full
vision is in [`doc/design/android-hypertalk-dialect.md`](design/android-hypertalk-dialect.md).

**Shipped** as additive slices on that target (each its own ADR, each verified with Rust +
instrumented tests on a 16 KB-page emulator):

- **Layout** — nested `column`/`row`/`grid` group overlays + per-object `weight`/group `padding`
  ([ADR-0014](adr/0014-layout-model-group-containers.md), [ADR-0016](adr/0016-grid-layout-and-card-layout-scripting.md));
  `set the layout/padding of this card` scripting; a `free`/absolute mode. A card with **no overlay
  defaults to `free`** so native mirrors the classic layout (just as Material widgets); authors opt
  **into** responsive layout by adding a `layout` ([ADR-0017](adr/0017-free-absolute-layout-mode.md)).
- **`switch` kind** — a button with `checked`, auto-toggled, rendered as a Material `Switch`
  ([ADR-0015](adr/0015-switch-object-kind.md)).
- **Material theming** — `the role of` button (filled/tonal/outlined/text/elevated/fab), field
  `textRole` type scale, stack `theme`/`accentColor` → a seeded `MaterialTheme` (Material You on
  Android 12+) ([ADR-0018](adr/0018-material-roles-and-theme.md)).
- **Lifecycle messages** — host-fired `resume`/`suspend`/`backPressed`/`rotate`; `DispatchResult.handled`
  lets a stack consume back; `idle` dropped ([ADR-0019](adr/0019-lifecycle-messages.md)).
- **Safe-area insets** — `the safeTop/safeRight/safeBottom/safeLeft of this card`, in dp
  ([ADR-0020](adr/0020-safe-area-insets-and-constraints.md)).

`assets/layout_demo.yaml` ("Layout Demo") showcases the grid/row/column reflow + roles + theme +
switch; toggling Native/Classic on its card is a before/after of the whole dialect.

The remaining native components and platform facilities are sequenced below as **Phases 6–11**, in
priority order.

### Phase 6 — Native component palette *(done)*

The object taxonomy beyond `switch`, each following the same Design-B recipe (a `control`
discriminator on `Button` + state fields, projected to a distinct view-tree `kind`, rendered in
Compose, scriptable via interp arms): **`checkbox`** / **`radio`** (boolean), **`slider`** /
**`progress`** (`the value of`), **`image`** (`the source of`, local assets), **`chip`**, and
**`divider`** ([ADR-0021](adr/0021-component-palette.md)). Demoed in `assets/layout_demo.yaml`.

### Phase 7 — Accessibility & theming polish

`the contentDescription of <object>`, focus order, and live regions for **TalkBack** (ADR-0010's
"single strongest argument for native"); a seed→tonal palette so non-`dynamic` light/dark themes use
the stack's `accentColor`. Host-only, riding the existing view-tree pipe.

### Phase 8 — Platform escape hatches

Host-realized `HostEffect`s with light parser sugar: `open url`, `share`, `toast`, `send intent`, and
local prefs (`get/set the pref "key"`). No async; quick wins that make stacks feel like real apps.

### Phase 9 — Language & async foundation *(the enabler)*

Two cross-cutting capabilities, each its own ADR: **typed message args** (`on rotate w,h`,
`on permissionResult cam, granted`, handler params); and an **async bridge channel** (host→core
callbacks for deferred completions — the standing [ADR-0008](adr/0008-native-view-rendering.md) open
question). Largest/most architectural; unblocks Phase 10.

### Phase 10 — Async platform facilities

On the Phase 9 foundation: networking (`get url` → `on responseReceived data`), permissions
(`ask permission` → `on permissionResult`), snackbar actions, scheduled/notification messages, and
remote `image` sources.

### Phase 11 — Motion, navigation & layout completion

Material **visual effects** (`visual effect` → container-transform/fade-through), a real back-stack,
**shared-element transitions** (`go card "x" with shared "hero"`), and the **`constraints`/anchor**
layout solver (deferred from [ADR-0020](adr/0020-safe-area-insets-and-constraints.md)).

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
- [ADR-0018 — Material roles, `textRole`, theme/dynamic color](adr/0018-material-roles-and-theme.md)
- [ADR-0019 — Activity-lifecycle messages](adr/0019-lifecycle-messages.md)
- [ADR-0020 — Safe-area insets (constraint solver deferred)](adr/0020-safe-area-insets-and-constraints.md)
- [ADR-0021 — Native component palette](adr/0021-component-palette.md)
