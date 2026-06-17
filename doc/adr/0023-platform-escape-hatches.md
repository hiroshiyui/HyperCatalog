# ADR-0023 — Platform escape hatches (`open url` / `share` / `toast`)

- Status: **Accepted** — implemented (Phase 8).
- Date: 2026-06-17
- Related: [ADR-0008](0008-native-view-rendering.md) (the native dialect this extends),
  [ADR-0012](0012-uniffi-bridge.md) (the `HostEffect` channel reused here),
  [ADR-0019](0019-lifecycle-messages.md) (another host-driven message).

## Context

The native dialect can now render and theme real widgets, but a stack still couldn't *reach out* of
its own card — open a link, share text, flash a quick confirmation. These are the small affordances
that make a stack feel like a real app, and HyperCard's lineage already had the shape for them: the
core can't perform them (no `Activity`, no `Intent`, no `Toast`), so they belong on the existing
**`HostEffect`** channel the host already drains for `answer`/`beep`/`go to stack`/`show stacks`.

The constraint that scoped this phase: **no async**. `open url`, `share`, and `toast` are all
fire-and-forget — the core emits an effect and never waits for a result — so they need nothing from
the (deferred) async bridge channel. Facilities that *do* need a result (`send intent` for a
returned payload, `get/set the pref` which must read state back into the script) are held for the
Phase 9 language/async foundation rather than bolted on here.

## Decision

Three new fire-and-forget host effects, each carrying one evaluated string, threaded end to end
through the established pattern (interp `HostCmd` → session `HostEffect` → bridge mirror → host):

- **`HostCmd::OpenUrl` / `Share` / `Toast`** (`script/interp.rs`). `exec_send` recognizes the
  command names `openurl` / `share` / `toast`, evaluates the first argument to text, and pushes the
  corresponding `HostCmd`. Because they ride `Stmt::Send`, no new statement variant is needed.
- **Parser sugar** (`script/parser.rs`): `open url <expr>` desugars to `Stmt::Send("openurl", [expr])`
  (the `url` keyword is optional). `share <expr>` and `toast <expr>` need no special parsing — an
  unknown leading identifier already parses as a `Send`, so they arrive as `Stmt::Send("share"/"toast", …)`
  for free.
- **`HostEffect::OpenUrl(String)` / `Share(String)` / `Toast(String)`** (`session.rs`), mapped from
  the `HostCmd`s in `host_effect()`, and mirrored in the UniFFI bridge enum (`hyperffi/src/bridge.rs`)
  with `From` arms — a compile-checked match, so a missing variant fails the build.
- **Host realization** (`MainActivity.onEffects`): `openurl` → `Intent.ACTION_VIEW` on the parsed
  `Uri` (guarded by `resolveActivity`, so a bad/unhandled URI toasts instead of crashing); `share`
  → `Intent.ACTION_SEND` `text/plain` via the system chooser; `toast` → a short `Toast`.
  `hostEffectsOf` flattens the three new bridge variants into the host's `(type, text)` form, so
  **both** render targets (Canvas and Compose) surface them identically.

## Consequences

- **Positive:** stacks can link out, share, and confirm with three lines of script and zero new
  bridge *methods* — only additive enum variants. The URL path is defensively guarded (no crash on a
  malformed or unhandled URI).
- **Positive — pattern reuse:** this is the `HostEffect` recipe applied verbatim; the only host code
  is two small `Intent` helpers and three `when` arms. No async machinery, no new threading.
- **Caveat:** fire-and-forget only — a stack can't learn whether the user actually shared, nor read a
  result back. Anything request/response (`send intent` with a return, prefs round-trip, networking)
  needs the Phase 9 async channel and typed message args.

## Non-goals (deferred to Phase 9/10)

`send intent` with a returned result, local preferences (`get/set the pref "key"` — needs read-state
back into the script), and any networking. These wait on the async bridge channel and typed message
arguments.
