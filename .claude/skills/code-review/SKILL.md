---
name: code-review
description: Full-scope code review for HyperCatalog — the Rust core (model, HyperTalk interpreter, Session facade), the JNI bridge, and the Android/Kotlin host. Covers correctness, FFI/memory safety, the cross-language JSON contract, test coverage, Android rendering/lifecycle, and clippy/convention hygiene, then reports findings and fixes critical issues.
---

Conduct a **project-wide sweep** — do not limit scope to recent changes. HyperCatalog is a
HyperCard-like player: a platform-agnostic Rust core (`rust/`) plus a thin Android host
(`app/`), connected by a small JSON-over-JNI bridge. Read broadly and apply every check below.

---

## Step 1 — Orient

- Read `CLAUDE.md` (architecture, gotchas) and `rust/README.md` (crate map, HyperTalk subset).
- Internalize the two halves and the **bridge contract** that spans three files which must stay
  in sync: serde structs in `rust/hypercore/src/session.rs` ↔ JNI funcs in
  `rust/hyperffi/src/android.rs` ↔ JSON parsing in `app/.../CardView.kt` & `NativeBridge.kt`.
- Prioritise: the FFI boundary, the interpreter, and the touch→script→render→persist path.

---

## Step 2 — Correctness (Rust core: `hypercore`)

- **No panics on script or host input.** The interpreter and `Session` must return `Result::Err`,
  never `unwrap`/`expect`/`panic!`/indexing that can panic on malformed scripts, bad selectors,
  out-of-range indices, or untrusted JSON. A panic that unwinds across the `extern "system"` FFI
  boundary is undefined behaviour.
- **Message path** (`session::collect_path`, `interp`): object → card → background → stack; first
  matching handler wins. A tapped object's own script must be found whether it lives on the card
  **or the background** layer (this was a real bug — see `background_button_script_runs`).
- **Selector resolution** (`interp::locate_field`/`locate_button`): by-number is 1-based; by-name
  is case-insensitive; missing objects yield `Err`, not a panic or silent wrong object.
- **Value coercion** (`value.rs`): HyperTalk string-centric semantics — empty→0 numerically,
  integral floats print without `.0`, comparisons numeric when both sides parse else text.
- **Navigation** wraps (`go next/previous`) and `card "name"`/`card N` resolve correctly.
- **Persistence**: `to_json`→`load_from_json` round-trips; serde defaults (`#[serde(default)]`)
  keep older/partial stacks loadable; field/button ids are stable across save/load.

---

## Step 3 — FFI & memory safety (`hyperffi/src/android.rs`)

- **Handle lifecycle**: `Box::into_raw`/`from_raw` paired exactly once; every entry point guards
  `handle == 0`; `nativeFree` is idempotent-safe and not used-after-free. The handle is the only
  owner of the `Session`.
- **`unsafe`** blocks are minimal and justified; raw-pointer deref only behind a null check.
- **No unwinding across FFI**: any code that could panic inside a `Java_*` function is a defect;
  prefer returning a sentinel (`0`/`"{}"`/`JNI_FALSE`) and surfacing errors as data.
- **Symbol/signature parity**: every `Java_org_ghostsinthelab_app_hypercatalog_NativeBridge_*`
  function has a matching `external fun` in `NativeBridge.kt` (same name, arg types, return type).
- Edition-2024 FFI form: `#[unsafe(no_mangle)]`, `unsafe {}` inside `unsafe fn`.

---

## Step 4 — Cross-language contract

- Any change to a `RenderList`/`DrawCmd`/`DispatchResult`/`HostEffect` field must be reflected in
  the Kotlin JSON parsing, and vice-versa. Missing/renamed keys fail silently in `org.json`.
- **`org.json` null quirk**: `optString(key)` returns the literal string `"null"` for a JSON
  `null`; guard with `isNull(key)` first (Rust serializes `Option::None` as `null`).
- Coordinates cross the bridge in **card space**; the host owns the letterbox transform.

---

## Step 5 — Test coverage

- `hypercore` unit tests (`src/tests.rs`) cover parser, interpreter, and `Session` behaviour. New
  HyperTalk constructs need both a **parse** test and an **eval** test.
- Every fixed bug gets a regression test (pattern: `background_button_script_runs`).
- The `hyper-desktop` REPL must still drive `sample.json` end-to-end (`tap`, `go`, `type`, `dump`).
- Tests must not depend on `Date::now`/`Math::random` (unavailable in the workflow harness; the
  interpreter's `random()` uses a seeded xorshift — keep it deterministic-friendly).

---

## Step 6 — Android host (`app/`)

- **Coordinate mapping** (`CardView`): `recomputeTransform` scale/offset applied consistently in
  both `onDraw` (card→view) and `onTouchEvent` (view→card); no drift.
- **Hit-test z-order** matches draw order (topmost wins; card layer above background; buttons
  above fields within a layer); invisible objects excluded.
- **Editable fields**: tap on an unlocked field opens the `EditText` overlay over the field rect;
  any subsequent tap commits first (`commitPendingEdit`), so scripts read fresh contents.
- **Lifecycle**: load in `onCreate` (saved `filesDir/stack.json` else `assets/sample.json`), save
  in `onPause`, `nativeFree` in `onDestroy`; all native calls guarded on `handle != 0`.
- No blocking/heavy work added to the UI thread; redraws stay event-driven, not per-frame.

---

## Step 7 — Code smells & hygiene

- **clippy is kept warning-free**: `cargo clippy --workspace --all-targets` must be clean.
- `cargo fmt --all` applied; idiomatic edition-2024 (let-chains, no needless `matches!(x, true)`,
  derive `Default` over hand-written impls).
- No dead branches, leftover `dbg!`/`println!` debugging, or commented-out code.
- Docs accurate: Rust `//!` module docs / `///` item docs, Kotlin KDoc; the **HyperTalk subset**
  list in `rust/README.md` and the gotchas in `CLAUDE.md` match the code.
- One concern per module; the bridge surface stays small and data-only.

---

## Reporting

Group findings by severity:

| Severity | Criteria |
|----------|----------|
| **Critical** | Memory-safety/UB, panic across the FFI boundary, use-after-free/double-free of the handle, persistence data loss — fix immediately |
| **Major** | Interpreter logic errors, drift between the three bridge layers, missing tests for observable behaviour, Android lifecycle/coordinate bugs |
| **Minor** | clippy/style, doc drift, naming, cosmetic issues |

For each finding cite **file:line**, describe the issue and its impact, and give a **concrete fix**.

---

## Fixing

Apply fixes for all Critical and Major findings directly, then verify the whole stack:

```bash
cd rust && cargo test -p hypercore && cargo clippy --workspace --all-targets && cargo fmt --all --check
cd .. && ./gradlew :app:assembleDebug
```

Do not consider the review complete until tests pass, clippy is clean, and the APK assembles.
Diagnose and resolve any failure before finishing. If a Rust source change was made, ensure the
`.so` is rebuilt (the `:app:assembleDebug` `cargoNdkBuild` task does this) before any on-device
re-verification.
