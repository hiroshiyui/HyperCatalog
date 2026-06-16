# Android-native HyperTalk — a reference target (vision)

- Status: **Vision / north-star.** Not scheduled; no code committed to it yet. This document
  exists to steer decisions, not to describe shipped behavior.
- Date: 2026-06-16
- Related: [ADR-0001](../adr/0001-rust-native-hypertalk.md) (Rust-native language),
  [ADR-0002](../adr/0002-json-string-jni-bridge.md) (host bridge),
  [ADR-0006](../adr/0006-geometry-properties.md) (properties), and the
  [roadmap](../roadmap.md) Phase 5.

## The idea

HyperCatalog's HyperTalk today faithfully echoes 1987 Macintosh HyperCard: monochrome bitmap
buttons, absolute `x/y/w/h` layout, a fixed letterboxed card, a synchronous world. That's a fine
*player*. This document describes a different long-term target: a HyperTalk **dialect that feels
native to Android** — one whose primitives are Material components, the Activity lifecycle, a
responsive layout system, and the platform's reach (permissions, intents, async, accessibility).

The bet is that HyperTalk's *soul* survives the move: an approachable, English-like,
message-passing language; live on-device authoring; one implementation in Rust with no embedded
VM. What changes is the **vocabulary and the primitives** — we re-base them on Android instead of
on the Mac of 1987. Most of this is additive to the architecture we already have.

## The load-bearing decision: native-view rendering

Today the core emits **draw primitives** (`RenderList` of `DrawCmd`) and `CardView` paints them
on a `Canvas`. To be *truly* Material — ripples, IME behavior, accessibility, motion, system
theming — you cannot convincingly fake the platform on a Canvas. So the dialect targets a
different render contract:

| Today | Android-native target |
|---|---|
| Core emits **draw primitives** → `CardView` paints a `Canvas` | Core emits a **view tree** → host instantiates real Material **Views/Composables** |
| Letterbox-scaled fixed card | Responsive layout, real density (dp), insets |
| Custom-drawn buttons/fields | Real `Button`, `TextField`, `Switch`, … with ripples + a11y for free |

This is the one part that is **not** purely additive, and it is the decision that *defines* the
dialect. It is a coherent evolution rather than a teardown: ADR-0002 deliberately kept the host
boundary a thin, swappable, data-only channel, so "emit a view tree as JSON" is a new render
*target* over the same bridge shape, not a new bridge. The core stays platform-agnostic — it
describes *what* the UI is; the host decides *how* to realize it (today: Canvas; target: Material
views). The retro Canvas renderer can remain as an alternate "classic" target.

This render-contract fork is drafted as **[ADR-0008 — Native-view rendering](../adr/0008-native-view-rendering.md)**
(status: *proposed*); accepting it — and choosing **Android Views vs Jetpack Compose** as the
realization layer — is gated on the Open Questions below.

## Lifecycle — Android's, expressed as HyperCard system messages

HyperCard already dispatched system messages (`openStack`, `openCard`). Re-base them on the
Activity/Fragment lifecycle and route them through the existing message path
(object → card → background → stack):

```
on openStack        -- app launch (exists today)
on resume           -- onResume: refresh data, restart anything paused
on suspend          -- onPause: persist (today the host hard-codes the save)
on backPressed      -- intercept the system back gesture: go previous card, or confirm exit
on rotate w, h      -- configuration change → reflow
on permissionResult "camera", granted
on saveState / on restoreState
```

`idle` (HyperCard's busy-loop message) is a battery anti-pattern on mobile and is intentionally
dropped in favor of explicit event handlers. Mechanically this is additive: the host fires named
messages at lifecycle transitions; the interpreter already routes named messages.

## Design language — Material, not monochrome

Retire `ButtonStyle {rounded, rectangle, transparent}` in favor of **Material roles**, and reframe
the deferred text-styling work ([ADR-0006](../adr/0006-geometry-properties.md)) as **type roles**
on the Material scale rather than Mac font/size/style:

```
set the role of button "save" to "filled"        -- filled | tonal | outlined | text | elevated | fab
set the textRole of field "title" to "headlineSmall"   -- Material type scale, not a point size
set the accentColor of this stack to "#6750A4"          -- seed → dynamic color scheme (Material You)
set the theme of this stack to "dark"                   -- light | dark | system | dynamic
```

And a richer object **taxonomy** than button/field — HyperCard had two widgets; Android wants a
palette: `switch`, `slider`, `chip`, `checkbox`, `radio`, `image`, `progress`, `divider`. Each is
a model object with a `kind` and kind-specific properties (`the checked of`, `the value of slider`,
`the source of image`). This is a natural extension of the authoring taxonomy from
[ADR-0005](../adr/0005-object-authoring.md).

## Layout — responsive dp, not absolute letterboxed pixels

Make layout a property of a card or group, keeping "free" (absolute) for the classic feel but
adding real responsive modes:

```
set the layout of this card to "column"     -- free | row | column | grid | constraints
set the weight of field "body" to 1          -- flex within a row/column
set the padding of this card to 16           -- dp
anchor the top of button "ok" to the safeBottom of this card   -- insets are first-class
```

Coordinates become **dp**, content **reflows** across sizes/orientations, and safe-area insets are
addressable. This replaces letterbox scaling with genuine adaptivity — and pairs with the
native-view renderer, which already understands constraint/flex layout.

## Navigation & motion

`go card "x"` grows Material motion: `go card "detail" with shared "hero"` for shared-element
transitions; `visual effect` becomes Material container-transform / fade-through rather than a
1-bit wipe. The card stack maps onto a real back-stack so `on backPressed` and system gestures
behave as users expect.

## Platform reach — extend the host-effect channel

The core already returns `HostEffect`s it can't perform itself (`answer`, `beep`, message box).
The dialect widens that enum:

```
ask permission "camera"           -- returns via on permissionResult
open url "https://…"
share "text or file"
toast "saved"  /  snackbar "Undo" with action "undo"
send intent …                     -- escape hatch to the platform
```

## Async & data

HyperCard was synchronous; Android is not. Add non-blocking I/O with a completion message:

```
get url "https://api…/items"      -- non-blocking
on responseReceived data          -- fires when it returns; data is the body
```

Backed by coroutines/WorkManager on the host. Local persistence beyond the whole-stack JSON save
could surface as `the pref "key"` (SharedPreferences) or a small record store (Room).

## Accessibility

First-class, not bolted on: `the contentDescription of <object>`, focus order, and live-region
semantics — all of which the native-view renderer provides to TalkBack automatically. This alone
is a strong argument for the native-view direction.

## Scheduling & background

`schedule message "remind" after 1 hour` → WorkManager; notifications as a host effect. Lets a
stack do something useful while not in the foreground — very un-HyperCard, very Android.

## How this maps onto today's architecture

- **[ADR-0001] stays as-is.** The language remains lexer→parser→AST→interp in Rust; the dialect is
  new *vocabulary*, not a new runtime.
- **[ADR-0002] evolves.** The bridge carries a view tree and a wider host-effect set. One
  assumption to revisit: today the bridge is explicitly "event-driven, not per-frame." Material
  motion and `get url` callbacks push toward more frequent, asynchronous host↔core traffic — the
  contract may need an async/event-stream story, not just request/response.
- **`model.rs` grows:** an object `kind` taxonomy, Material properties (role, textRole, color),
  layout params (mode, weight, anchors, padding), and a stack `theme`. Existing classic stacks
  should still load (additive, defaulted fields), preserving the player.
- **ADRs:** native-view rendering (the fork above) is drafted as
  [ADR-0008](../adr/0008-native-view-rendering.md) *(proposed)*; the object-kind taxonomy, the
  layout model, and the lifecycle-message set remain to write when scheduled.

## Non-goals / honesty

- This is **not scheduled** and not a commitment to break the current player. The classic
  Canvas/absolute-layout mode can persist alongside the Material mode.
- It is a **large** effort and only makes sense incrementally — likely starting with the
  native-view renderer for the *existing* button/field set before adding new kinds, so each step
  ships something runnable.
- Faithful 1987 HyperCard fidelity is explicitly *not* the goal here; that's the current player's
  job. This target trades pixel-nostalgia for platform-nativeness.

## Open questions

1. **Views or Compose** as the host realization layer? Compose matches the declarative view-tree
   contract more naturally; Views are more battle-tested for dynamic, script-mutated trees.
2. How much **absolute/"free" layout** to keep once responsive layout exists — escape hatch only,
   or co-equal mode?
3. **Backward compatibility:** can classic JSON stacks (rounded/rectangle buttons, absolute rects)
   load unchanged into the Material renderer with sensible defaults?
4. **Bridge cadence:** does Material motion / async data force the event-driven JSON bridge to grow
   an async channel, and does that change the marshalling choice for hot paths?
