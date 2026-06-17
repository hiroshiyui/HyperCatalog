# ADR-0018 — Material roles, `textRole`, and stack theme/dynamic color

- Status: **Accepted** — implemented (slice 6 of the native render target).
- Date: 2026-06-17
- Related: [ADR-0008](0008-native-view-rendering.md) (native rendering),
  [ADR-0010](0010-modern-ui-considerations.md) ("Material roles/type scale, not Mac font/size/style"),
  and the dialect's `set the role of`, `set the textRole of`, `set the accentColor/theme of this stack`.

## Context

Slices 1–5 rendered native widgets but styled them with the legacy `ButtonStyle`
(rounded/rectangle/transparent) and Mac-ish font/size. The dialect re-frames appearance in
**Material terms**: button **roles** (filled/tonal/outlined/text/elevated/fab), text **type-scale
roles** (`headlineSmall`, …), and a stack-level **theme + dynamic color** (Material You).

## Decision

Additive model fields, projected as abstract props, realized by the Compose host:

- `Button.role: String` (`""` = fall back to `style`; else `filled|tonal|outlined|text|elevated|fab`),
  `Field.text_role: String` (a Material type-scale token), `Stack.theme: String`
  (`light|dark|system|dynamic`), `Stack.accent_color: String` (seed hex). All `#[serde(default)]`.
- `render_view_tree`: button nodes carry a `role` prop, field nodes a `textRole` prop, and
  `ViewTree` carries stack-level `theme`/`accent_color`.
- Scriptable: `the role of <button>`, `the textRole of <field>`, `the theme`/`the accentColor of
  this stack` (get/set). This required teaching the parser that **`this stack`** is a target (it
  previously collapsed any `this …` to the card — fixed, which also enables stack scripting broadly).
- Host: `NativeCardScreen` wraps the card in a `MaterialTheme` whose `ColorScheme` comes from
  `theme` (`darkColorScheme`/`lightColorScheme`, `system` follows `isSystemInDarkTheme`) seeded by
  `accent_color` (copied into `primary`); `dynamic` uses `dynamicLight/DarkColorScheme` on Android
  12+ and falls back to the seed below. `role` selects the Material button composable
  (`Button`/`FilledTonalButton`/`OutlinedButton`/`TextButton`/`ElevatedButton`/
  `ExtendedFloatingActionButton`); a missing role maps from the legacy `style` so slice-1 stacks
  look unchanged. `textRole` maps to `MaterialTheme.typography`.

## Consequences

- **Positive:** native mode is genuinely Material — themed color schemes (incl. Material You),
  semantic button roles, and the type scale — all authorable and scriptable. Existing stacks render
  unchanged (no role → style fallback; no theme → default scheme).
- **Positive:** the parser `this stack` fix is a general improvement (stack-level scripting was
  effectively unreachable before for non-`name` props).
- **Caveat:** full Material You needs a real seed→tonal-palette algorithm (`material-color-utilities`);
  this slice approximates by copying the seed into `primary` for the non-`dynamic` path. Dynamic
  color is correct on Android 12+, falls back to the seed below. The Canvas target ignores roles/theme
  (it stays 1-bit retro by design).

## Non-goals

A full seed→scheme tonal palette without `dynamic`; per-widget color overrides; dark/light *per
card*; typography beyond the standard Material tokens; theming the Canvas target.
