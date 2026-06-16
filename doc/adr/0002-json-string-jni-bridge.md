# ADR-0002 — JSON-string JNI bridge between host and core

- Status: Accepted
- Date: 2026-06-16 (records a decision made at project inception)

## Context

The Android host (Kotlin) must drive the Rust core: load a stack, render a card, deliver
taps, edit fields, save. Options for the boundary ranged from a rich typed JNI surface (many
methods, JNI object marshalling of model structs) to a coarse data-only channel.

Rendering in HyperCatalog is **event-driven** — the screen repaints on taps and navigation,
not on a per-frame animation loop — so the boundary is crossed rarely and with modest payloads.

## Decision

Keep a **small, opaque-handle JNI surface that exchanges JSON strings**. The handle is a
`Box<Session>` pointer as a `jlong`. Structured data (render lists, dispatch results, host
effects) crosses as serde-serialized JSON; the Kotlin side parses with `org.json`. The surface
is intentionally tiny — currently: `nativeLoad`, `nativeOpenCard`, `nativeRender`,
`nativeDispatchTouch`, `nativeSetFieldText`, `nativeToJson`, `nativeFree`.

The cross-language contract therefore lives in **three places that must stay in sync**: the
serde structs in `hypercore::session`, the JNI signatures in `hyperffi/src/android.rs`, and the
JSON parsing in `CardView.kt` / `NativeBridge.kt`.

## Consequences

- **Positive:** the boundary is trivial to evolve — adding a capability is one Rust method, one
  `extern "system"` wrapper, and one `external fun`, with no IDL or generated bindings.
- **Positive:** the core's facade (`Session`) is the single host-facing surface and is testable
  without JNI (the `hyper-desktop` REPL drives the same methods).
- **Negative:** JSON marshalling is unsuitable for high-frequency/per-frame data; this is an
  accepted limit given event-driven redraws. If real-time animation is ever needed, that path
  would bypass this bridge.
- **Negative:** three-place sync is a manual discipline; a drift shows up as a parse error or a
  missing field at runtime, not a compile error. Documented as a gotcha in `CLAUDE.md`.
- **Gotcha:** `org.json`'s `optString("k")` returns the literal `"null"` for a JSON `null`;
  the Kotlin side must check `isNull(k)` first (Rust serializes `Option::None` as `null`).
