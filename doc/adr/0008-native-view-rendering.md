# ADR-0008 — Native-view rendering (semantic view tree)

- Status: **Accepted** — slice 1 implemented (the existing button/field set, rendered via Jetpack
  Compose); **slice 2** adds nested layout (see [ADR-0014](0014-layout-model-group-containers.md):
  `group` nodes populate `child_ids`, `ViewTree` gains `layout`/`padding`, objects gain `weight`).
  Open questions 1 and 4 are resolved (below); 2 and 3 remain open.
- Date: 2026-06-16 (accepted 2026-06-17)
- Related: [ADR-0001](0001-rust-native-hypertalk.md) (Rust-native language),
  [ADR-0002](0002-json-string-jni-bridge.md) (host bridge — this evolves its payload),
  [ADR-0006](0006-geometry-properties.md) / [ADR-0007](0007-text-styling.md) (properties this
  re-bases), and the [Android-native dialect vision](../design/android-hypertalk-dialect.md) +
  [roadmap](../roadmap.md) Phase 5.

## Context

Today the core emits **draw primitives** — a `RenderList` of `DrawCmd` in card coordinates — and
`CardView` paints them on a `Canvas` with letterbox scaling (and maps taps back via
`CardTransform`). This is a faithful retro *player*, but a `Canvas` cannot convincingly *be*
Android: ripples, IME behavior, TalkBack accessibility, Material motion, and system theming are
not paint operations — they are properties of real platform widgets. The Phase 5 dialect wants
those, and getting them by hand-drawing is a losing game.

The load-bearing question is therefore the **render contract**: what does the platform-agnostic
core hand the host, and who owns realization? The hard constraint is
[ADR-0001](0001-rust-native-hypertalk.md)'s and [ADR-0002](0002-json-string-jni-bridge.md)'s
shared premise — **`hypercore` has zero platform dependencies** and is fully testable headless
(the `hyper-desktop` REPL drives the same `Session` facade). Any answer that smuggles Android
into the core would break that premise.

## Decision

Introduce a **semantic view tree** as a *new render target* over the *existing* bridge, and have
the host instantiate real Material **Views/Composables** from it. The classic Canvas renderer is
**kept as an alternate target**, not replaced.

Concretely, the contract is **intent, not widgets**:

- The core emits a tree of nodes, each with a **stable id**, a **`kind`** (e.g. `button`, `field`,
  `switch`, `image`, `column`), a flat **property bag**, and **children**. It crosses as **typed
  UniFFI records** (ADR-0012 — *not* JSON; the original "serde JSON over the opaque-handle bridge"
  framing predates the UniFFI migration). A new `Session::render_view_tree` sits beside
  `render_current_card`, selected by a render *mode*. **Implementation note:** the tree is **flat**
  — `ViewTree { root_ids, nodes }` with `ViewNode { id, kind, props, child_ids }` and ordered
  `Prop { key, value }` — rather than a recursive record, so it crosses UniFFI cleanly and the
  desktop dump stays deterministic; layout containers later populate `child_ids` without a shape
  change. Slice 1 is one level deep (no containers yet).
- The tree carries **no Android types, no measured geometry, no pixels, no layout math**. The core
  says *what the UI is and what it means*; the **host decides how to realize it** — widget class,
  density (dp), insets, theming, reconciliation.

This keeps the platform-agnostic boundary exactly where ADR-0002 drew it, and only widens the
*payload*, not the bridge shape.

### Guardrails that keep the core platform-agnostic

These are the explicit rules that make "native rendering" not leak the platform into Rust:

1. **No Android in the contract.** Node kinds and property *keys* are abstract UI vocabulary
   (`button`, `checked`, `role`, `layout`), realizable by any host. Material-flavored property
   *values* (e.g. `role: "filled"`, `textRole: "headlineSmall"`) are **opaque strings the host
   interprets**, with graceful fallback for unknown kinds/roles — never Rust enums that bake in a
   platform's design system.
2. **The core never measures or lays out.** No dp, no insets, no view sizes, no hit-testing cross
   the boundary outward. Responsive layout is requested declaratively (`layout: column`, `weight`,
   `padding`) and resolved entirely by the host.
3. **Two hosts prove it.** `hyper-desktop` must still consume the same `Session` — at minimum
   serializing/walking the view tree as text — so the tree is exercised with **no Android present**
   in CI. The retro `CardView` target also remains, so the core demonstrably feeds two unrelated
   renderers. If only one host can render it, the abstraction has leaked.
4. **Events are semantic and id-addressed, not coordinate-based.** With native widgets the host no
   longer hit-tests pixels; instead a widget fires a **semantic event keyed by node id** (button
   clicked, switch toggled) back through an id-addressed dispatch into the same message path
   (object → card → background → stack). `dispatch_touch(x,y)` stays for the Canvas target; the
   view-tree target uses a `dispatch(id, message, args)` form. Both live behind the one facade.

### Realization layer (host side)

The choice of **Android Views vs Jetpack Compose**, and the tree-diffing/reconciliation strategy
(keyed by stable node id), are **host concerns** that do not affect the core contract. **Resolved:
Jetpack Compose (Material 3).** Compose maps naturally onto the declarative tree, and its
recomposition does the id-keyed reconciliation *for free* — there is no hand-written host diffing
(see the struck negative below). The host wraps each node in `key(node.id)` and re-fetches the tree
after a dispatch; `NativeCardScreen` realizes `button → Button/OutlinedButton/TextButton` (by the
abstract `style` value) and `field → OutlinedTextField`. The classic Canvas `CardView` remains for
the retro mode, selected by a host-side render toggle.

## Consequences

- **Positive:** Material affordances — ripples, IME, **TalkBack accessibility**, motion, dynamic
  color/theming — come from real widgets *for free*, instead of being faked on a Canvas.
- **Positive:** The core stays platform-agnostic and headless-testable; the dialect is new
  **payload vocabulary**, not a new runtime (ADR-0001 holds) nor a new bridge (ADR-0002 holds).
- **Positive:** Keeping the Canvas target makes "classic" vs "material" a per-stack/per-build
  **render mode**; classic JSON stacks still load and play unchanged.
- ~~**Negative:** A view tree needs **reconciliation/diffing** on the host (rebuild vs patch by
  id) — materially more host complexity than blitting a flat draw list.~~ **Dissolved by the
  Compose choice:** recomposition keyed by node id reconciles automatically; the host writes no
  diffing logic.
- **Negative:** Inverts the event model — the host gains an **id-addressed dispatch** path, and the
  core grows a second dispatch entry point beside `dispatch_touch`.
- **Negative / to revisit:** ADR-0002 assumes the bridge is **event-driven, not per-frame**.
  Material motion and async data (`get url` → `on responseReceived`) push toward more frequent,
  asynchronous host↔core traffic; the JSON request/response shape may need an **async/event-stream
  channel** for hot paths. Tracked as an open question, not decided here.
- **Negative:** `model.rs` and the property set grow (object-kind taxonomy, roles, layout params);
  must stay additive with serde defaults so existing stacks load.

## Open questions

1. ~~**Views or Compose** as the host realization layer, and the diffing strategy?~~ **Resolved:
   Jetpack Compose** — recomposition is the diffing strategy (keyed by node id).
2. Does Material motion / async data force the bridge to grow an **async channel**, changing the
   marshalling choice for hot paths? *(Still open.)*
3. Can classic stacks (rounded/rectangle buttons, absolute rects) render in the Material target
   with **sensible defaults**, or is a translation/compat layer required? *(Partly answered by
   slice 1: button `style` maps to filled/outlined/text widgets with an outlined fallback, and a
   simple vertical `Column` stands in until the layout model lands. A fuller compat story waits on
   the layout ADR.)*
4. ~~Migration order: ship the native renderer for the **existing button/field set first**?~~
   **Resolved: yes** — slice 1 does exactly this, proving the contract before any new kinds.

## Follow-on ADRs (when scheduled)

The object-**kind taxonomy** (switch/slider/chip/image/…), the **layout model** (free/row/column/
grid/constraints, dp, insets), and the **lifecycle-message set** (`resume`/`suspend`/
`backPressed`/`rotate`) each warrant their own ADR; this one only fixes the render-contract fork.
