# ADR-0012 — UniFFI-generated typed bridge (supersedes ADR-0002)

- Status: **Accepted** — supersedes [ADR-0002](0002-json-string-jni-bridge.md); migration staged
  and in progress (not yet implemented).
- Date: 2026-06-17
- Related: supersedes [ADR-0002](0002-json-string-jni-bridge.md); complements
  [ADR-0011](0011-yaml-stack-files.md) (YAML *files*; this is the *bridge*); unblocks the async
  channel flagged in [ADR-0008](0008-native-view-rendering.md).

## Context

[ADR-0002](0002-json-string-jni-bridge.md) deliberately chose a tiny JSON-string JNI surface for
simplicity. Its accepted costs have become the thing to fix:

1. **Three-place manual sync** — the same shape is hand-maintained in the serde structs
   (`session.rs`), the `extern "system"` signatures (`hyperffi/src/android.rs`), and the Kotlin
   `org.json` parsing (`CardView.kt`/`MainActivity.kt`). Drift surfaces at **runtime**, not compile
   time (plus the `org.json` `optString` → `"null"` gotcha).
2. **JSON is the wrong tool for what's coming** — ADR-0008's native-view rendering and async data
   push toward typed, possibly streaming traffic.

The directive is to **deprecate JSON for the bridge entirely**. Readability is irrelevant on a
machine-to-machine wire, so the replacement must serve **type-safety, eliminating the hand-written
sync, and an async path** — i.e. a codegen/binding generator, **not** another text format. (YAML on
the wire would be slower, force a new YAML parser dependency into the Android app, and would *not*
fix the sync — strictly worse.)

## Decision

Adopt **UniFFI** (Mozilla) to **generate the FFI scaffolding and the Kotlin bindings from the Rust
`Session` interface**. The bridge stops being hand-written JNI exchanging JSON strings; instead:

- `Session` is exported as a UniFFI object; its methods (`load`, `render_current_card`,
  `dispatch_touch`, `dispatch_gesture`, authoring calls, …) are exported with `#[uniffi::export]`
  and return **typed records/enums** — `RenderList`, `DrawCmd`, `DispatchResult`, `HostEffect`,
  object props — instead of JSON strings.
- `uniffi-bindgen` generates the **Kotlin API** (data classes + the native loader) into the app's
  generated sources via a Gradle step, alongside the existing `cargo-ndk` `.so` build.
- The Kotlin host consumes generated typed objects; `org.json` parsing and the opaque `jlong`
  handle marshalling are **removed**.

The result: **no JSON on the bridge, no `org.json`, no three-place sync** — the Kotlin side is
generated from the single Rust definition.

### Choices and their reasons

- **UniFFI over Protobuf.** UniFFI removes **both** the wire format *and* the hand-written JNI/sync
  in one tool; it is purpose-built for Rust-on-mobile and yields iOS bindings later for free.
  Protobuf only solves the *data schema* — we'd still hand-write the JNI byte-passing — and its
  language-neutral/cross-process strengths don't apply to an in-process JNI call.
- **Async-ready.** UniFFI supports async exported functions, aligning with ADR-0008's future
  async/event-stream need without another bridge change.
- **Stack files stay serde/YAML.** ADR-0011's file format is independent: the model can keep serde
  for YAML/JSON persistence while the *bridge* types are UniFFI-exported (separate concern; types
  may be dual-purposed or mirrored).

## Consequences

- **Positive:** type-safe end-to-end; the three-place-sync gotcha and the `org.json` null quirk
  disappear (Kotlin is generated from Rust); async-ready; portable to an iOS host later.
- **Negative:** a **large** migration touching every bridge call, `hyperffi`, the Kotlin host, and
  the build (adding `uniffi-bindgen` codegen on top of `cargo-ndk` under **AGP 9.2 / Gradle 9.4** —
  bleeding-edge, the main integration risk). UniFFI **owns the boundary**: its type model
  constrains shapes (no arbitrary generics), and generated code enters the build. Bigger binary;
  a learning curve.
- **Supersedes ADR-0002**, whose JSON-wire decision and three-place-sync gotcha no longer hold once
  migrated. ADR-0011's "the bridge stays JSON" note is also overtaken.

## Migration plan (staged; re-verify on device at each stage)

1. **Toolchain spike** — add UniFFI; export one trivial function; generate Kotlin; wire the Gradle
   codegen task; confirm it builds and is callable on the emulator under cargo-ndk + AGP 9. This
   de-risks the whole effort before touching the real API.
2. **Read path** — port `load` + `render_current_card` to typed records (`RenderList`/`DrawCmd`);
   `CardView` draws from generated objects. Dispatch may stay on the old path temporarily.
3. **Dispatch path** — port `dispatch_touch`/`dispatch_gesture`/`open_card` to typed
   `DispatchResult` + `HostEffect`.
4. **Authoring path** — port the object/property/script calls.
5. **Remove JSON** — delete the hand-written JSON bridge and all `org.json` usage; update the
   "three-place sync" guidance in `CLAUDE.md`.

## Non-goals

- **Not** changing the stack *file* format decision (ADR-0011 / YAML stands).
- **Not** keeping a permanent JSON fallback — JSON is removed once each path is ported.
