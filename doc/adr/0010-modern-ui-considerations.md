# ADR-0010 — Modern user-interface considerations (post-WIMP principles)

- Status: Accepted (guiding constraints; individual realizations are tracked by the ADRs cited)
- Date: 2026-06-16
- Related: [ADR-0001](0001-rust-native-hypertalk.md) (platform-agnostic core),
  [ADR-0002](0002-json-string-jni-bridge.md) (host bridge),
  [ADR-0007](0007-text-styling.md), [ADR-0008](0008-native-view-rendering.md) (proposed),
  [ADR-0009](0009-touchscreen-gestures.md), and the
  [Android-native dialect vision](../design/android-hypertalk-dialect.md).

## Context

HyperCard is a 1987 **WIMP** artifact: a single mouse pointer, a fixed monochrome card, modal
dialogs, a busy-loop `idle`, absolute pixel layout, no notion of accessibility, theming, density,
or asynchrony. HyperCatalog deliberately starts as a faithful *player* of that world, but its
north star is a player and dialect that feel native to a **post-WIMP** device — primarily a
touchscreen phone, but also tablets, foldables, dark mode, dynamic color, screen readers, and
varied input devices.

Decisions about input, rendering, layout, and theming keep recurring (ADR-0007, -0008, -0009).
Rather than re-argue first principles each time, this ADR records the **constraints** those
decisions must satisfy, and how they bind to the architecture. It is a guideline ADR: it does not
ship code, it governs the ADRs that do.

## Decision

Adopt the following as **binding design constraints** for HyperCatalog's evolution. Each names what
it requires and where it is (or will be) realized.

1. **Input is multi-modal, not mouse-only.** The mouse is one device among touch, stylus,
   hardware keyboard, D-pad/remote, and (later) voice. New interaction is expressed as **semantic
   messages** (`on longPress`, `on swipeLeft`), not pointer mechanics, so it is device-neutral and
   bubbles the normal message path. *Realized:* touchscreen gestures in
   [ADR-0009](0009-touchscreen-gestures.md). *Implied:* `mouseDown`/`mouseStillDown`-style
   continuous input, if ever added, needs a richer event channel (see constraint 7).

2. **Accessibility is first-class, never bolted on.** Content must be reachable by TalkBack, focus
   order, and live regions. This is the single strongest argument for **native-view rendering**
   (a `Canvas` cannot expose semantics to a screen reader); objects carry a
   `contentDescription`-style property. *Realized/owned by:* [ADR-0008](0008-native-view-rendering.md).

3. **Layout is responsive and density-aware, not fixed letterboxed pixels.** Coordinates trend
   toward **dp**; content reflows across sizes, orientations, foldable postures, and **safe-area
   insets** are addressable. Absolute "free" layout remains as a classic/escape-hatch mode.
   *Owned by:* the layout-model ADR to be written (design doc, Phase 5); today's letterbox +
   `CardTransform` is the interim.

4. **Theming is dynamic, not 1-bit.** Light/dark/system and seed-based dynamic color (Material You)
   are stack-level properties; the design language is **Material roles/type scale**, not Mac
   font/size/style. *Stepping stone:* [ADR-0007](0007-text-styling.md) (generic styling today);
   *target:* roles via [ADR-0008](0008-native-view-rendering.md).

5. **Motion is meaningful and platform-native.** Navigation maps to a real back-stack with
   Material transitions (container-transform, shared element), replacing 1-bit visual wipes. The
   system back gesture is an interceptable message (`on backPressed`). *Owned by:* the lifecycle /
   native-view ADRs.

6. **No busy-loop; lifecycle drives the UI.** HyperCard's `idle` is a battery anti-pattern and is
   dropped in favor of explicit Activity-lifecycle messages (`resume`/`suspend`/`rotate`/…) routed
   through the existing message path. *Owned by:* the lifecycle-message ADR to be written.

7. **The platform-agnostic seam is preserved.** Every modern affordance is realized **host-side**;
   the Rust core describes *what the UI means*, never *how the platform draws or recognizes it*
   (no Android types, measurement, theming, or gesture recognition in `hypercore`). Per
   [ADR-0001](0001-rust-native-hypertalk.md)/[ADR-0002](0002-json-string-jni-bridge.md), additions
   widen the JSON payload/vocabulary, not the bridge shape. Continuous/async interaction (motion,
   `get url`, drag deltas) may require the bridge to grow an async/event-stream channel — an open
   question flagged in [ADR-0008](0008-native-view-rendering.md), decided when scheduled.

8. **Backward compatibility by additive defaults.** Every new property/message/kind is optional
   with a sensible default (`#[serde(default)]`), so classic stacks load and play unchanged and
   the retro Canvas player can persist alongside the modern renderer. Unknown gestures, roles, and
   kinds **degrade gracefully** (no-op / fallback), never error.

## Consequences

- **Positive:** a stable rubric for UI decisions — proposals are checked against these eight
  constraints, and each constraint points to the ADR that owns it, reducing re-litigation.
- **Positive:** keeps the post-WIMP ambition honest about the platform-agnostic core: modernity
  lives in the host, intent lives in the core, and the two-host rule (Android + desktop) keeps the
  seam from leaking.
- **Negative / tension:** constraints 1, 5, and 7 collectively pressure ADR-0002's "event-driven,
  not per-frame" bridge; some (continuous gestures, motion, async data) cannot be satisfied without
  revisiting the marshalling/cadence decision.
- **Negative:** several constraints (2, 3, 4, 5, 6) are **aspirational** — owned by ADRs not yet
  written or accepted; this ADR records the target, not shipped behavior, for those rows.
- **Scope:** this is a principles record, not a schedule. It steers; the roadmap sequences.
