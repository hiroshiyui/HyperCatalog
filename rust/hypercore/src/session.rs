//! The host-facing facade. A `Session` owns a `Stack` and the current card index, and
//! exposes the small surface the platform host (Android/desktop) drives: render the
//! current card, dispatch a touch, edit a field, navigate, and serialize for saving.
//!
//! The boundary is deliberately data-only (serde structs) so it can cross JNI as JSON.

use serde::Serialize;

use crate::model::Stack;
use crate::script::{HostCmd, Me, Runtime};

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
}

/// A host effect to perform after a dispatch (dialog, beep, message-box output).
#[derive(Debug, Serialize, PartialEq)]
#[serde(tag = "type", content = "text", rename_all = "lowercase")]
pub enum HostEffect {
    Answer(String),
    Message(String),
    Beep,
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
}

impl DispatchResult {
    fn nothing() -> DispatchResult {
        DispatchResult {
            needs_redraw: false,
            card_changed: false,
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
        for (src, me) in &path {
            match rt.run_handler(src, message, *me, &[]) {
                Ok(true) => break,
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
        }
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
    }
}

fn button_cmd(b: &crate::model::Button) -> DrawCmd {
    DrawCmd {
        kind: "button".to_string(),
        id: b.id,
        x: b.rect.x,
        y: b.rect.y,
        w: b.rect.w,
        h: b.rect.h,
        text: b.label().to_string(),
        style: format!("{:?}", b.style).to_lowercase(),
        visible: b.visible,
        locked: false,
    }
}

fn host_effect(c: &HostCmd) -> HostEffect {
    match c {
        HostCmd::Answer(s) => HostEffect::Answer(s.clone()),
        HostCmd::Message(s) => HostEffect::Message(s.clone()),
        HostCmd::Beep => HostEffect::Beep,
    }
}
