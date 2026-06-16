# ADR-0001 — HyperTalk interpreter implemented directly in Rust

- Status: Accepted
- Date: 2026-06-16 (records a decision made at project inception)

## Context

HyperCatalog needs to execute HyperTalk, the HyperCard scripting language. An early idea was
to embed a scripting VM — specifically mRuby — and translate or host HyperTalk on top of it.
That implies a C/FFI boundary, a second language runtime in the build, and impedance between
HyperTalk's string-centric, message-passing semantics and the host VM's object model.

## Decision

Implement HyperTalk **directly in Rust** as a classic pipeline: `lexer → parser → AST →
interpreter`, operating on the document model (`hypercore::model`). There is **no mRuby and no
C/FFI** for scripting. `Value` is HyperTalk's own string-centric type; the interpreter
(`script::interp::Runtime`) executes handler bodies against a `&mut Stack`.

## Consequences

- **Positive:** one language and one toolchain; the core stays `#![no_android]`-clean and
  fully unit-testable with `cargo test`; HyperTalk semantics (coercions, the message path) are
  modeled on their own terms; no FFI memory-safety surface for the language itself.
- **Positive:** scripts are stored as **source text** and parsed lazily on each handler run
  (`parse_script` in `run_handler`). There is no compile/link step, so editing a script string
  takes effect on the next dispatch — a property [ADR-0004](0004-in-app-script-editor.md)
  relies on.
- **Negative:** we reimplement language machinery (parsing, control flow, functions) instead
  of borrowing a VM; coverage is a deliberate **subset** and grows by hand (see the roadmap
  Phase 3 and `rust/README.md`).
- The only FFI in the project is the host bridge for data, not scripting —
  [ADR-0002](0002-json-string-jni-bridge.md).
