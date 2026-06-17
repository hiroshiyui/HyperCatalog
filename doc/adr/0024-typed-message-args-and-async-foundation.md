# ADR-0024 — Typed message arguments and the async-delivery foundation

- Status: **Accepted** — implemented (Phase 9, the foundation only).
- Date: 2026-06-17
- Related: [ADR-0008](0008-native-view-rendering.md) (this resolves its open question #2),
  [ADR-0019](0019-lifecycle-messages.md) (lifecycle messages, now arg-bearing),
  [ADR-0023](0023-platform-escape-hatches.md) (fire-and-forget effects this complements with
  *return paths*).

## Context

The dialect could dispatch messages by **name** but not carry **arguments**, and a script had no way
to receive a **deferred result**. Both gaps block the same class of features: `on rotate w, h`,
`on permissionResult cam, granted`, `get url …` → `on responseReceived data`, and the Phase-8
deferrals (`send intent` with a result, prefs round-trip). ADR-0008's open question #2 — "does async
data force the bridge to grow an async channel?" — was still open.

Two capabilities unblock all of it: **handlers that take arguments**, and a **host→core entry point
to deliver a deferred message**. The interpreter was already half-built for the first
(`Handler.params` existed; `run_handler(.., args: &[Value])` already bound args→params by position),
so this phase is mostly *threading args through* the dispatch surface, plus one new behavior
(re-entrant custom `send`).

## Decision

**Typed message arguments** — string-centric, end to end (HyperTalk's `Value` is string-centric, so
`Vec<String>` on the wire suffices; no new typed-arg enum):

- `Session::dispatch_message` gains `args: &[Value]`, forwarded to `run_handler` (which already binds
  by position, missing→empty). `dispatch_lifecycle(message, args)` and `dispatch_by_id(id, message,
  args)` thread their `&[String]` through (mapped to `Value`). The host fires `on rotate w, h` with
  the new width/height (dp).
- **Custom-message send up the path.** A bare command whose name isn't a built-in
  (`greet "World"`) was a no-op; it now **re-dispatches** along the *same* message path the run was
  built with (object → card → background → stack). The `Runtime` carries that pre-collected path and
  a `send_depth` counter; `MAX_SEND_DEPTH = 64` bounds a handler that sends itself (returns `Err`,
  never overflows). The invoked handler's `me` is the object whose script **defines** it (HyperCard
  semantics), not the sender. An unmatched name stays a silent no-op (typo-safe). This also closes
  the Phase-3 "custom-message dispatch up the path" gap.

**Async-delivery foundation** — *host-driven re-entrant dispatch* (ADR-0008 Q#2 **resolved**):

- New `HyperStack::dispatch_message(name, args)` (over `Session::dispatch_message_named`) injects a
  top-level message with string args, bubbling the current card's path. This is the **delivery
  point** for any deferred completion: the host owns concurrency (Kotlin coroutines), performs the
  async work, and on completion calls `dispatch_message("responseReceived", [body])`, which runs the
  handler synchronously. **The core holds no callback object** and stays synchronous and pure — a
  rejected alternative (a core-held UniFFI callback interface) would invert ownership, complicate the
  `Mutex<Session>` with re-entrancy, and break the "core is pure" invariant for no gain, since the
  host already runs the event loop.

## Consequences

- **Positive:** one mechanism (`dispatch_message(name, args)`) serves both script→script custom sends
  and host→core async delivery. Phase 10 facilities (`get url`, `ask permission`) become a *request*
  `HostEffect` plus a completion call into this entry point — no further bridge architecture.
- **Positive — cheap:** no new wire types; the interpreter scaffolding (`params`, arg-binding) was
  already present, so the core change is small and the bridge adds two methods.
- **Caveat — scope:** `send … to <object>` target syntax is **not** supported (the parser has no
  `to`-target); a custom send bubbles the current path from the sender. Args are positional and
  string-typed. Both are acceptable for the foundation and revisited if a real need appears.

## Non-goals (Phase 10+)

The actual async facilities themselves — networking (`get url`), permissions, snackbar actions,
scheduled/notification messages — and `send … to <object>` targeting. This ADR lays only the
arguments + delivery foundation they build on.
