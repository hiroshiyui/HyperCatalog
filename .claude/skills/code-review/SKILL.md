---
name: code-review
description: Full-scope code review for HyperCatalog — the Rust core (model, HyperTalk interpreter, Session facade), the UniFFI bridge, and the Android/Kotlin host. Covers correctness, FFI/object-lifecycle safety, the generated typed cross-language contract, test coverage, Android rendering/lifecycle, and clippy/convention hygiene, then reports findings and fixes critical issues.
---

Conduct a **project-wide sweep** — do not limit scope to recent changes. HyperCatalog is a
HyperCard-like player/authoring app: a platform-agnostic Rust core (`rust/`) plus a thin Android
host (`app/`), connected by a **UniFFI-generated typed bridge** (ADR-0012). Read broadly and apply
every check below.

---

## Step 1 — Orient

- Read `CLAUDE.md` (architecture, gotchas) and `rust/README.md` (crate map, HyperTalk subset).
- Internalize the two halves and the **bridge contract**: `hypercore::Session` is re-exposed by
  `rust/hyperffi/src/bridge.rs` as a typed UniFFI `HyperStack` object (records/enums mirroring
  `hypercore` types — `RenderList`/`DrawItem`/`DispatchResult`/`HostEffect`/`ObjectProps`). The
  Kotlin bindings (`uniffi.hyperffi.*`) are **generated** by the `uniffiBindgen` Gradle task and
  consumed by `CardView.kt`/`MainActivity.kt`. There is **no hand-written JNI and no JSON on the
  wire** (the old `android.rs`/`NativeBridge.kt`/`org.json` path was removed — ADR-0012 supersedes
  ADR-0002).
- Prioritise: the FFI boundary, the interpreter, and the touch→script→render→persist path.

---

## Step 2 — Correctness (Rust core: `hypercore`)

- **No panics on script or host input.** The interpreter and `Session` must return `Result::Err`,
  never `unwrap`/`expect`/`panic!`/indexing that can panic on malformed scripts, bad selectors, or
  out-of-range indices. The release `.so` is built `panic = "abort"`, so a panic is a **clean
  process abort** (not unwinding-into-foreign-code UB) — but it still crashes the app, so it's a
  defect. (Selectors go through `find_index`, which bounds-checks; `card_index` is modulo-bounded;
  loops are budget-capped — keep it that way.)
- **Message path** (`session::collect_path`, `interp`): object → card → background → stack; first
  matching handler wins. A tapped object's own script must be found whether it lives on the card
  **or the background** layer (this was a real bug — see `background_button_script_runs`). Touch
  gestures and `openCard` bubble the same path.
- **Selector resolution** (`interp::locate_field`/`locate_button`): by-number is 1-based; by-name
  is case-insensitive; missing objects yield `Err`, not a panic or silent wrong object.
- **Value coercion** (`value.rs`): HyperTalk string-centric semantics — empty→0 numerically,
  integral floats print without `.0`, comparisons numeric when both sides parse else text.
- **Navigation** wraps (`go next/previous`); `card "name"`/`card N` resolve; `go to stack "Name"`
  and `show stacks` come back as host effects (the core has no asset access).
- **Persistence**: `to_yaml`→`load_from_yaml` round-trips (and legacy `load_from_json` still reads
  `.json`); serde defaults (`#[serde(default)]`) keep older/partial stacks loadable; field/button
  ids are stable across save/load.

---

## Step 3 — FFI & object-lifecycle safety (`hyperffi/src/bridge.rs`)

The hand-written JNI is gone, so most of the old hazards (raw `Box::into_raw`/`from_raw`, null
handle guards, `Java_*` symbol parity) no longer apply. The new concerns:

- **`HyperStack` lifecycle**: it wraps `Mutex<Session>` and is shared as `Arc<Self>`. The Kotlin
  side must `destroy()` it on stack switch and in `onDestroy` (else the native object leaks); never
  use a `HyperStack` after `destroy()`.
- **No panics reach the boundary**: `#[uniffi::export]` methods call `hypercore`, which must not
  panic (see Step 2). `self.inner.lock().unwrap()` is acceptable under `panic = "abort"` (a poisoned
  mutex can't arise — the first panic aborts), but the underlying `hypercore` call must stay
  panic-free.
- **Bridge mirrors stay in sync with `hypercore`**: the `From` conversions in `bridge.rs` must cover
  every field of the mirrored type; `i32` ids (not `u32`) so the generated Kotlin is `Int`. This is
  now a **compile-checked Rust-side** concern, not a manual three-place sync.
- **Generated-binding gotchas**: `uniffi-bindgen` reads metadata from a **host** (unstripped) build
  because the release `.so` is stripped; the generated Kotlin needs **JNA ≥ 5.16** at runtime
  (16 KB-aligned on x86_64). A UniFFI error/record field named `message` clashes with
  `Throwable.message` in Kotlin — avoid it.

---

## Step 4 — Cross-language contract (generated)

- A change to a bridge record/enum/method in `bridge.rs` regenerates the Kotlin automatically (the
  `uniffiBindgen` task), so there is **no second place to hand-edit** — but Kotlin call sites in
  `CardView`/`MainActivity` must be updated to the new typed shape (a *compile error* now, not a
  silent runtime mismatch).
- Records map snake_case → camelCase in Kotlin; enums with data become sealed classes
  (`is HostEffect.Answer` → `e.text`). `Option<T>` → nullable; `-1` is the "none" sentinel for ids.
- Object **props** cross as a typed `ObjectProps` record (no JSON). `org.json` is **not used** in
  the host at all; the picker reads a stack file's `name` via a regex (`HostLogic.stackNameFrom`).
- Coordinates cross the bridge in **card space**; the host owns the letterbox transform.

---

## Step 5 — Test coverage

Assess coverage on **both** halves of the project — never sign off on the Rust side alone because
it is easier to run. Drive any coverage improvement from **tool-reported data**, not a guess about
what "looks" untested: gather a report and target the lines/branches it actually flags.

**Rust core (`rust/`):**
- `hypercore` unit tests (`src/tests.rs`) cover parser, interpreter, `Value` coercion, and
  `Session` behaviour (incl. typed `object_props`/`apply_object_props`). New HyperTalk constructs
  need both a **parse** test and an **eval** test.
- Every fixed bug gets a regression test (pattern: `background_button_script_runs`,
  `open_card_surfaces_host_effects`).
- The `hyper-desktop` REPL must still drive a sample stack end-to-end (`tap`, `go`, `type`, `dump`)
  — it loads `.yaml`/`.json` (e.g. `assets/sample.yaml`).
- Tests must not depend on `Date::now`/`Math::random` (unavailable in the workflow harness; the
  interpreter's `random()` uses a seeded xorshift — keep it deterministic-friendly).
- Measure with `cargo llvm-cov` (or `cargo tarpaulin`) when judging whether a module is covered.

**Android host (`app/`):**
- Local JVM unit tests live in `app/src/test` and run fast offline via
  `./gradlew :app:testDebugUnitTest` (no emulator, no `.so` — the bridge loads only at runtime).
  Instrumented tests in `app/src/androidTest` need a device/emulator.
- Pure host logic must be **framework-free and unit-tested**, not buried in a `View`. The letterbox
  coordinate math lives in `CardTransform` precisely so it is testable on the JVM
  (`CardTransformTest`); prefer extracting such logic over leaving it untestable inside `CardView`.
  When reviewing, flag testable logic that is trapped behind Android types (`Canvas`/`Paint`/
  `MotionEvent`) or the generated bridge with no test. (Bridge-touching code can't run in a JVM
  unit test, so push pure logic out of it.)
- Measure with JaCoCo (`createDebugUnitTestCoverageReport`) and target what it reports.

---

## Step 6 — Android host (`app/`)

- **Coordinate mapping** (`CardView`/`CardTransform`): scale/offset applied consistently in both
  `onDraw` (card→view) and `onTouchEvent` (view→card); no drift.
- **Hit-test z-order** matches draw order (topmost wins; card layer above background; buttons
  above fields within a layer); invisible objects excluded.
- **Editable fields**: tap on an unlocked field opens the `EditText` overlay over the field rect
  (`focusField` → `onEditField`); any subsequent tap commits first (`commitPendingEdit` →
  `setFieldText`), so scripts read fresh contents.
- **Dispatch results**: `applyDispatchResult` surfaces `hostCmds`/`error`, opens the field editor on
  `focusField`, and on `cardChanged` runs `openCard` **and surfaces its effects too**, then repaints.
- **Lifecycle**: load in `onCreate` (last-used `filesDir/stacks/<key>.yaml` or the bundled YAML
  asset, default `productivity`), save each stack's YAML working copy in `onPause`/on switch,
  `HyperStack.destroy()` in `onDestroy`; native calls guarded on `stack != null`.
- No blocking/heavy work added to the UI thread; redraws stay event-driven, not per-frame.

---

## Step 7 — Code smells & hygiene

- **clippy is kept warning-free**: `cargo clippy --workspace --all-targets` must be clean.
- `cargo fmt --all` applied; idiomatic edition-2024 (let-chains, no needless `matches!(x, true)`,
  derive `Default` over hand-written impls).
- No dead branches, leftover `dbg!`/`println!`/`Log` debugging, or commented-out code.
- Docs accurate: Rust `//!` module docs / `///` item docs, Kotlin KDoc; the **HyperTalk subset**
  list in `rust/README.md` and the gotchas in `CLAUDE.md` match the code.
- One concern per module; the bridge surface stays small and data-only.

---

## Reporting

Group findings by severity:

| Severity | Criteria |
|----------|----------|
| **Critical** | Memory-safety/UB, a panic that crashes the app from a bridge call, use-after-`destroy()` of a `HyperStack`, persistence data loss — fix immediately |
| **Major** | Interpreter logic errors, a typed-bridge mismatch (Rust ↔ Kotlin call site), missing tests for observable behaviour, Android lifecycle/coordinate bugs |
| **Minor** | clippy/style, doc drift, naming, cosmetic issues |

For each finding cite **file:line**, describe the issue and its impact, and give a **concrete fix**.

---

## Fixing

Apply fixes for all Critical and Major findings directly, then verify the whole stack:

```bash
cd rust && cargo test -p hypercore && cargo clippy --workspace --all-targets && cargo fmt --all --check
cd .. && ./gradlew :app:testDebugUnitTest && ./gradlew :app:assembleDebug
```

Do not consider the review complete until **both** the Rust and Android unit tests pass, clippy is
clean, and the APK assembles.
Diagnose and resolve any failure before finishing. If a Rust source change was made, ensure the
`.so` is rebuilt and the Kotlin bindings regenerated (the `:app:assembleDebug` `cargoNdkBuild` +
`uniffiBindgen` tasks do this) before any on-device re-verification.
