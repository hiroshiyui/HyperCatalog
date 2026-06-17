# ADR-0025 — Async platform facilities (networking, permissions, snackbar, notifications)

- Status: **Accepted** — implemented (Phase 10).
- Date: 2026-06-17
- Related: [ADR-0024](0024-typed-message-args-and-async-foundation.md) (the foundation this spends),
  [ADR-0023](0023-platform-escape-hatches.md) (the fire-and-forget effects this extends with return
  paths), [ADR-0008](0008-native-view-rendering.md).

## Context

ADR-0024 built the async foundation — handlers take positional args, and the host delivers a deferred
result by calling `dispatch_message(name, args)` (host owns concurrency; core stays synchronous, holds
no callback). Phase 10 spends it on the real facilities the dialect design names: **networking,
permissions, snackbar actions, and notifications** (remote images are a render-path change — ADR-0026).

Every one is the same shape: a script command → a **request `HostEffect`** the host performs off the
core → the host calls `dispatch_message(<completionMsg>, args)` when done, which runs the handler.

## Decision

Four new request `HostEffect`s, each with light parser sugar, realized by the host with **no new
dependencies** (the lean-host stance — `HttpURLConnection`, the already-present Material `Snackbar`,
`NotificationManager`):

| Script | Request effect | Completion message |
|---|---|---|
| `get url <expr>` | `GetUrl(url)` | `on responseReceived data, status, url` |
| `ask permission <expr>` | `AskPermission(name)` | `on permissionResult name, granted` |
| `snackbar <text> [action <label> send <msg>]` | `Snackbar(text, label, msg)` | the named `msg` on action tap |
| `notify <title>, <body> [send <msg>]` | `Notify(title, body, msg)` | the named `msg` on notification tap |

- **Parser** (`parser.rs`): `get` is guarded — `get url <expr>` is a fetch only when `url` is
  *followed by an expression*, so plain `get <var>` / `get the … of …` / `get field "x"` are
  untouched. `ask` is guarded on a following `permission` keyword (a bare `ask` stays free for a
  future dialog). `snackbar`/`notify` are new leading keywords with optional `action … send …` /
  `send …` tails. All desugar to `Stmt::Send("<name>", [args])`.
- **Interp** (`exec_send`): `geturl`/`askpermission` join the single-string-arg effect table;
  `snackbar`/`notify` evaluate their (≤3) positional args. Effects flow `HostCmd` → `HostEffect` →
  the bridge mirror exactly like the escape hatches.
- **Host** (`MainActivity`): `get url` fetches on a shared `Executor` (`HttpURLConnection`) then
  `runOnUiThread { deliverMessage("responseReceived", [body, status, url]) }`. `ask permission` maps a
  friendly name → an Android permission and uses an `ActivityResultContracts.RequestPermission`
  launcher (registered at construction); already-granted/unknown short-circuit. `snackbar` is a
  Material `Snackbar` whose action calls `deliverMessage(msg)`. `notify` posts via
  `NotificationManagerCompat` with a `PendingIntent` carrying the tap message back through
  `onNewIntent` → `deliverMessage(msg)`. `INTERNET` + `POST_NOTIFICATIONS` (runtime, API 33+) added.
- **`deliverMessage(name, args)`** is the single host→core delivery helper: it reads `stack` *fresh on
  the UI thread* (`val s = stack ?: return`), so a completion landing after `destroy()`/a stack switch
  is a safe no-op — the worker thread holds no `HyperStack` handle.

**Reply convention**: success and failure both fire the completion message with positional args; a
handler declaring fewer params ignores the rest (binding fills missing with empty), so `on
responseReceived data` and `on responseReceived data, status` both work. A failed fetch delivers empty
`data` with `status = "0"`.

## Consequences

- **Positive:** stacks can fetch data, request permissions, offer undo, and notify — the payoff the
  whole ADR-0008→0024 arc was building toward — for a request effect + a completion handler each, with
  zero new dependencies and no new bridge architecture (just additive enum variants).
- **Positive — uniform:** all four reuse `deliverMessage`, the lifecycle-safe re-entrant delivery
  point; both render targets surface them identically.
- **Caveat — no correlation token:** concurrent `get url`s all fire `responseReceived`; a request
  can't be distinguished from another in flight. A request-id token + an `on responseReceived id, …`
  form is a deliberate later slice. A completion that lands after a stack switch reaches the *new*
  stack (safe, but semantically loose) — also a later (epoch-gated) refinement.
- **Caveat — verification:** the Rust request-emission and the bridge crossing are CI-tested; the
  delivery half is covered by ADR-0024's tests. The actual device UI (live fetch, the permission
  dialog, the snackbar, the posted notification) is **device/manual** — not in the CI gate.

## Non-goals (later)

Scheduled/recurring notifications (WorkManager), request correlation tokens, `send … to <object>`
targeting, streaming/websockets, OkHttp, and download-to-file.
