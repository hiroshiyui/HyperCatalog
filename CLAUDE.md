# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

HyperCatalog — a HyperCard-like UI player. The scripting language is **HyperTalk implemented
directly in Rust** (lexer → parser → AST → interpreter). There is deliberately **no mRuby and
no C/FFI**; an earlier mRuby-embedding idea was dropped. The Rust core is platform-agnostic;
Android is the reference host. Current scope is a stack **player** (browse mode), not an
authoring environment.

## Architecture (the big picture)

Two halves connected by a **UniFFI-generated typed bridge** (ADR-0012 — no hand-written JNI, no
JSON on the wire); the host drives a typed `HyperStack` object:

```
Android (Kotlin host, thin)                    rust/ workspace
  MainActivity ─ load/save, EditText overlay     hyperffi  (cdylib)  UniFFI: bridge.rs (HyperStack)
  CardView     ─ Canvas draw + hit-test ─uniffi─▶ hypercore (lib)    model + HyperTalk + Session
  (uniffi.hyperffi.* generated Kotlin, via JNA)   hyper-desktop (bin) headless REPL
```

- **`rust/hypercore`** is the heart and has **no Android dependencies**. Sub-structure:
  - `model.rs` — `Stack → Card/Background → Button/Field` (serde; persisted as YAML, legacy JSON).
  - `script/` — `lexer` → `parser` → `ast` → `interp` (the `Runtime` that executes handlers
    against a `&mut Stack`), plus `value` (HyperTalk's string-centric `Value`).
  - `session.rs` — the **only** surface hosts call (the platform-agnostic facade): `load_from_json`
    / `load_from_yaml` (YAML is the readable authoring format, ADR-0011; same model, alternate
    parser), `render_current_card` (→ `RenderList` of draw primitives), `dispatch_touch` (hit-tests,
    runs scripts, returns `DispatchResult` with `host_cmds`/`focus_field`/`card_changed`),
    `dispatch_gesture` (post-WIMP touchscreen gestures — `tap`/`doubleTap`/`longPress`/`swipe*`
    — sent as messages that bubble the same path; never focuses a field), `set_field_text`,
    `to_yaml`. `hyperffi/bridge.rs` re-exposes these as a UniFFI `HyperStack` object (typed
    records, `i32` ids); the Kotlin bindings are generated, so there is no hand-written JNI.
- **Message path** (HyperCard semantics, in `session::collect_path`): a tapped object's script
  runs first, then card → background → stack; the first matching handler wins. **Background
  objects' own scripts must be searched too** — a past bug only looked at the card layer. Touch
  gestures (`dispatch_gesture`) bubble this same path, so stack-level `on swipeLeft` works.
- **Host effects** the core can't do itself (`answer`, `beep`, message-box `put`, and
  `go [to] stack "Name"` — the core has no asset access) come back as `HostEffect` values for the
  host to perform; the host also performs the EditText overlay for
  editable (unlocked) fields when `dispatch_touch` returns `focus_field`.
- **Rendering**: the core emits card-coordinate draw primitives; `CardView` letterbox-scales
  them onto a Canvas and maps touches back. Redraws are event-driven (taps), not per-frame.
- **Persistence** is layered by *what the data is* (ADR-0013): **document content vs. session view
  state**.
  - *Document content* → **YAML files**, end to end (ADR-0011). Bundled assets are `assets/*.yaml`
    (readable block scalars; default `productivity`); the host saves each stack's per-stack working
    copy as `filesDir/stacks/<key>.yaml` (on pause/switch) **atomically** (`writeFileAtomically`:
    temp + `rename`, so a crash mid-save can't truncate it). JSON is **deprecated for stacks**:
    `load_from_json` still reads legacy `.json` assets/copies, but nothing writes JSON.
  - *Session view state* → a **Preferences DataStore** (`StackPrefs`), host-owned, never in the
    document: the last-used stack key and **each stack's last-viewed card index** (`card_index/<key>`)
    — so reopening lands on the card you left. The old `filesDir/last_stack` text file is migrated
    in once. Card index is deliberately *not* in the stack YAML (it's the viewer's cursor, not
    document content), so a shared/exported stack opens to its first card. The bridge gained
    `currentCardIndex()`/`openCardAt(i)` (thin wrappers over `Session::card_index`/`goto_card`) for
    the host to read/restore it.

When changing the cross-language contract, edit the typed surface in `hyperffi/src/bridge.rs`
(UniFFI records/enums + `HyperStack` methods, mirroring `hypercore` types) — the Kotlin bindings
are **generated** by the `uniffiBindgen` Gradle task, so there's no second place to hand-edit (the
old serde ↔ `android.rs` ↔ `org.json` three-place sync is gone). The whole surface is typed,
**object props included** (`ObjectProps` record); no JSON crosses the bridge.

## Commands

Rust core (no emulator needed — do logic work here first; it's fast and fully testable):
```sh
cd rust
cargo test -p hypercore                          # full suite
cargo test -p hypercore button_handler_mutates_field   # a single test by name
cargo run -p hyper-desktop -- ../app/src/main/assets/sample.yaml   # drive a stack headlessly (.yaml/.json)
cargo fmt --all
cargo clippy --workspace --all-targets           # kept warning-free
```
`hyper-desktop` REPL commands: `dump`, `tap <name|id>`, `tap <x> <y>`, `type <field-id> <text>`,
`go next|prev|first|last`, `save [path]`, `quit`.

Android app (builds the Rust `.so` automatically via the `cargoNdkBuild` Gradle task):
```sh
./gradlew :app:assembleDebug                     # runs cargo-ndk, then packages
adb install -r app/build/outputs/apk/debug/app-debug.apk
adb shell am start -n org.ghostsinthelab.app.hypercatalog/.MainActivity
```
Manual cross-compile (what the Gradle task runs):
```sh
ANDROID_NDK_HOME=$ANDROID_HOME/ndk/29.0.14206865 \
  cargo ndk -t arm64-v8a -t x86_64 -o app/src/main/jniLibs build --release -p hyperffi
```
After editing Rust, rebuild the `.so` (command above or `:app:assembleDebug`) **before**
reinstalling — Gradle won't see Rust source changes unless `cargoNdkBuild` reruns.

## Project-specific gotchas

- **Rust 2024 edition** + toolchain pinned to **1.95** (`rust/rust-toolchain.toml`); let-chains
  (`if let ... && let ...`) are used and available. The FFI is now generated by UniFFI proc-macros
  (`#[uniffi::export]` in `bridge.rs`), so there's no hand-written `#[unsafe(no_mangle)]`/`unsafe`.
- **UniFFI bridge** (ADR-0012): the host↔core boundary is a generated typed `HyperStack` (no JSON,
  no `org.json` for render/dispatch). The generated Kotlin uses **JNA** at runtime (`jna:5.17@aar`),
  and `uniffiBindgen` reads metadata from a **host** (unstripped) build because the Android release
  `.so` is stripped. The whole surface is typed (incl. object props as an `ObjectProps` record), so
  **`org.json` is gone from the host entirely** — the picker reads a stack file's `name` with a
  small regex (`stackNameFrom` in `HostLogic.kt`, unit-tested).
- **compileSdk is 37** (androidx.core 1.19.0 requires it); NDK `29.0.14206865`; ABIs limited to
  arm64-v8a + x86_64. The NDK revision is the `rustNdkVersion` constant in `app/build.gradle.kts`,
  shared with AGP and cargo-ndk.
- cargo-ndk is invoked via a plain `Exec` task, **not** a third-party Rust/Gradle plugin
  (AGP 9.2.1 / Gradle 9.4.1 are bleeding edge). `resolveCargo()`/`resolveSdkDir()` find the tools.
- **16 KB page size** ([guide](https://developer.android.com/guide/practices/page-sizes)): Android
  15+ devices may use 16 KB pages and Play requires support. NDK 29 (≥ r28) links the `.so`
  16 KB-aligned by default; `rust/.cargo/config.toml` also sets `-Wl,-z,max-page-size=16384`
  explicitly. Verify with `llvm-readelf -l libhyperffi.so | grep LOAD` (align must be `0x4000`).
  The UniFFI bridge (ADR-0012) bundles JNA's `libjnidispatch.so`: pin **JNA ≥ 5.16** (we use
  5.17) — it's 16 KB-aligned on both arm64-v8a and x86_64, whereas 5.15 was only 4 KB-aligned on
  x86_64 and tripped the emulator's page-size warning.
- The HyperTalk interpreter is a **subset** (documented in `rust/README.md`). Unknown custom
  messages (`Stmt::Send`) are no-ops; `repeat`/property coverage is partial.

## Test coverage

When discussing or improving test coverage, consider **both** the Rust core (`rust/hypercore`
tests, plus `hyperffi`/`hyper-desktop`) **and** the Android/Kotlin side (`app/src/test`,
`app/src/androidTest`) — don't stop at the Rust half just because it's easier to run. Drive
improvements from **tool-reported data**, not guesses about what "looks" untested: gather
coverage with the appropriate tooling (e.g. `cargo llvm-cov`/`cargo tarpaulin` for Rust,
JaCoCo via `./gradlew :app:testDebugUnitTest`/`createDebugCoverageReport` for Android) and
target the lines/branches the reports actually flag.

See `rust/README.md` for the prerequisites and the full list of supported HyperTalk constructs.
