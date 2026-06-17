//! Typed UniFFI bridge (ADR-0012). A [`HyperStack`] wraps a `hypercore::Session` and exposes the
//! whole host surface as typed calls, replacing the hand-written JSON-string JNI bridge. Object
//! props are still passed as a JSON string (the inspector blob) pending their own typed shape.
//!
//! Ids cross as `i32` (not `u32`) so the generated Kotlin uses `Int`, matching the host; `-1`
//! means "none".

use std::sync::{Arc, Mutex};

use hypercore::Session;

/// Error thrown across the UniFFI boundary (e.g. a stack failed to parse). The field is named
/// `reason` (not `message`) so the generated Kotlin doesn't clash with `Throwable.message`.
#[derive(Debug, uniffi::Error)]
pub enum BridgeError {
    Load { reason: String },
}

impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BridgeError::Load { reason } => write!(f, "{reason}"),
        }
    }
}

impl std::error::Error for BridgeError {}

/// One draw primitive for the current card — mirrors `hypercore::DrawCmd` as a UniFFI record.
#[derive(uniffi::Record)]
pub struct DrawItem {
    pub kind: String,
    pub id: i32,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub text: String,
    pub style: String,
    pub visible: bool,
    pub locked: bool,
    pub text_font: String,
    pub text_size: f32,
    pub text_style: String,
    pub text_align: String,
}

/// The draw list for the current card — mirrors `hypercore::RenderList`.
#[derive(uniffi::Record)]
pub struct RenderList {
    pub stack_name: String,
    pub card_name: String,
    pub card_index: i32,
    pub card_count: i32,
    pub width: f32,
    pub height: f32,
    pub items: Vec<DrawItem>,
}

impl From<hypercore::DrawCmd> for DrawItem {
    fn from(d: hypercore::DrawCmd) -> Self {
        DrawItem {
            kind: d.kind,
            id: d.id as i32,
            x: d.x,
            y: d.y,
            w: d.w,
            h: d.h,
            text: d.text,
            style: d.style,
            visible: d.visible,
            locked: d.locked,
            text_font: d.text_font,
            text_size: d.text_size,
            text_style: d.text_style,
            text_align: d.text_align,
        }
    }
}

impl From<hypercore::RenderList> for RenderList {
    fn from(r: hypercore::RenderList) -> Self {
        RenderList {
            stack_name: r.stack_name,
            card_name: r.card_name,
            card_index: r.card_index as i32,
            card_count: r.card_count as i32,
            width: r.width,
            height: r.height,
            items: r.items.into_iter().map(DrawItem::from).collect(),
        }
    }
}

/// A side effect the host performs after a dispatch — mirrors `hypercore::HostEffect`.
#[derive(uniffi::Enum)]
pub enum HostEffect {
    Answer { text: String },
    Message { text: String },
    Beep,
    GoStack { name: String },
    ShowStacks,
}

impl From<hypercore::HostEffect> for HostEffect {
    fn from(e: hypercore::HostEffect) -> Self {
        match e {
            hypercore::HostEffect::Answer(text) => HostEffect::Answer { text },
            hypercore::HostEffect::Message(text) => HostEffect::Message { text },
            hypercore::HostEffect::Beep => HostEffect::Beep,
            hypercore::HostEffect::GoStack(name) => HostEffect::GoStack { name },
            hypercore::HostEffect::ShowStacks => HostEffect::ShowStacks,
        }
    }
}

/// Result of a dispatch (tap/gesture/openCard) — mirrors `hypercore::DispatchResult`.
#[derive(uniffi::Record)]
pub struct DispatchResult {
    pub needs_redraw: bool,
    pub card_changed: bool,
    pub focus_field: Option<i32>,
    pub host_cmds: Vec<HostEffect>,
    pub error: Option<String>,
}

impl From<hypercore::DispatchResult> for DispatchResult {
    fn from(r: hypercore::DispatchResult) -> Self {
        DispatchResult {
            needs_redraw: r.needs_redraw,
            card_changed: r.card_changed,
            focus_field: r.focus_field.map(|id| id as i32),
            host_cmds: r.host_cmds.into_iter().map(HostEffect::from).collect(),
            error: r.error,
        }
    }
}

/// Validate HyperTalk source without running it; returns the parser error, or "" if it parses.
#[uniffi::export]
pub fn check_script(src: String) -> String {
    Session::check_script(&src).unwrap_or_default()
}

/// A loaded stack the host drives. Wraps `Session` behind a `Mutex` — UniFFI shares objects as
/// `Arc<Self>` (must be `Send + Sync`) and exposes `&self` methods, while `Session` needs `&mut`
/// for dispatch/edits, so interior mutability is required.
#[derive(uniffi::Object)]
pub struct HyperStack {
    inner: Mutex<Session>,
}

#[uniffi::export]
impl HyperStack {
    /// Load a stack from YAML (the authoring format, ADR-0011).
    #[uniffi::constructor]
    pub fn load_yaml(yaml: String) -> Result<Arc<HyperStack>, BridgeError> {
        Self::wrap(Session::load_from_yaml(&yaml))
    }

    /// Load a stack from JSON (legacy/compat).
    #[uniffi::constructor]
    pub fn load_json(json: String) -> Result<Arc<HyperStack>, BridgeError> {
        Self::wrap(Session::load_from_json(&json))
    }

    /// The draw list for the current card — the typed replacement for `nativeRender`'s JSON.
    pub fn render_current_card(&self) -> RenderList {
        self.inner.lock().unwrap().render_current_card().into()
    }

    /// Fire the current card's `openCard` handler (run after navigation).
    pub fn open_card(&self) -> DispatchResult {
        self.inner.lock().unwrap().open_current_card().into()
    }

    /// Dispatch a touch at a card-space point; `phase` is "up" for a completed tap.
    pub fn dispatch_touch(&self, x: f32, y: f32, phase: String) -> DispatchResult {
        self.inner
            .lock()
            .unwrap()
            .dispatch_touch(x, y, &phase)
            .into()
    }

    /// Dispatch a touchscreen gesture (`tap`/`doubleTap`/`longPress`/`swipe*`).
    pub fn dispatch_gesture(&self, x: f32, y: f32, gesture: String) -> DispatchResult {
        self.inner
            .lock()
            .unwrap()
            .dispatch_gesture(x, y, &gesture)
            .into()
    }

    /// Set a field's text by id (host-edited); true if a field changed.
    pub fn set_field_text(&self, field_id: i32, text: String) -> bool {
        self.inner
            .lock()
            .unwrap()
            .set_field_text(field_id as u32, &text)
    }

    /// Topmost object id at a card-space point (edit-mode selection), or -1.
    pub fn object_at(&self, x: f32, y: f32) -> i32 {
        self.inner
            .lock()
            .unwrap()
            .object_at(x, y)
            .map(|id| id as i32)
            .unwrap_or(-1)
    }

    /// Read an object's HyperTalk source by id ("" if it doesn't exist).
    pub fn object_script(&self, object_id: i32) -> String {
        self.inner
            .lock()
            .unwrap()
            .get_object_script(object_id as u32)
            .unwrap_or_default()
    }

    /// Write an object's HyperTalk source by id; true if updated.
    pub fn set_object_script(&self, object_id: i32, src: String) -> bool {
        self.inner
            .lock()
            .unwrap()
            .set_object_script(object_id as u32, &src)
    }

    /// Create a "button" or "field" on the current card; returns the new id, or -1.
    pub fn add_object(&self, kind: String) -> i32 {
        self.inner
            .lock()
            .unwrap()
            .add_object(&kind)
            .map(|id| id as i32)
            .unwrap_or(-1)
    }

    /// Delete an object by id; true if one was removed.
    pub fn delete_object(&self, object_id: i32) -> bool {
        self.inner.lock().unwrap().delete_object(object_id as u32)
    }

    /// Move/resize an object by id (drag commit); true if one was updated.
    pub fn set_object_rect(&self, object_id: i32, x: f32, y: f32, w: f32, h: f32) -> bool {
        self.inner
            .lock()
            .unwrap()
            .set_object_rect(object_id as u32, x, y, w, h)
    }

    /// Read an object's editable properties as a JSON blob ("" if it doesn't exist). Still JSON
    /// pending a typed props record.
    pub fn object_props(&self, object_id: i32) -> String {
        self.inner
            .lock()
            .unwrap()
            .get_object_props(object_id as u32)
            .unwrap_or_default()
    }

    /// Apply a JSON property blob to an object; true if the object was found.
    pub fn set_object_props(&self, object_id: i32, props_json: String) -> bool {
        self.inner
            .lock()
            .unwrap()
            .set_object_props(object_id as u32, &props_json)
    }

    /// Serialize the current stack to YAML (for saving the per-stack working copy).
    pub fn to_yaml(&self) -> String {
        self.inner.lock().unwrap().to_yaml()
    }
}

impl HyperStack {
    fn wrap(loaded: Result<Session, String>) -> Result<Arc<HyperStack>, BridgeError> {
        let session = loaded.map_err(|reason| BridgeError::Load { reason })?;
        Ok(Arc::new(HyperStack {
            inner: Mutex::new(session),
        }))
    }
}
