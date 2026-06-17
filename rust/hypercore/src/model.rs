//! The HyperCard-like document model: a Stack of Cards, each holding Buttons and
//! Fields, layered over shared Backgrounds. Everything is plain data with serde so it
//! round-trips to YAML for persistence (ADR-0011; legacy JSON still loads).

use serde::{Deserialize, Serialize};

/// A rectangle in card coordinates (logical pixels, origin top-left).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ButtonStyle {
    #[default]
    Rounded,
    Rectangle,
    Transparent,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Button {
    pub id: u32,
    pub name: String,
    pub rect: Rect,
    /// Visible label. Falls back to `name` when empty.
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub style: ButtonStyle,
    /// Boolean state for toggle controls — `switch`/`checkbox`/`radio` (ADR-0015/0021). When
    /// `Some` and `control` is empty, the object is a legacy `switch`. Auto-toggled before `mouseUp`.
    #[serde(default)]
    pub checked: Option<bool>,
    /// Material **control** kind (ADR-0021): `""` (plain button, or `switch` when `checked` is set)
    /// | `checkbox` | `radio` | `slider` | `progress` | `image` | `chip` | `divider`. The native
    /// target renders the named widget; the Canvas target shows a terse textual stand-in.
    #[serde(default)]
    pub control: String,
    /// Numeric state for `slider`/`progress` controls (ADR-0021), in 0.0..=1.0; `None` otherwise.
    #[serde(default)]
    pub value: Option<f32>,
    /// Asset name / URL for an `image` control (ADR-0021); `""` otherwise.
    #[serde(default)]
    pub source: String,
    #[serde(default = "default_true")]
    pub visible: bool,
    /// HyperTalk source for this object's handlers.
    #[serde(default)]
    pub script: String,
    /// Font family name ("", "sans-serif", "serif", "monospace"); "" = host default.
    #[serde(default)]
    pub text_font: String,
    #[serde(default = "default_text_size")]
    pub text_size: f32,
    /// Comma-separated styles, any of `bold`, `italic`, `underline`; "" = plain.
    #[serde(default)]
    pub text_style: String,
    /// `left`, `center`, or `right`; "" = host default (left for fields).
    #[serde(default)]
    pub text_align: String,
    /// Flex weight within a `row`/`column` layout group (native render target, ADR-0014);
    /// 0 = no flex (natural/full size). Ignored by the Canvas target.
    #[serde(default)]
    pub weight: f32,
    /// Material button role (ADR-0018): `""` (use `style`) | `filled` | `tonal` | `outlined` |
    /// `text` | `elevated` | `fab`. The native target prefers this over `style`; Canvas ignores it.
    #[serde(default)]
    pub role: String,
    /// Accessibility label for TalkBack (ADR-0022); `""` = derive from the visible label. The
    /// native target applies it via Compose semantics; the Canvas target ignores it.
    #[serde(default)]
    pub content_description: String,
}

impl Button {
    pub fn label(&self) -> &str {
        if self.title.is_empty() {
            &self.name
        } else {
            &self.title
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Field {
    pub id: u32,
    pub name: String,
    pub rect: Rect,
    #[serde(default)]
    pub text: String,
    /// Locked fields cannot be edited by the user (still scriptable).
    #[serde(default)]
    pub locked: bool,
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default)]
    pub script: String,
    /// Font family name ("", "sans-serif", "serif", "monospace"); "" = host default.
    #[serde(default)]
    pub text_font: String,
    #[serde(default = "default_text_size")]
    pub text_size: f32,
    /// Comma-separated styles, any of `bold`, `italic`, `underline`; "" = plain.
    #[serde(default)]
    pub text_style: String,
    /// `left`, `center`, or `right`; "" = host default (left for fields).
    #[serde(default)]
    pub text_align: String,
    /// Flex weight within a `row`/`column` layout group (native render target, ADR-0014);
    /// 0 = no flex (natural/full size). Ignored by the Canvas target.
    #[serde(default)]
    pub weight: f32,
    /// Material type-scale token for the field's text (ADR-0018), e.g. `headlineSmall`,
    /// `bodyLarge`; `""` = host default. The native target maps it to `MaterialTheme.typography`.
    #[serde(default)]
    pub text_role: String,
    /// Accessibility label for TalkBack (ADR-0022); `""` = derive from contents.
    #[serde(default)]
    pub content_description: String,
    /// Live-region politeness (ADR-0022): `""` | `polite` | `assertive`. A `polite`/`assertive`
    /// field announces its text changes to TalkBack — for status readouts. Native target only.
    #[serde(default)]
    pub live_region: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Background {
    pub id: u32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub fields: Vec<Field>,
    #[serde(default)]
    pub buttons: Vec<Button>,
    #[serde(default)]
    pub script: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Card {
    pub id: u32,
    pub name: String,
    /// Optional shared background layer.
    #[serde(default)]
    pub background_id: Option<u32>,
    #[serde(default)]
    pub fields: Vec<Field>,
    #[serde(default)]
    pub buttons: Vec<Button>,
    #[serde(default)]
    pub script: String,
    /// Optional **layout overlay** for the native render target (ADR-0014): a tree of `row`/
    /// `column` groups that arrange this card's objects (referenced by id) into a responsive
    /// grid. `None` = no layout → the native renderer falls back to a flat column, and the
    /// Canvas target ignores this entirely (it always uses each object's absolute `rect`).
    #[serde(default)]
    pub layout: Option<LayoutGroup>,
}

/// A container node in a card's layout overlay (ADR-0014): arranges its `children` in a `row`
/// or `column`. Carries no geometry — only an abstract `mode`, `padding`, and flex `weight`
/// (within a parent group). The host maps these onto real layout (dp, Compose Row/Column).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayoutGroup {
    /// `"column"`, `"row"`, or `"grid"` (ADR-0016). Grid wraps `children` into rows of `columns`.
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default)]
    pub padding: f32,
    #[serde(default)]
    pub weight: f32,
    /// Columns per row when `mode == "grid"`; 0/unused otherwise (ADR-0016).
    #[serde(default)]
    pub columns: u32,
    #[serde(default)]
    pub children: Vec<LayoutChild>,
}

/// One child of a [`LayoutGroup`]: either a nested group (a map) or an existing object referenced
/// by id (a bare number). **Untagged** — the two forms are structurally disjoint (map vs number),
/// so it reads cleanly as `children: [10, 20, { mode: row, children: [...] }]` in both YAML and
/// JSON (and round-trips in both, unlike an externally-tagged enum, which yaml_serde would emit as
/// a `!group` YAML tag).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LayoutChild {
    Group(LayoutGroup),
    Object(u32),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stack {
    pub name: String,
    #[serde(default)]
    pub width: f32,
    #[serde(default)]
    pub height: f32,
    #[serde(default)]
    pub backgrounds: Vec<Background>,
    pub cards: Vec<Card>,
    #[serde(default)]
    pub script: String,
    /// Material theme for the native target (ADR-0018): `""`/`light` | `dark` | `system` |
    /// `dynamic` (Material You, Android 12+; falls back to the seed below). Canvas ignores it.
    #[serde(default)]
    pub theme: String,
    /// Seed color (hex, e.g. `#6750A4`) for the Material color scheme (ADR-0018); `""` = default.
    #[serde(default)]
    pub accent_color: String,
    /// Safe-area insets (dp) the host pushes in each layout pass (ADR-0020); **session state**, not
    /// document content (`#[serde(skip)]`), readable from scripts as `the safeTop of this card` etc.
    #[serde(skip)]
    pub safe_insets: SafeInsets,
}

/// Safe-area insets in dp (status bar, navigation bar, display cutout), set by the host each layout
/// pass (ADR-0020). Exposed to scripts so layouts can avoid system UI; never serialized.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SafeInsets {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Stack {
    pub fn background(&self, id: u32) -> Option<&Background> {
        self.backgrounds.iter().find(|b| b.id == id)
    }
}

fn default_true() -> bool {
    true
}

fn default_text_size() -> f32 {
    16.0
}

fn default_mode() -> String {
    "column".to_string()
}
