---
name: docs-engineering
description: Audit and update HyperCatalog documentation (CLAUDE.md, rust/README.md, in-code doc comments) to stay in sync with the current code.
---

When performing documentation engineering, follow these steps:

1. **Audit** all documentation against the current codebase. Scope, without exception:
   - `CLAUDE.md` — architecture (the two halves + JSON-over-JNI bridge), commands, and the
     project-specific gotchas (edition-2024 FFI form, `org.json` null quirk, compileSdk/NDK pins,
     cargo-ndk Exec task). The "three places to keep in sync" rule must remain accurate.
   - `rust/README.md` — crate map (`hypercore` / `hyperffi` / `hyper-desktop`), prerequisites
     (toolchain 1.95, android targets, cargo-ndk, NDK revision), commands, and especially the
     **supported HyperTalk subset** list — keep it matching what the parser/interpreter actually
     accept.
   - `README.md` at the repo root, if present.
   - In-code docs: Rust `//!` module docs and `///` item docs on public APIs (`model`, `session`,
     `script::*`); KDoc on the Kotlin host classes (`NativeBridge`, `CardView`, `MainActivity`).

2. **Revise** anything stale, incomplete, or inconsistent. New HyperTalk constructs, new bridge
   calls, changed build settings (compileSdk, NDK, ABIs, `rustNdkVersion`), and architectural
   decisions must be reflected accurately. When the bridge contract changes, confirm the docs
   describe the new `RenderList`/`DispatchResult`/`HostEffect` shape.

3. **Keep the memory notes current** if they drift: `hypercatalog-architecture` and
   `hypercatalog-known-gaps` describe decisions and deferred scope; move items out of the gaps
   note as they are implemented.

4. **Commit** documentation changes grouped by topic (e.g. `docs: document repeat loops in
   HyperTalk subset`). Do not mix unrelated doc changes in one commit.
