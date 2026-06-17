//! The host-facing facade. A `Session` owns a `Stack` and the current card index, and
//! exposes the small surface the platform host (Android/desktop) drives: render the
//! current card, dispatch a touch, edit a field, navigate, and serialize for saving.
//!
//! The boundary is data-only (serde structs / plain records); `hyperffi` re-exposes it as a typed
//! UniFFI `HyperStack`, so the host gets generated, type-safe bindings (ADR-0012).

use serde::Serialize;

use crate::model::{Button, ButtonStyle, Field, Rect, Stack};
use crate::script::{HostCmd, Me, Runtime};

/// Smallest width/height an object may be resized to (card units), so drag-resize can't
/// produce a zero or negative rect.
const MIN_OBJECT_SIZE: f32 = 12.0;

pub struct Session {
    stack: Stack,
    card_index: usize,
}

/// A flat list of draw primitives for the current card, background layer first.
#[derive(Debug, Serialize)]
pub struct RenderList {
    pub stack_name: String,
    pub card_name: String,
    pub card_index: usize,
    pub card_count: usize,
    pub width: f32,
    pub height: f32,
    pub items: Vec<DrawCmd>,
}

#[derive(Debug, Serialize)]
pub struct DrawCmd {
    /// "button" or "field".
    pub kind: String,
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub text: String,
    /// Button style ("rounded"/"rectangle"/"transparent"); empty for fields.
    pub style: String,
    pub visible: bool,
    pub locked: bool,
    /// Text styling for the host to apply when drawing the label/contents.
    pub text_font: String,
    pub text_size: f32,
    pub text_style: String,
    pub text_align: String,
}

/// A **semantic view tree** for the native (Material) render target (ADR-0008) — the alternate
/// to `RenderList`. The core says *what the UI is and means*; the host realizes it as real
/// widgets. The tree is **flat** (id-indexed `nodes` + `root_ids`) so it crosses UniFFI cleanly
/// (no recursive records) and stays forward-compatible: layout containers later populate
/// `child_ids` without changing the shape. **No geometry/pixels cross outward** — the host owns
/// layout (contrast `DrawCmd`, which carries card-coord rects for the Canvas target).
#[derive(Debug, Serialize, PartialEq)]
pub struct ViewTree {
    pub stack_name: String,
    pub card_name: String,
    pub card_index: usize,
    pub card_count: usize,
    /// Root container arrangement: `"column"`, `"row"`, or `"grid"` (ADR-0014/0016). Always
    /// `"column"` for a card with no layout overlay (the flat fallback).
    pub layout: String,
    /// Root container padding (abstract units; the host maps to dp). 0 when no overlay.
    pub padding: f32,
    /// Columns per row when `layout == "grid"` (ADR-0016); 0 otherwise.
    pub columns: u32,
    /// Card size in card units, for `layout == "free"` absolute placement (ADR-0017); 0 otherwise.
    pub width: f32,
    pub height: f32,
    /// Stack-level Material theme + seed color (ADR-0018); the host builds a color scheme from them.
    pub theme: String,
    pub accent_color: String,
    /// Top-level node ids, in render (z) order.
    pub root_ids: Vec<u32>,
    pub nodes: Vec<ViewNode>,
}

/// One node in a [`ViewTree`]. `kind` and prop *keys* are abstract UI vocabulary (`button`,
/// `field`, `group`, `text`, `locked`); the host maps them to widgets/containers and degrades
/// gracefully on unknowns.
#[derive(Debug, Serialize, PartialEq)]
pub struct ViewNode {
    pub id: u32,
    /// Abstract UI kind: `"button"`, `"field"`, or `"group"` (a layout container; ADR-0014).
    pub kind: String,
    /// Stable, ordered abstract properties (a `Vec`, not a map, so the desktop `tree` dump and
    /// golden tests are deterministic). Group nodes carry `mode`/`padding`/`weight`; objects
    /// carry their abstract props plus `weight`.
    pub props: Vec<Prop>,
    /// Child node ids — empty for objects; a group's laid-out children (ADR-0014).
    pub child_ids: Vec<u32>,
}

/// One abstract key/value property of a [`ViewNode`]. Values are opaque strings the host interprets.
#[derive(Debug, Serialize, PartialEq, Clone)]
pub struct Prop {
    pub key: String,
    pub value: String,
}

/// An object's editable properties for the host inspector — the union of the button and field
/// shapes (button-only fields blank for a field, and vice versa). The typed replacement for the
/// JSON props blob; geometry is read-only here (set it with `set_object_rect`).
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectProps {
    pub id: u32,
    /// "button", "switch", or "field".
    pub kind: String,
    pub name: String,
    pub title: String,
    pub style: String,
    pub text: String,
    pub locked: bool,
    /// Switch state (ADR-0015); false for plain buttons and fields.
    pub checked: bool,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub text_font: String,
    pub text_size: f32,
    pub text_style: String,
    pub text_align: String,
}

/// A host effect to perform after a dispatch (dialog, beep, message-box output).
#[derive(Debug, Serialize, PartialEq)]
#[serde(tag = "type", content = "text", rename_all = "lowercase")]
pub enum HostEffect {
    Answer(String),
    Message(String),
    Beep,
    /// `go [to] stack "Name"` — the host loads the named stack (serialized as `type:"gostack"`).
    GoStack(String),
    /// `show stacks` — the host opens its stack picker (serialized as `type:"showstacks"`).
    ShowStacks,
}

/// Result of dispatching a touch back to the host.
#[derive(Debug, Serialize)]
pub struct DispatchResult {
    pub needs_redraw: bool,
    pub card_changed: bool,
    /// If an editable (unlocked) field was tapped, its id — the host opens an editor.
    pub focus_field: Option<u32>,
    pub host_cmds: Vec<HostEffect>,
    pub error: Option<String>,
    /// Whether a handler actually matched and ran (ADR-0019); lets the host decide e.g. whether to
    /// consume a `backPressed` or fall back to the default.
    pub handled: bool,
}

impl DispatchResult {
    fn nothing() -> DispatchResult {
        DispatchResult {
            needs_redraw: false,
            card_changed: false,
            handled: false,
            focus_field: None,
            host_cmds: Vec::new(),
            error: None,
        }
    }
}

impl Session {
    pub fn load_from_json(json: &str) -> Result<Session, String> {
        let stack: Stack =
            serde_json::from_str(json).map_err(|e| format!("invalid stack JSON: {e}"))?;
        Session::from_stack(stack)
    }

    /// Load a stack from YAML — the readable authoring format (ADR-0011). The same model as
    /// JSON, so this is purely an alternate deserializer; the JNI bridge and saves stay JSON.
    pub fn load_from_yaml(yaml: &str) -> Result<Session, String> {
        let stack: Stack =
            yaml_serde::from_str(yaml).map_err(|e| format!("invalid stack YAML: {e}"))?;
        Session::from_stack(stack)
    }

    fn from_stack(stack: Stack) -> Result<Session, String> {
        if stack.cards.is_empty() {
            return Err("stack has no cards".to_string());
        }
        Ok(Session {
            stack,
            card_index: 0,
        })
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(&self.stack).unwrap_or_else(|_| "{}".to_string())
    }

    /// Serialize the stack to YAML — the format runtime working copies are now saved in, so
    /// stacks are JSON-free end to end (ADR-0011). Machine-written, so block-scalar prettiness
    /// isn't required; hand-authored assets are the readable ones.
    pub fn to_yaml(&self) -> String {
        yaml_serde::to_string(&self.stack).unwrap_or_else(|_| "{}".to_string())
    }

    /// Push the host's current safe-area insets (dp) into the session (ADR-0020), so scripts can
    /// read `the safeTop/safeRight/safeBottom/safeLeft of this card`. Session state, not persisted.
    pub fn set_insets(&mut self, top: f32, right: f32, bottom: f32, left: f32) {
        self.stack.safe_insets = crate::model::SafeInsets {
            top,
            right,
            bottom,
            left,
        };
    }

    pub fn card_index(&self) -> usize {
        self.card_index
    }

    pub fn card_count(&self) -> usize {
        self.stack.cards.len()
    }

    /// Build the draw list for the current card (background objects under card objects).
    pub fn render_current_card(&self) -> RenderList {
        let card = &self.stack.cards[self.card_index];
        let mut items = Vec::new();

        if let Some(bg) = card.background_id.and_then(|id| self.stack.background(id)) {
            for f in &bg.fields {
                items.push(field_cmd(f));
            }
            for b in &bg.buttons {
                items.push(button_cmd(b));
            }
        }
        for f in &card.fields {
            items.push(field_cmd(f));
        }
        for b in &card.buttons {
            items.push(button_cmd(b));
        }

        RenderList {
            stack_name: self.stack.name.clone(),
            card_name: card.name.clone(),
            card_index: self.card_index,
            card_count: self.stack.cards.len(),
            width: self.stack.width,
            height: self.stack.height,
            items,
        }
    }

    /// Build the **semantic view tree** for the current card — the native render target
    /// (ADR-0008), the structural analogue of [`Session::render_current_card`]. Same model walk
    /// and z-order (background under card), but projected into abstract [`ViewNode`]s carrying
    /// meaning, **not geometry**. The host realizes it as Material widgets.
    pub fn render_view_tree(&self) -> ViewTree {
        let card = &self.stack.cards[self.card_index];
        let bg = card.background_id.and_then(|id| self.stack.background(id));
        let mut nodes = Vec::new();

        let (layout, padding, columns, root_ids) = if let Some(root) = &card.layout {
            // Layout overlay (ADR-0014): walk the group tree, referencing objects by id. Group
            // nodes get synthetic ids above any object id, so they never collide or dispatch.
            // `free` mode (ADR-0017) emits absolute geometry on each object node so the host can
            // place them like the Canvas player.
            let geometry = root.mode.eq_ignore_ascii_case("free");
            let mut next_group_id = max_object_id(card, bg) + 1;
            let root_ids = project_children(
                card,
                bg,
                &root.children,
                &mut next_group_id,
                &mut nodes,
                geometry,
            );
            (root.mode.clone(), root.padding, root.columns, root_ids)
        } else {
            // No layout overlay → **mirror the classic Canvas layout** (ADR-0017): emit every
            // object with its absolute geometry as `free`, so native looks the same as classic for
            // un-authored cards (authors opt into responsive layout by adding a `layout` overlay).
            let mut root_ids = Vec::new();
            if let Some(bg) = bg {
                for f in &bg.fields {
                    push_free_node(&mut nodes, &mut root_ids, field_node(f), f.rect);
                }
                for b in &bg.buttons {
                    push_free_node(&mut nodes, &mut root_ids, button_node(b), b.rect);
                }
            }
            for f in &card.fields {
                push_free_node(&mut nodes, &mut root_ids, field_node(f), f.rect);
            }
            for b in &card.buttons {
                push_free_node(&mut nodes, &mut root_ids, button_node(b), b.rect);
            }
            ("free".to_string(), 0.0, 0, root_ids)
        };

        ViewTree {
            stack_name: self.stack.name.clone(),
            card_name: card.name.clone(),
            card_index: self.card_index,
            card_count: self.stack.cards.len(),
            layout,
            padding,
            columns,
            width: self.stack.width,
            height: self.stack.height,
            theme: self.stack.theme.clone(),
            accent_color: self.stack.accent_color.clone(),
            root_ids,
            nodes,
        }
    }

    /// Set a field's text by object id (host pushes back edited text). Searches the card
    /// layer then the background layer.
    pub fn set_field_text(&mut self, field_id: u32, text: &str) -> bool {
        let bg_id = {
            let card = &mut self.stack.cards[self.card_index];
            if let Some(f) = card.fields.iter_mut().find(|f| f.id == field_id) {
                f.text = text.to_string();
                return true;
            }
            card.background_id
        };
        if let Some(bg_id) = bg_id
            && let Some(bg) = self.stack.backgrounds.iter_mut().find(|b| b.id == bg_id)
            && let Some(f) = bg.fields.iter_mut().find(|f| f.id == field_id)
        {
            f.text = text.to_string();
            return true;
        }
        false
    }

    /// Topmost object id at a card-space point, regardless of lock state. Unlike
    /// `dispatch_touch` this only *selects* (it runs nothing); the host uses it in edit
    /// mode to pick which object's script to edit. Reuses the same hit-test traversal.
    pub fn object_at(&self, x: f32, y: f32) -> Option<u32> {
        self.hit_test(x, y).map(|h| match h {
            Hit::Button(id) | Hit::EditableField(id) | Hit::LockedField(id) => id,
        })
    }

    /// Read an object's HyperTalk source by id. Searches the card layer then the
    /// background layer; buttons before fields. Returns None if no object has that id.
    pub fn get_object_script(&self, id: u32) -> Option<String> {
        let card = &self.stack.cards[self.card_index];
        if let Some(b) = card.buttons.iter().find(|b| b.id == id) {
            return Some(b.script.clone());
        }
        if let Some(f) = card.fields.iter().find(|f| f.id == id) {
            return Some(f.script.clone());
        }
        if let Some(bg) = card.background_id.and_then(|i| self.stack.background(i)) {
            if let Some(b) = bg.buttons.iter().find(|b| b.id == id) {
                return Some(b.script.clone());
            }
            if let Some(f) = bg.fields.iter().find(|f| f.id == id) {
                return Some(f.script.clone());
            }
        }
        None
    }

    /// Write an object's HyperTalk source by id (same search order as `get_object_script`).
    /// Returns true if an object was updated. Validate with `check_script` first if you
    /// want to reject unparseable source.
    pub fn set_object_script(&mut self, id: u32, src: &str) -> bool {
        let bg_id = {
            let card = &mut self.stack.cards[self.card_index];
            if let Some(b) = card.buttons.iter_mut().find(|b| b.id == id) {
                b.script = src.to_string();
                return true;
            }
            if let Some(f) = card.fields.iter_mut().find(|f| f.id == id) {
                f.script = src.to_string();
                return true;
            }
            card.background_id
        };
        if let Some(bg_id) = bg_id
            && let Some(bg) = self.stack.backgrounds.iter_mut().find(|b| b.id == bg_id)
        {
            if let Some(b) = bg.buttons.iter_mut().find(|b| b.id == id) {
                b.script = src.to_string();
                return true;
            }
            if let Some(f) = bg.fields.iter_mut().find(|f| f.id == id) {
                f.script = src.to_string();
                return true;
            }
        }
        false
    }

    /// Validate HyperTalk source without running it: `Some(error)` if it fails to parse,
    /// else `None`. Lets the host reject bad scripts before saving.
    pub fn check_script(src: &str) -> Option<String> {
        crate::script::parse_script(src).err()
    }

    // ---- object authoring (Phase 2) ----

    /// Lowest id not used by any button or field on any layer of the stack.
    fn next_object_id(&self) -> u32 {
        let mut max = 0;
        for c in &self.stack.cards {
            for b in &c.buttons {
                max = max.max(b.id);
            }
            for f in &c.fields {
                max = max.max(f.id);
            }
        }
        for bg in &self.stack.backgrounds {
            for b in &bg.buttons {
                max = max.max(b.id);
            }
            for f in &bg.fields {
                max = max.max(f.id);
            }
        }
        max + 1
    }

    /// Create a new `"button"` or `"field"` on the current card at a default position and
    /// return its id, or None for an unknown kind. New objects always land on the card
    /// layer (not the shared background).
    pub fn add_object(&mut self, kind: &str) -> Option<u32> {
        let id = self.next_object_id();
        let card = &mut self.stack.cards[self.card_index];
        match kind {
            "button" => {
                card.buttons.push(Button {
                    id,
                    name: format!("Button {id}"),
                    rect: Rect {
                        x: 20.0,
                        y: 80.0,
                        w: 120.0,
                        h: 44.0,
                    },
                    title: "Button".to_string(),
                    style: ButtonStyle::Rounded,
                    checked: None,
                    visible: true,
                    script: String::new(),
                    text_font: String::new(),
                    text_size: 16.0,
                    text_style: String::new(),
                    text_align: String::new(),
                    weight: 0.0,
                    role: String::new(),
                });
                Some(id)
            }
            "field" => {
                card.fields.push(Field {
                    id,
                    name: format!("Field {id}"),
                    rect: Rect {
                        x: 20.0,
                        y: 80.0,
                        w: 200.0,
                        h: 36.0,
                    },
                    text: String::new(),
                    locked: false,
                    visible: true,
                    script: String::new(),
                    text_font: String::new(),
                    text_size: 16.0,
                    text_style: String::new(),
                    text_align: String::new(),
                    weight: 0.0,
                    text_role: String::new(),
                });
                Some(id)
            }
            _ => None,
        }
    }

    /// Delete an object by id from the current card, or its background. Returns true if one
    /// was removed.
    pub fn delete_object(&mut self, id: u32) -> bool {
        let bg_id = {
            let card = &mut self.stack.cards[self.card_index];
            let before = card.buttons.len() + card.fields.len();
            card.buttons.retain(|b| b.id != id);
            card.fields.retain(|f| f.id != id);
            if card.buttons.len() + card.fields.len() != before {
                return true;
            }
            card.background_id
        };
        if let Some(bg_id) = bg_id
            && let Some(bg) = self.stack.backgrounds.iter_mut().find(|b| b.id == bg_id)
        {
            let before = bg.buttons.len() + bg.fields.len();
            bg.buttons.retain(|b| b.id != id);
            bg.fields.retain(|f| f.id != id);
            return bg.buttons.len() + bg.fields.len() != before;
        }
        false
    }

    /// Move/resize an object by id (drag commit). Width/height are clamped to a minimum.
    /// Searches card layer then background layer. Returns true if one was updated.
    pub fn set_object_rect(&mut self, id: u32, x: f32, y: f32, w: f32, h: f32) -> bool {
        let rect = Rect {
            x,
            y,
            w: w.max(MIN_OBJECT_SIZE),
            h: h.max(MIN_OBJECT_SIZE),
        };
        let bg_id = {
            let card = &mut self.stack.cards[self.card_index];
            if let Some(b) = card.buttons.iter_mut().find(|b| b.id == id) {
                b.rect = rect;
                return true;
            }
            if let Some(f) = card.fields.iter_mut().find(|f| f.id == id) {
                f.rect = rect;
                return true;
            }
            card.background_id
        };
        if let Some(bg_id) = bg_id
            && let Some(bg) = self.stack.backgrounds.iter_mut().find(|b| b.id == bg_id)
        {
            if let Some(b) = bg.buttons.iter_mut().find(|b| b.id == id) {
                b.rect = rect;
                return true;
            }
            if let Some(f) = bg.fields.iter_mut().find(|f| f.id == id) {
                f.rect = rect;
                return true;
            }
        }
        false
    }

    /// Read an object's editable properties as JSON, for a host inspector. Buttons report
    /// `title`/`style`, fields report `text`/`locked`; both report `name` and geometry.
    pub fn get_object_props(&self, id: u32) -> Option<String> {
        if let Some(b) = self.find_button(id) {
            return Some(
                serde_json::json!({
                    "id": b.id, "kind": "button", "name": b.name, "title": b.title,
                    "style": format!("{:?}", b.style).to_lowercase(),
                    "x": b.rect.x, "y": b.rect.y, "w": b.rect.w, "h": b.rect.h,
                    "text_font": b.text_font, "text_size": b.text_size,
                    "text_style": b.text_style, "text_align": b.text_align,
                })
                .to_string(),
            );
        }
        if let Some(f) = self.find_field(id) {
            return Some(
                serde_json::json!({
                    "id": f.id, "kind": "field", "name": f.name, "text": f.text,
                    "locked": f.locked,
                    "x": f.rect.x, "y": f.rect.y, "w": f.rect.w, "h": f.rect.h,
                    "text_font": f.text_font, "text_size": f.text_size,
                    "text_style": f.text_style, "text_align": f.text_align,
                })
                .to_string(),
            );
        }
        None
    }

    /// Apply a JSON property blob to an object (any subset of keys; unknowns ignored).
    /// Returns true if the object was found. Geometry is not set here — use
    /// `set_object_rect`.
    pub fn set_object_props(&mut self, id: u32, props_json: &str) -> bool {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(props_json) else {
            return false;
        };
        let bg_id = {
            let card = &mut self.stack.cards[self.card_index];
            if let Some(b) = card.buttons.iter_mut().find(|b| b.id == id) {
                apply_button_props(b, &v);
                return true;
            }
            if let Some(f) = card.fields.iter_mut().find(|f| f.id == id) {
                apply_field_props(f, &v);
                return true;
            }
            card.background_id
        };
        if let Some(bg_id) = bg_id
            && let Some(bg) = self.stack.backgrounds.iter_mut().find(|b| b.id == bg_id)
        {
            if let Some(b) = bg.buttons.iter_mut().find(|b| b.id == id) {
                apply_button_props(b, &v);
                return true;
            }
            if let Some(f) = bg.fields.iter_mut().find(|f| f.id == id) {
                apply_field_props(f, &v);
                return true;
            }
        }
        false
    }

    /// Read an object's editable properties as a typed [`ObjectProps`] (the typed replacement for
    /// the JSON `get_object_props`).
    pub fn object_props(&self, id: u32) -> Option<ObjectProps> {
        if let Some(b) = self.find_button(id) {
            return Some(ObjectProps {
                id: b.id,
                kind: if b.checked.is_some() {
                    "switch"
                } else {
                    "button"
                }
                .to_string(),
                name: b.name.clone(),
                title: b.title.clone(),
                style: format!("{:?}", b.style).to_lowercase(),
                text: String::new(),
                locked: false,
                checked: b.checked.unwrap_or(false),
                x: b.rect.x,
                y: b.rect.y,
                w: b.rect.w,
                h: b.rect.h,
                text_font: b.text_font.clone(),
                text_size: b.text_size,
                text_style: b.text_style.clone(),
                text_align: b.text_align.clone(),
            });
        }
        if let Some(f) = self.find_field(id) {
            return Some(ObjectProps {
                id: f.id,
                kind: "field".to_string(),
                name: f.name.clone(),
                title: String::new(),
                style: String::new(),
                text: f.text.clone(),
                locked: f.locked,
                checked: false,
                x: f.rect.x,
                y: f.rect.y,
                w: f.rect.w,
                h: f.rect.h,
                text_font: f.text_font.clone(),
                text_size: f.text_size,
                text_style: f.text_style.clone(),
                text_align: f.text_align.clone(),
            });
        }
        None
    }

    /// Apply a typed [`ObjectProps`] to its object (typed replacement for the JSON
    /// `set_object_props`). Geometry is ignored here — use `set_object_rect`.
    pub fn apply_object_props(&mut self, p: &ObjectProps) -> bool {
        let bg_id = {
            let card = &mut self.stack.cards[self.card_index];
            if let Some(b) = card.buttons.iter_mut().find(|b| b.id == p.id) {
                apply_button(b, p);
                return true;
            }
            if let Some(f) = card.fields.iter_mut().find(|f| f.id == p.id) {
                apply_field(f, p);
                return true;
            }
            card.background_id
        };
        if let Some(bg_id) = bg_id
            && let Some(bg) = self.stack.backgrounds.iter_mut().find(|b| b.id == bg_id)
        {
            if let Some(b) = bg.buttons.iter_mut().find(|b| b.id == p.id) {
                apply_button(b, p);
                return true;
            }
            if let Some(f) = bg.fields.iter_mut().find(|f| f.id == p.id) {
                apply_field(f, p);
                return true;
            }
        }
        false
    }

    fn find_button(&self, id: u32) -> Option<&Button> {
        let card = &self.stack.cards[self.card_index];
        card.buttons.iter().find(|b| b.id == id).or_else(|| {
            card.background_id
                .and_then(|i| self.stack.background(i))
                .and_then(|bg| bg.buttons.iter().find(|b| b.id == id))
        })
    }

    /// If `id` is a **switch** (a button with `checked: Some`), flip it before its `mouseUp` runs,
    /// so a script-less switch still toggles and `the checked of me` reads the new state (ADR-0015).
    fn toggle_if_switch(&mut self, id: u32) {
        let bg_id = {
            let card = &mut self.stack.cards[self.card_index];
            if let Some(b) = card.buttons.iter_mut().find(|b| b.id == id) {
                if let Some(c) = b.checked {
                    b.checked = Some(!c);
                }
                return;
            }
            card.background_id
        };
        if let Some(bg_id) = bg_id
            && let Some(bg) = self.stack.backgrounds.iter_mut().find(|b| b.id == bg_id)
            && let Some(b) = bg.buttons.iter_mut().find(|b| b.id == id)
            && let Some(c) = b.checked
        {
            b.checked = Some(!c);
        }
    }

    fn find_field(&self, id: u32) -> Option<&Field> {
        let card = &self.stack.cards[self.card_index];
        card.fields.iter().find(|f| f.id == id).or_else(|| {
            card.background_id
                .and_then(|i| self.stack.background(i))
                .and_then(|bg| bg.fields.iter().find(|f| f.id == id))
        })
    }

    /// Fire the `openCard` handler for the current card (card then stack). Call after
    /// load and after navigation.
    pub fn open_current_card(&mut self) -> DispatchResult {
        self.dispatch_message(None, "openCard")
    }

    /// Move directly to a card index (clamped) and fire its `openCard`. Used by hosts
    /// for navigation that isn't script-driven (e.g. the desktop harness's `go`).
    pub fn goto_card(&mut self, index: usize) -> DispatchResult {
        let n = self.stack.cards.len();
        if n == 0 {
            return DispatchResult::nothing();
        }
        self.card_index = index.min(n - 1);
        let mut r = self.open_current_card();
        r.card_changed = true;
        r.needs_redraw = true;
        r
    }

    /// Find a button/field on the current card by name (case-insensitive) or numeric id
    /// and return the center of its rect, for name-based tapping in tools.
    pub fn object_center(&self, key: &str) -> Option<(f32, f32)> {
        let by_id: Option<u32> = key.parse().ok();
        let card = &self.stack.cards[self.card_index];
        let centers = card
            .buttons
            .iter()
            .map(|b| (b.id, &b.name, b.rect))
            .chain(card.fields.iter().map(|f| (f.id, &f.name, f.rect)));
        for (id, name, rect) in centers {
            if Some(id) == by_id || name.eq_ignore_ascii_case(key) {
                return Some((rect.x + rect.w / 2.0, rect.y + rect.h / 2.0));
            }
        }
        None
    }

    /// Hit-test a tap at (x, y) and run the resulting behavior.
    /// `phase` is "up" for a completed tap (the only phase that acts in the MVP).
    pub fn dispatch_touch(&mut self, x: f32, y: f32, phase: &str) -> DispatchResult {
        if phase != "up" && phase != "tap" {
            return DispatchResult::nothing();
        }
        let Some(hit) = self.hit_test(x, y) else {
            return DispatchResult::nothing();
        };
        match hit {
            Hit::Button(id) => {
                self.toggle_if_switch(id);
                let mut r = self.dispatch_message(Some(Me::Button(id)), "mouseUp");
                r.needs_redraw = true;
                r
            }
            Hit::EditableField(id) => {
                let mut r = DispatchResult::nothing();
                r.focus_field = Some(id);
                r
            }
            Hit::LockedField(id) => {
                let mut r = self.dispatch_message(Some(Me::Field(id)), "mouseUp");
                r.needs_redraw = true;
                r
            }
        }
    }

    /// **Id-addressed semantic dispatch** for the native render target (ADR-0008) — the
    /// view-tree analogue of `dispatch_touch`. A widget fires `message` (e.g. `mouseUp`) at the
    /// node `id`; we resolve the object and run the message along the **same** message path
    /// (object → card → background → stack), so id-dispatch and a coordinate tap are behaviorally
    /// identical. `args` is accepted for a future-proof signature (typed/lifecycle args) but is
    /// unused in slice 1. Unknown ids are no-ops.
    pub fn dispatch_by_id(&mut self, id: u32, message: &str, _args: &[String]) -> DispatchResult {
        let Some(me) = self.me_for_id(id) else {
            return DispatchResult::nothing();
        };
        // A switch toggles its state before its mouseUp runs (ADR-0015).
        if matches!(me, Me::Button(_)) && message.eq_ignore_ascii_case("mouseup") {
            self.toggle_if_switch(id);
        }
        let mut r = self.dispatch_message(Some(me), message);
        r.needs_redraw = true;
        r
    }

    /// Resolve a node id to its `Me`, searching card then background, buttons then fields
    /// (mirroring `hit_test`'s z-priority). `None` if no such object is on the current card.
    fn me_for_id(&self, id: u32) -> Option<Me> {
        let card = &self.stack.cards[self.card_index];
        if card.buttons.iter().any(|b| b.id == id) {
            return Some(Me::Button(id));
        }
        if card.fields.iter().any(|f| f.id == id) {
            return Some(Me::Field(id));
        }
        if let Some(bg) = card.background_id.and_then(|i| self.stack.background(i)) {
            if bg.buttons.iter().any(|b| b.id == id) {
                return Some(Me::Button(id));
            }
            if bg.fields.iter().any(|f| f.id == id) {
                return Some(Me::Field(id));
            }
        }
        None
    }

    /// Dispatch a **touchscreen gesture** as a HyperTalk message — the post-WIMP companion to
    /// `dispatch_touch`. `gesture` is a handler name like `tap`, `doubleTap`, `longPress`, or
    /// `swipeLeft`/`swipeRight`/`swipeUp`/`swipeDown` (matched case-insensitively). The message
    /// is sent to the object under the gesture's start point and then **bubbles** card →
    /// background → stack, so a stack-level `on swipeLeft` catches a swipe made anywhere while
    /// an object can still intercept its own. Gestures with no matching handler are no-ops.
    ///
    /// Unlike `dispatch_touch`, a gesture never opens the field editor: long-pressing or
    /// swiping an editable field runs script, it doesn't start typing. (A plain tap on an
    /// unlocked field still goes through `dispatch_touch` for focus.)
    pub fn dispatch_gesture(&mut self, x: f32, y: f32, gesture: &str) -> DispatchResult {
        let origin = self.me_at(x, y);
        let mut r = self.dispatch_message(origin, gesture);
        r.needs_redraw = true;
        r
    }

    /// The topmost object at a card-space point as a `Me`, regardless of lock state (a
    /// gesture targets locked and unlocked objects alike). Mirrors `hit_test`'s z-order.
    fn me_at(&self, x: f32, y: f32) -> Option<Me> {
        self.hit_test(x, y).map(|h| match h {
            Hit::Button(id) => Me::Button(id),
            Hit::EditableField(id) | Hit::LockedField(id) => Me::Field(id),
        })
    }

    fn hit_test(&self, x: f32, y: f32) -> Option<Hit> {
        let card = &self.stack.cards[self.card_index];
        // Topmost first: card buttons, card fields, then background buttons, fields.
        for b in card.buttons.iter().rev() {
            if b.visible && b.rect.contains(x, y) {
                return Some(Hit::Button(b.id));
            }
        }
        for f in card.fields.iter().rev() {
            if f.visible && f.rect.contains(x, y) {
                return Some(field_hit(f.id, f.locked));
            }
        }
        if let Some(bg) = card.background_id.and_then(|id| self.stack.background(id)) {
            for b in bg.buttons.iter().rev() {
                if b.visible && b.rect.contains(x, y) {
                    return Some(Hit::Button(b.id));
                }
            }
            for f in bg.fields.iter().rev() {
                if f.visible && f.rect.contains(x, y) {
                    return Some(field_hit(f.id, f.locked));
                }
            }
        }
        None
    }

    /// Run `message` along the HyperCard message path starting at `origin` (or the card
    /// when `origin` is None): object → card → background → stack. The first matching
    /// handler runs; control flow stops there.
    fn dispatch_message(&mut self, origin: Option<Me>, message: &str) -> DispatchResult {
        let path = self.collect_path(origin);
        let before = self.card_index;

        let mut rt = Runtime::new(&mut self.stack, self.card_index);
        let mut error = None;
        let mut handled = false;
        for (src, me) in &path {
            match rt.run_handler(src, message, *me, &[]) {
                Ok(true) => {
                    handled = true;
                    break;
                }
                Ok(false) => continue,
                Err(e) => {
                    error = Some(e);
                    break;
                }
            }
        }
        let host_cmds: Vec<HostEffect> = rt.host.iter().map(host_effect).collect();
        let new_index = rt.card_index;
        self.card_index = new_index;
        let card_changed = new_index != before;

        DispatchResult {
            needs_redraw: card_changed || !host_cmds.is_empty(),
            card_changed,
            focus_field: None,
            host_cmds,
            error,
            handled,
        }
    }

    /// Dispatch a **lifecycle message** (ADR-0019): `resume`/`suspend`/`backPressed`/`rotate`, fired
    /// by the host at Activity-lifecycle transitions. Sent with no object origin, so it bubbles
    /// card → background → stack (a stack-level `on resume` catches it). `idle` is intentionally
    /// not used (battery). The host inspects `handled` (e.g. to consume a `backPressed`).
    pub fn dispatch_lifecycle(&mut self, message: &str) -> DispatchResult {
        let mut r = self.dispatch_message(None, message);
        r.needs_redraw = r.needs_redraw || r.card_changed;
        r
    }

    /// Gather (script source, `me`) pairs along the message path. Scripts are cloned so
    /// the runtime can borrow the stack mutably afterward.
    fn collect_path(&self, origin: Option<Me>) -> Vec<(String, Me)> {
        let card = &self.stack.cards[self.card_index];
        let mut path = Vec::new();

        if let Some(me) = origin {
            // The tapped object may live on the card layer or the shared background layer.
            let bg = card.background_id.and_then(|id| self.stack.background(id));
            let src = match me {
                Me::Button(id) => card
                    .buttons
                    .iter()
                    .find(|b| b.id == id)
                    .map(|b| b.script.clone())
                    .or_else(|| {
                        bg.and_then(|b| {
                            b.buttons
                                .iter()
                                .find(|x| x.id == id)
                                .map(|x| x.script.clone())
                        })
                    }),
                Me::Field(id) => card
                    .fields
                    .iter()
                    .find(|f| f.id == id)
                    .map(|f| f.script.clone())
                    .or_else(|| {
                        bg.and_then(|b| {
                            b.fields
                                .iter()
                                .find(|x| x.id == id)
                                .map(|x| x.script.clone())
                        })
                    }),
                _ => None,
            };
            if let Some(src) = src {
                path.push((src, me));
            }
        }

        path.push((card.script.clone(), Me::Card));
        if let Some(bg) = card.background_id.and_then(|id| self.stack.background(id)) {
            path.push((bg.script.clone(), Me::Background(bg.id)));
        }
        path.push((self.stack.script.clone(), Me::Stack));
        path
    }
}

enum Hit {
    Button(u32),
    EditableField(u32),
    LockedField(u32),
}

/// Apply a typed [`ObjectProps`] to a button (name/title/style + text styling; not geometry).
fn apply_button(b: &mut Button, p: &ObjectProps) {
    b.name = p.name.clone();
    b.title = p.title.clone();
    b.style = parse_style(&p.style);
    // Only switches carry `checked`; the inspector can't turn a plain button into a switch.
    if b.checked.is_some() {
        b.checked = Some(p.checked);
    }
    b.text_font = p.text_font.clone();
    b.text_size = p.text_size;
    b.text_style = p.text_style.clone();
    b.text_align = p.text_align.clone();
}

/// Apply a typed [`ObjectProps`] to a field (name/text/locked + text styling; not geometry).
fn apply_field(f: &mut Field, p: &ObjectProps) {
    f.name = p.name.clone();
    f.text = p.text.clone();
    f.locked = p.locked;
    f.text_font = p.text_font.clone();
    f.text_size = p.text_size;
    f.text_style = p.text_style.clone();
    f.text_align = p.text_align.clone();
}

fn apply_button_props(b: &mut Button, v: &serde_json::Value) {
    if let Some(s) = v.get("name").and_then(|x| x.as_str()) {
        b.name = s.to_string();
    }
    if let Some(s) = v.get("title").and_then(|x| x.as_str()) {
        b.title = s.to_string();
    }
    if let Some(s) = v.get("style").and_then(|x| x.as_str()) {
        b.style = parse_style(s);
    }
    apply_text_attrs(
        &mut b.text_font,
        &mut b.text_size,
        &mut b.text_style,
        &mut b.text_align,
        v,
    );
}

fn apply_field_props(f: &mut Field, v: &serde_json::Value) {
    if let Some(s) = v.get("name").and_then(|x| x.as_str()) {
        f.name = s.to_string();
    }
    if let Some(s) = v.get("text").and_then(|x| x.as_str()) {
        f.text = s.to_string();
    }
    if let Some(b) = v.get("locked").and_then(|x| x.as_bool()) {
        f.locked = b;
    }
    apply_text_attrs(
        &mut f.text_font,
        &mut f.text_size,
        &mut f.text_style,
        &mut f.text_align,
        v,
    );
}

/// Apply the four text-styling keys (any subset) to an object's fields. Shared by buttons
/// and fields. `text_size` accepts a JSON number or a numeric string.
fn apply_text_attrs(
    font: &mut String,
    size: &mut f32,
    style: &mut String,
    align: &mut String,
    v: &serde_json::Value,
) {
    if let Some(s) = v.get("text_font").and_then(|x| x.as_str()) {
        *font = s.to_string();
    }
    if let Some(ts) = v.get("text_size") {
        if let Some(n) = ts.as_f64() {
            *size = n as f32;
        } else if let Some(n) = ts.as_str().and_then(|s| s.trim().parse::<f32>().ok()) {
            *size = n;
        }
    }
    if let Some(s) = v.get("text_style").and_then(|x| x.as_str()) {
        *style = s.to_string();
    }
    if let Some(s) = v.get("text_align").and_then(|x| x.as_str()) {
        *align = s.to_string();
    }
}

fn parse_style(s: &str) -> ButtonStyle {
    match s.to_ascii_lowercase().as_str() {
        "rectangle" => ButtonStyle::Rectangle,
        "transparent" => ButtonStyle::Transparent,
        _ => ButtonStyle::Rounded,
    }
}

fn field_hit(id: u32, locked: bool) -> Hit {
    if locked {
        Hit::LockedField(id)
    } else {
        Hit::EditableField(id)
    }
}

fn field_cmd(f: &crate::model::Field) -> DrawCmd {
    DrawCmd {
        kind: "field".to_string(),
        id: f.id,
        x: f.rect.x,
        y: f.rect.y,
        w: f.rect.w,
        h: f.rect.h,
        text: f.text.clone(),
        style: String::new(),
        visible: f.visible,
        locked: f.locked,
        text_font: f.text_font.clone(),
        text_size: f.text_size,
        text_style: f.text_style.clone(),
        text_align: f.text_align.clone(),
    }
}

fn button_cmd(b: &crate::model::Button) -> DrawCmd {
    // A switch (checked: Some) shows its state on the Canvas target with a ☑/☐ prefix; the
    // native target renders a real Material Switch (ADR-0015).
    let text = match b.checked {
        Some(true) => format!("\u{2611} {}", b.label()),
        Some(false) => format!("\u{2610} {}", b.label()),
        None => b.label().to_string(),
    };
    DrawCmd {
        kind: "button".to_string(),
        id: b.id,
        x: b.rect.x,
        y: b.rect.y,
        w: b.rect.w,
        h: b.rect.h,
        text,
        style: format!("{:?}", b.style).to_lowercase(),
        visible: b.visible,
        locked: false,
        text_font: b.text_font.clone(),
        text_size: b.text_size,
        text_style: b.text_style.clone(),
        text_align: b.text_align.clone(),
    }
}

/// The largest object id on a card and its background — the base for allocating collision-free
/// synthetic group ids in the layout overlay (ADR-0014). 0 if the card and bg have no objects.
fn max_object_id(card: &crate::model::Card, bg: Option<&crate::model::Background>) -> u32 {
    let card_ids = card
        .fields
        .iter()
        .map(|f| f.id)
        .chain(card.buttons.iter().map(|b| b.id));
    let bg_ids = bg.into_iter().flat_map(|bg| {
        bg.fields
            .iter()
            .map(|f| f.id)
            .chain(bg.buttons.iter().map(|b| b.id))
    });
    card_ids.chain(bg_ids).max().unwrap_or(0)
}

/// Build a field [`ViewNode`] with abstract props only (no geometry — ADR-0008).
fn field_node(f: &crate::model::Field) -> ViewNode {
    ViewNode {
        id: f.id,
        kind: "field".to_string(),
        child_ids: Vec::new(),
        props: vec![
            Prop {
                key: "text".to_string(),
                value: f.text.clone(),
            },
            Prop {
                key: "locked".to_string(),
                value: f.locked.to_string(),
            },
            Prop {
                key: "visible".to_string(),
                value: f.visible.to_string(),
            },
            Prop {
                key: "font".to_string(),
                value: f.text_font.clone(),
            },
            Prop {
                key: "size".to_string(),
                value: f.text_size.to_string(),
            },
            Prop {
                key: "textStyle".to_string(),
                value: f.text_style.clone(),
            },
            Prop {
                key: "align".to_string(),
                value: f.text_align.clone(),
            },
            Prop {
                key: "textRole".to_string(),
                value: f.text_role.clone(),
            },
        ],
    }
}

/// Build a button [`ViewNode`]. `style` is the existing abstract `ButtonStyle` string
/// (`rounded`/`rectangle`/`transparent`); the host maps it to a Material widget. A button with
/// `checked: Some` is a **switch** (kind `"switch"` + a `checked` prop; ADR-0015).
fn button_node(b: &crate::model::Button) -> ViewNode {
    let mut props = vec![
        Prop {
            key: "title".to_string(),
            value: b.label().to_string(),
        },
        Prop {
            key: "style".to_string(),
            value: format!("{:?}", b.style).to_lowercase(),
        },
        Prop {
            key: "role".to_string(),
            value: b.role.clone(),
        },
        Prop {
            key: "visible".to_string(),
            value: b.visible.to_string(),
        },
        Prop {
            key: "font".to_string(),
            value: b.text_font.clone(),
        },
        Prop {
            key: "size".to_string(),
            value: b.text_size.to_string(),
        },
        Prop {
            key: "textStyle".to_string(),
            value: b.text_style.clone(),
        },
        Prop {
            key: "align".to_string(),
            value: b.text_align.clone(),
        },
    ];
    if let Some(checked) = b.checked {
        props.push(Prop {
            key: "checked".to_string(),
            value: checked.to_string(),
        });
    }
    ViewNode {
        id: b.id,
        kind: if b.checked.is_some() {
            "switch".to_string()
        } else {
            "button".to_string()
        },
        child_ids: Vec::new(),
        props,
    }
}

/// No-layout (`free`) projection: append a node's absolute geometry and push it as a root
/// (ADR-0017) — used when a card has no layout overlay so native mirrors the classic Canvas.
fn push_free_node(
    nodes: &mut Vec<ViewNode>,
    roots: &mut Vec<u32>,
    mut node: ViewNode,
    rect: crate::model::Rect,
) {
    for (k, v) in [("x", rect.x), ("y", rect.y), ("w", rect.w), ("h", rect.h)] {
        node.props.push(Prop {
            key: k.to_string(),
            value: v.to_string(),
        });
    }
    roots.push(node.id);
    nodes.push(node);
}

/// Resolve a layout-overlay object reference to a [`ViewNode`] (with a `weight` prop appended),
/// searching the card then the background. `None` if no object on the card/bg has that id —
/// such a reference is silently skipped (ADR-0014's unreferenced/dangling caveat).
fn object_node(
    card: &crate::model::Card,
    bg: Option<&crate::model::Background>,
    id: u32,
    geometry: bool,
) -> Option<ViewNode> {
    // Find the object once, capturing its node, weight, and rect (for free-mode geometry).
    let found = card
        .fields
        .iter()
        .find(|f| f.id == id)
        .map(|f| (field_node(f), f.weight, f.rect))
        .or_else(|| {
            card.buttons
                .iter()
                .find(|b| b.id == id)
                .map(|b| (button_node(b), b.weight, b.rect))
        })
        .or_else(|| {
            bg.and_then(|bg| {
                bg.fields
                    .iter()
                    .find(|f| f.id == id)
                    .map(|f| (field_node(f), f.weight, f.rect))
                    .or_else(|| {
                        bg.buttons
                            .iter()
                            .find(|b| b.id == id)
                            .map(|b| (button_node(b), b.weight, b.rect))
                    })
            })
        });
    let (mut node, weight, rect) = found?;
    node.props.push(Prop {
        key: "weight".to_string(),
        value: weight.to_string(),
    });
    if geometry {
        // `free` layout (ADR-0017): the host places the object by its authored card-unit rect.
        for (k, val) in [("x", rect.x), ("y", rect.y), ("w", rect.w), ("h", rect.h)] {
            node.props.push(Prop {
                key: k.to_string(),
                value: val.to_string(),
            });
        }
    }
    Some(node)
}

/// Walk a layout group's children into [`ViewNode`]s (ADR-0014), returning the ordered child-id
/// list for the container. Object refs reuse [`object_node`]; nested groups get a synthetic id
/// (allocated from `next_group_id`) and recurse. Dangling object refs are skipped.
fn project_children(
    card: &crate::model::Card,
    bg: Option<&crate::model::Background>,
    children: &[crate::model::LayoutChild],
    next_group_id: &mut u32,
    nodes: &mut Vec<ViewNode>,
    geometry: bool,
) -> Vec<u32> {
    use crate::model::LayoutChild;
    let mut ids = Vec::new();
    for child in children {
        match child {
            LayoutChild::Object(id) => {
                if let Some(node) = object_node(card, bg, *id, geometry) {
                    ids.push(node.id);
                    nodes.push(node);
                }
            }
            LayoutChild::Group(g) => {
                let gid = *next_group_id;
                *next_group_id += 1;
                let child_ids =
                    project_children(card, bg, &g.children, next_group_id, nodes, geometry);
                nodes.push(ViewNode {
                    id: gid,
                    kind: "group".to_string(),
                    child_ids,
                    props: vec![
                        Prop {
                            key: "mode".to_string(),
                            value: g.mode.clone(),
                        },
                        Prop {
                            key: "padding".to_string(),
                            value: g.padding.to_string(),
                        },
                        Prop {
                            key: "weight".to_string(),
                            value: g.weight.to_string(),
                        },
                        Prop {
                            key: "columns".to_string(),
                            value: g.columns.to_string(),
                        },
                    ],
                });
                ids.push(gid);
            }
        }
    }
    ids
}

fn host_effect(c: &HostCmd) -> HostEffect {
    match c {
        HostCmd::Answer(s) => HostEffect::Answer(s.clone()),
        HostCmd::Message(s) => HostEffect::Message(s.clone()),
        HostCmd::Beep => HostEffect::Beep,
        HostCmd::GoStack(s) => HostEffect::GoStack(s.clone()),
        HostCmd::ShowStacks => HostEffect::ShowStacks,
    }
}
