# HyperCatalog Rust core

A platform-agnostic HyperCard-like engine: a document model, a HyperTalk interpreter
(written directly in Rust — no mRuby/C), JSON persistence, and a small host-facing
`Session` facade. Android is the reference host; the core has **no** Android dependencies.

## Crates

| Crate          | Type    | What it is |
|----------------|---------|------------|
| `hypercore`    | lib     | Model + HyperTalk lexer/parser/interpreter + persistence + `Session` facade. Platform-neutral. |
| `hyperffi`     | cdylib  | JNI bridge (`Java_..._NativeBridge_*`). Android-only body; builds empty elsewhere. |
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

# Drive the sample stack headlessly
cargo run -p hyper-desktop -- ../app/src/main/assets/sample.json
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

Handlers `on <msg> ... end <msg>` (`mouseUp`, `openCard`, `openStack`); commands `put`,
`get`, `set ... of ... to`, `go [to] next|previous|first|last|card "x"|card N`, `answer`,
`beep`, `add/subtract/multiply/divide`; `if/then/else/end if`; `repeat with`/`repeat N times`;
expressions with `& && + - * / mod`, comparisons, `the <prop> of <object>`,
`the number of cards`, `length()`, `field "name"` contents. Message path:
object → card → background → stack.

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
- card & stack: `name`, `number`.
