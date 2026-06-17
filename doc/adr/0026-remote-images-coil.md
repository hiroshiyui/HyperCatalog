# ADR-0026 — Remote `image` sources via Coil

- Status: **Accepted** — implemented (Phase 10).
- Date: 2026-06-17
- Related: [ADR-0021](0021-component-palette.md) (the `image` control this extends),
  [ADR-0008](0008-native-view-rendering.md) (the native render target), [ADR-0025](0025-async-platform-facilities.md)
  (the other async facilities, shipped together).

## Context

The `image` control (ADR-0021) loaded a **local asset** by name (`source: "logo.png"` →
`AssetManager`). The dialect design calls for **remote** images too (`source` = an `http(s)` URL).
Unlike the local case, a remote image is an asynchronous load — exactly the kind of off-thread work
the host owns. It needs an image-loading library; the lean-host stance (ADR-0025) means adding one
only where there's no dependency-free path, and async image loading with caching is that case.

## Decision

- Add **Coil** (`io.coil-kt:coil-compose`) — a small, Compose-native, coroutine-based image loader —
  to the version catalog and app deps. It is the one new dependency in Phase 10.
- `NativeCardScreen`'s `image` branch checks the `source`: an `http://`/`https://` prefix → Coil
  `AsyncImage(model = source)` (async fetch + memory/disk cache + recomposition on load); otherwise the
  existing local `assetImage` path is unchanged.
- **Canvas** (classic) target: a remote `source` shows the existing `[image: <source>]` placeholder —
  the Canvas draws synchronously and can't async-load. Remote images are a **native-target** feature;
  the Canvas player degrades gracefully.
- No new `HostEffect`, parser sugar, or bridge change — this reuses the Phase-6 `image` control's
  `source` property end to end.

## Consequences

- **Positive:** `image` controls can show network images in native mode with one library and a
  three-line render branch; caching and lifecycle are Coil's job, not ours.
- **Positive — additive:** local-asset images, the Canvas path, and the view-tree contract are all
  unchanged; a stack with only local images pulls in Coil but exercises none of it.
- **Caveat:** remote images render in the **native** target only (Canvas shows a placeholder). Coil
  brings coroutines transitively (already on the Compose classpath). Network images need the
  `INTERNET` permission (added in ADR-0025). No explicit placeholder/error drawables yet — Coil's
  defaults (blank while loading) are used.

## Non-goals (later)

Author-controlled placeholder/error images, content-scale/crop controls, animated formats, and a
Canvas-target remote image (would require an async invalidate-and-redraw on the Canvas path).
