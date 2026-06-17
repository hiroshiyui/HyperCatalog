# ADR-0020 — Safe-area insets (and the deferred constraint solver)

- Status: **Accepted** — insets implemented (slice 8); `constraints`/anchor solver **deferred**.
- Date: 2026-06-17
- Related: [ADR-0008](0008-native-view-rendering.md), [ADR-0010](0010-modern-ui-considerations.md)
  ("safe-area insets are addressable"), [ADR-0014](0014-layout-model-group-containers.md)/[ADR-0017](0017-free-absolute-layout-mode.md)
  (the layout modes a constraint mode would join), and the dialect's
  `anchor the top of button "ok" to the safeBottom of this card`.

## Context

The dialect wants two related things for adaptive layout: **safe-area insets** addressable from
script, and a **`constraints`** layout mode with anchors. The insets are a small, high-value, clean
addition; the anchor *solver* is a large one (it needs `androidx.constraintlayout:constraintlayout-compose`
— not in our offline cache — plus a dynamic anchor model and edge-resolution). This ADR ships the
insets and **explicitly scopes the constraint solver to a follow-on**.

## Decision

**Safe-area insets (shipped):**

- The host pushes the current insets (status bar, nav bar, **display cutout**) in **dp** each layout
  pass via `HyperStack.set_insets(top, right, bottom, left)` →
  `Session::set_insets` → `Stack.safe_insets` (a `#[serde(skip)]` field — **session state, not
  document content**, so it never serializes).
- Scripts read `the safeTop / safeRight / safeBottom / safeLeft of this card` (and `of this stack`)
  as dp numbers, so a layout can avoid system UI (`set the y of button "ok" to the safeBottom of
  this card`, etc.). The native surface is already inset (the host pads the root for system bars +
  cutout); this makes the values *addressable*, the dialect's stated goal.
- Insets are **dp**, matching the dialect's "coordinates become dp" direction; the host owns the
  px→dp conversion.

**Constraints / anchors (deferred):** a `constraints` layout mode with
`anchor the <edge> of <obj> to the <edge|safeEdge> of <container>` needs the (uncached)
ConstraintLayout-Compose dependency and a dynamic anchor model. The insets shipped here are the
foundation it will build on (a safe-edge anchor resolves against exactly these values). Captured as
a follow-on rather than rushed.

## Consequences

- **Positive:** stacks can now adapt to real device chrome — notches, gesture nav, status bars —
  from script, for a tiny additive change (`#[serde(skip)]` field + four read-only properties + one
  bridge call). No model pollution (insets aren't persisted).
- **Positive:** `the safe* of this stack` works too (the `this stack` parser fix from ADR-0018
  pays off again).
- **Negative / deferred:** no `constraints` mode or `anchor … to …` yet — authors position against
  insets manually (read `the safeBottom`, set geometry) rather than declaratively. The full
  constraint solver (chains, barriers, ratios) is a separate, larger initiative.

## Non-goals (this slice)

The `constraints` layout mode, `anchor` statements, ConstraintLayout-Compose integration, and any
multi-object constraint solving — all deferred to a dedicated follow-on once the dependency and
anchor model are designed.
