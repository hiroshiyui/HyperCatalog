# HyperCatalog Rust core

A platform-agnostic HyperCard-like engine: a document model, a HyperTalk interpreter
(written directly in Rust — no mRuby/C), stack persistence, and a small host-facing
`Session` facade. Android is the reference host; the core has **no** Android dependencies.

Stacks are **YAML** end to end (ADR-0011) — authored as `.yaml`/`.yml` with multi-line scripts as
block scalars (`Session::load_from_yaml`), and saved as YAML (`to_yaml`). `load_from_json` is
retained only to read legacy `.json` stacks. See
[`doc/adr/0011-yaml-stack-files.md`](../doc/adr/0011-yaml-stack-files.md).

## Crates

| Crate          | Type    | What it is |
|----------------|---------|------------|
| `hypercore`    | lib     | Model + HyperTalk lexer/parser/interpreter + persistence + `Session` facade. Platform-neutral. |
| `hyperffi`     | cdylib  | **UniFFI** bridge (`bridge.rs` → a typed `HyperStack` object); `uniffi-bindgen` generates the Kotlin host bindings. No hand-written JNI, no JSON on the wire (ADR-0012). |
| `hyper-desktop`| bin     | Headless REPL to drive a stack without an emulator. |

## Prerequisites

- Rust (pinned by `rust-toolchain.toml`: channel **1.95**, with `rustfmt` + `clippy`).
- Android targets (also listed in the toolchain file):
  `rustup target add aarch64-linux-android x86_64-linux-android armv7-linux-androideabi i686-linux-android`
- `cargo install cargo-ndk`
- Android NDK (the app's Gradle build expects the revision in `app/build.gradle.kts`'s
  `rustNdkVersion`, currently `29.0.14206865`).

## Common commands

```sh
# Run the core test suite (no emulator needed)
cargo test -p hypercore

# Drive a sample stack headlessly (.yaml or legacy .json)
cargo run -p hyper-desktop -- ../app/src/main/assets/sample.yaml
#   commands: dump | tap <name|id> | tap <x> <y> | type <field-id> <text> | go next|prev|first|last | save [path] | quit

# Cross-compile the Android .so into app/src/main/jniLibs (the Gradle build also does this)
ANDROID_NDK_HOME=$ANDROID_HOME/ndk/29.0.14206865 \
  cargo ndk -t arm64-v8a -t x86_64 -o ../app/src/main/jniLibs build --release -p hyperffi

cargo fmt --all
cargo clippy --workspace --all-targets
```

The Android app builds the `.so` automatically: `./gradlew :app:assembleDebug` runs the
`cargoNdkBuild` task before packaging.

## Supported HyperTalk subset (MVP)

Handlers `on <msg> [param, param…] ... end <msg>` (`mouseUp`, `openCard`, `openStack`, and custom
names) — **handler parameters** bind positional message arguments (ADR-0024); commands `put`,
`get`, `set ... of ... to`, `go [to] next|previous|first|last|card "x"|card N|stack "x"`, `answer`,
`beep`, `show stacks`, `open url <expr>`, `share <expr>`, `toast <expr>` (host-realized escape hatches,
ADR-0023); **async facilities** (ADR-0025) `get url <expr>` (→ `on responseReceived data, status, url`),
`ask permission <expr>` (→ `on permissionResult name, granted`), `snackbar <text> [action <label> send
<msg>]`, `notify <title>, <body> [send <msg>]`; `add/subtract/multiply/divide`; `if/then/else/end if`;
`repeat with`/`repeat N times`;
expressions with `& && + - * / mod`, comparisons, `the <prop> of <object>`,
`the number of cards`, `length()`, `field "name"` contents. Message path:
object → card → background → stack.

A **custom message** is sent by naming it as a command with optional args (`greet "World"`): it
re-dispatches along the message path (sender → card → background → stack), the first `on greet`
handler runs with `me` bound to its *defining* object, an unmatched name is a silent no-op, and
self-recursion is depth-bounded (ADR-0024). The host can inject a top-level message —
`HyperStack.dispatch_message(name, args)` — to deliver a deferred result (e.g. `on responseReceived
data`); the core runs synchronously and the host owns any concurrency.

Touchscreen gestures (the post-WIMP companion to `mouseUp`) are dispatched as messages along
the same path, so a stack-level `on swipeLeft` catches a swipe anywhere while an object can
intercept its own: `on tap`, `on doubleTap`, `on longPress`,
`on swipeLeft|swipeRight|swipeUp|swipeDown`. The host (`Session::dispatch_gesture`) feeds the
gesture to the object under the gesture's start point; an unhandled gesture is a no-op, and a
gesture never opens the field editor (only a plain tap, via `dispatch_touch`, focuses an
unlocked field). See [`doc/adr/0009-touchscreen-gestures.md`](../doc/adr/0009-touchscreen-gestures.md).

Object properties via `get`/`set the <prop> of <object>`:

- buttons & fields: `name`, `visible`, `id` (read-only); buttons also `title`; fields also
  `text`/`value`/`contents` and `locked`.
- geometry (buttons & fields): `loc`/`location` (center `"h,v"`), `rect`/`rectangle`
  (`"left,top,right,bottom"`), `width`, `height`, `top`, `left`, `bottom`, `right`. Setting
  `width`/`height` keeps the top-left corner; `loc` re-centers. See
  [`doc/adr/0006-geometry-properties.md`](../doc/adr/0006-geometry-properties.md).
- text styling (buttons & fields): `textFont` (`sans-serif`/`serif`/`monospace`), `textSize`,
  `textStyle` (comma list of `bold`/`italic`/`underline`; reads back `plain` when unset),
  `textAlign` (`left`/`center`/`right`, applied to fields). See
  [`doc/adr/0007-text-styling.md`](../doc/adr/0007-text-styling.md).
- native dialect (ADR-0014…0020; honored by the Compose render target, ignored by Canvas):
  buttons `checked` (a button with `checked` is a **switch**, auto-toggled before `mouseUp`),
  `role` (`filled`/`tonal`/`outlined`/`text`/`elevated`/`fab`), buttons & fields `weight` (flex in
  a layout group); fields `textRole` (Material type scale) and `label` (an editable field's Material
  floating label — the caption renders inside the `OutlinedTextField` outline); card `layout` (`column`/`row`/`grid`/
  `free`) + `padding`, read-only `safeTop`/`safeRight`/`safeBottom`/`safeLeft` (dp); stack `theme`
  (`light`/`dark`/`system`/`dynamic`) + `accentColor`, and the same `safe*` insets.
- card & stack: `name`, `number`.

Lifecycle messages (ADR-0019) are fired by the host at Activity transitions and bubble the message
path: `on resume`, `on suspend`, `on backPressed` (a handler consumes the system back), `on rotate`.
HyperCard's `idle` busy-loop is intentionally dropped.
