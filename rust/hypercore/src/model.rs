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
