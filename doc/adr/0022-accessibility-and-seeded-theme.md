# ADR-0022 — Accessibility (TalkBack) and a seeded color scheme

- Status: **Accepted** — implemented (Phase 7).
- Date: 2026-06-17
- Related: [ADR-0010](0010-modern-ui-considerations.md) ("Accessibility is first-class… the single
  strongest argument for native-view rendering"), [ADR-0018](0018-material-roles-and-theme.md)
  (theming this extends), [ADR-0008](0008-native-view-rendering.md).

## Context

ADR-0010 calls accessibility "the single strongest argument for native rendering" — real Material
widgets give TalkBack semantics for free, which a Canvas can't. But the core wasn't yet *describing*
that intent (a Canvas-drawn icon button has no accessible name; a status field doesn't announce
changes). And the seeded theme (ADR-0018) only tinted `primary` for non-`dynamic` themes.

## Decision

**Accessibility** — additive model fields, projected as view-tree props, realized by Compose
semantics (host-only; no bridge change):

- `Button.content_description` and `Field.content_description` (`#[serde(default)]`) → a
  `contentDescription` prop → `Modifier.semantics { contentDescription = … }`, so an icon/image
  control with no visible text is still named for TalkBack.
- `Field.live_region` (`""` | `polite` | `assertive`) → a `liveRegion` prop →
  `Modifier.semantics { liveRegion = … }`, so a status readout announces its changes.
- Applied uniformly: `RenderNode` augments each node's `baseModifier` with `node.semanticsModifier()`,
  so every widget picks up its semantics in one place.
- Scriptable: `the contentDescription of <object>` (button & field) and `the liveRegion of <field>`.

**Seeded color scheme** — `seededScheme(base, seed, dark)` derives a cohesive scheme from the stack's
`accentColor`: `primary` from the seed, `onPrimary` by its value, and `secondary`/`tertiary`/
`primaryContainer` rotated/desaturated from its hue (HSV). Surfaces stay neutral. This is a
lightweight stand-in for a full Material tonal palette (which needs `material-color-utilities`),
applied to non-`dynamic` `light`/`dark` themes; `dynamic` still uses Material You on Android 12+.

## Consequences

- **Positive:** stacks are reachable by TalkBack — labelled controls, described images, announced
  status — the payoff ADR-0010 named, for a few additive fields. Seeded themes now feel cohesive
  (secondary/tertiary roles tinted), not just a single-color primary.
- **Positive — one-line application:** `RenderNode`'s `baseModifier → modifier` augmentation means
  every current and future widget honors semantics without per-branch edits.
- **Caveat:** `seededScheme` is an HSV approximation, not a true Material tonal palette; close enough
  for tinting, but a real palette (and full a11y of complex composites) waits on
  `material-color-utilities`. **Focus order** and custom traversal are deferred — Compose's natural
  reading order is used.

## Non-goals (later)

Explicit focus-order control, custom accessibility actions, heading/role semantics beyond
description/live-region, and a true seed→tonal palette (material-color-utilities).
