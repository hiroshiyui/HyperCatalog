# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

HyperCatalog — a HyperCard-like UI player. The scripting language is **HyperTalk implemented
directly in Rust** (lexer → parser → AST → interpreter). There is deliberately **no mRuby and
no C/FFI**; an earlier mRuby-embedding idea was dropped. The Rust core is platform-agnostic;
Android is the reference host. Current scope is a stack **player** (browse mode), not an
authoring environment.

## Architecture (the big picture)

Two halves connected by a small, event-driven JNI bridge that exchanges **JSON strings**:

```
Android (Kotlin host, thin)                    rust/ workspace
  MainActivity ─ load/save, EditText overlay     hyperffi  (cdylib)  JNI: Java_..._NativeBridge_*
  CardView     ─ Canvas draw + hit-test  ──JNI──▶ hypercore (lib)    model + HyperTalk + Session
  NativeBridge ─ external funs                    hyper-desktop (bin) headless REPL
```

- **`rust/hypercore`** is the heart and has **no Android dependencies**. Sub-structure:
  - `model.rs` — `Stack → Card/Background → Button/Field` (serde, JSON-persistable).
  - `script/` — `lexer` → `parser` → `ast` → `interp` (the `Runtime` that executes handlers
    against a `&mut Stack`), plus `value` (HyperTalk's string-centric `Value`).
  - `session.rs` — the **only** surface hosts call: `Session::load_from_json` /
    `load_from_yaml` (YAML is the readable authoring format, ADR-0011; same model, alternate
    parser), `render_current_card` (→ `RenderList` of draw primitives), `dispatch_touch` (hit-tests,
    runs scripts, returns `DispatchResult` with `host_cmds`/`focus_field`/`card_changed`),
    `dispatch_gesture` (post-WIMP touchscreen gestures — `tap`/`doubleTap`/`longPress`/`swipe*`
    — sent as messages that bubble the same path; never focuses a field), `set_field_text`,
    `to_json`.
- **Message path** (HyperCard semantics, in `session::collect_path`): a tapped object's script
  runs first, then card → background → stack; the first matching handler wins. **Background
  objects' own scripts must be searched too** — a past bug only looked at the card layer. Touch
  gestures (`dispatch_gesture`) bubble this same path, so stack-level `on swipeLeft` works.
- **Host effects** the core can't do itself (`answer`, `beep`, message-box `put`, and
  `go [to] stack "Name"` — the core has no asset access) come back as `HostEffect` values for the
  host to perform; the host also performs the EditText overlay for
  editable (unlocked) fields when `dispatch_touch` returns `focus_field`.
- **Rendering**: the core emits card-coordinate draw primitives; `CardView` letterbox-scales
  them onto a Canvas and maps touches back. Redraws are event-driven (taps), not per-frame —
  hence JSON-string marshalling is fine.
- **Persistence**: stacks are **YAML** end to end (ADR-0011) — bundled assets are `assets/*.yaml`
  (readable block scalars; default `productivity`), and the host saves each stack's per-stack
  working copy as `filesDir/stacks/<key>.yaml` (on pause/switch), remembering the last-used stack
  in `filesDir/last_stack`. JSON is **deprecated for stacks**: `load_from_json` still reads legacy
  `.json` assets/copies for compatibility, but nothing writes JSON. The **JNI bridge** still uses
  JSON for now (being migrated to UniFFI — ADR-0012). The current card index is **not** persisted
  (reopens at card 1).

When changing the cross-language contract, keep three things in sync: the serde structs in
`hypercore::session`, the JNI signatures in `hyperffi/src/android.rs`, and the JSON parsing in
`CardView.kt` / `NativeBridge.kt`.

## Commands

Rust core (no emulator needed — do logic work here first; it's fast and fully testable):
```sh
cd rust
cargo test -p hypercore                          # full suite
cargo test -p hypercore button_handler_mutates_field   # a single test by name
cargo run -p hyper-desktop -- ../app/src/main/assets/sample.json   # drive a stack headlessly
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

- **Rust 2024 edition** + toolchain pinned to **1.95** (`rust/rust-toolchain.toml`). This means
  `#[unsafe(no_mangle)]` (not `#[no_mangle]`) and `unsafe {}` blocks inside `unsafe fn`. Let-chains
  (`if let ... && let ...`) are used and available.
- **`org.json` quirk**: `optString("error")` returns the literal string `"null"` for a JSON
  `null`. Use `isNull(key)` first (see `CardView`). The Rust side serializes `Option::None` as
  `null`.
- **compileSdk is 37** (androidx.core 1.19.0 requires it); NDK `29.0.14206865`; ABIs limited to
  arm64-v8a + x86_64. The NDK revision is the `rustNdkVersion` constant in `app/build.gradle.kts`,
  shared with AGP and cargo-ndk.
- cargo-ndk is invoked via a plain `Exec` task, **not** a third-party Rust/Gradle plugin
  (AGP 9.2.1 / Gradle 9.4.1 are bleeding edge). `resolveCargo()`/`resolveSdkDir()` find the tools.
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
