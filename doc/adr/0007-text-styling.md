# ADR-0007 — Text styling (font / size / style / align)

- Status: Accepted
- Date: 2026-06-16

## Context

[ADR-0006](0006-geometry-properties.md) deferred `textStyle`/`textSize`/`textFont` because,
unlike geometry, they had **no backing model fields** and the renderer drew a fixed 16px style.
With geometry done, text styling is the remaining Phase 3 property gap and the most visible one —
it's what makes a card look designed rather than uniform. Unlike geometry this touches the whole
stack: model → render contract → renderer → scriptable properties → authoring inspector.

## Decision

Add four text attributes to **both** `Button` and `Field`, stored as plain serde fields with
defaults so existing stacks load unchanged:

| Field | Type | Default | Meaning |
|---|---|---|---|
| `text_font`  | String | `""` | family: `""`/`sans-serif`/`serif`/`monospace` (`""` = host default) |
| `text_size`  | f32    | `16` (via `default_text_size`) | point size in card units |
| `text_style` | String | `""` | comma list of `bold`, `italic`, `underline` (`""` = plain) |
| `text_align` | String | `""` | `left`/`center`/`right` (`""` = left for fields; buttons stay centered) |

Carried end-to-end:

- **Render contract:** `DrawCmd` gains the four fields; `field_cmd`/`button_cmd` copy them. The
  core describes styling; the host realizes it.
- **Renderer (`CardView`):** `applyTextStyle` builds a `Typeface` from family + bold/italic flags,
  sets size (× letterbox scale) and underline; fields honor `text_align`, button labels stay
  centered by convention.
- **Scriptable** (the HyperTalk-property angle): `get`/`set the textFont|textSize|textStyle|
  textAlign of <object>`. `the textStyle` reads back as `"plain"` when unset, per HyperTalk.
- **Authoring:** the property inspector gains size, font, align, and bold/italic/underline
  controls (the dialog is now wrapped in a `ScrollView`); `get_object_props`/`set_object_props`
  round-trip the four keys, with `set` accepting `text_size` as a number or numeric string.

### Choices and their reasons

- **Style as a comma string, not a bitfield/enum.** Matches HyperTalk (`the textStyle` is a list
  like `bold,italic`), keeps JSON readable, and avoids a new enum type across the bridge.
- **Generic font families only** (sans/serif/mono) rather than arbitrary fonts. Android can't
  honor a random family name without bundled font assets; an unknown name falls back gracefully
  via `Typeface.create`. Custom/bundled fonts are a later step.
- **`text_align` on fields; buttons stay centered.** Button labels are centered by convention;
  honoring alignment there adds little and risks surprise. The attribute is stored on buttons for
  symmetry but not applied to their label.
- **Size in card units, scaled at render** (not dp yet). Consistent with the current letterboxed
  coordinate model; the move to true dp belongs with the responsive-layout work
  ([Phase 5 vision](../design/android-hypertalk-dialect.md)).

## Consequences

- **Positive:** stacks can now look designed — headings, emphasis, alignment — both by authoring
  and from scripts (`set the textStyle of field "title" to "bold"`). Completes the documented
  Phase 3 property set for buttons/fields. Fully backward compatible: old stacks default to 16px
  plain, left-aligned.
- **Negative / limits:** no rich/per-run styling within one field (whole-object only); no real
  font families beyond the three generics; alignment not applied to button labels; size is still
  letterbox-scaled, not density-aware.
- **Follow-on:** Phase 5's native-view renderer would map these onto Material type roles and real
  resources; bundled custom fonts and dp sizing are natural extensions.
