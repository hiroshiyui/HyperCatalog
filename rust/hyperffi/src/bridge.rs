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

/// One abstract property of a view node — mirrors `hypercore::Prop`.
#[derive(uniffi::Record)]
pub struct ViewProp {
    pub key: String,
    pub value: String,
}

/// One node of the semantic view tree (native render target) — mirrors `hypercore::ViewNode`.
#[derive(uniffi::Record)]
pub struct ViewNode {
    pub id: i32,
    pub kind: String,
    pub props: Vec<ViewProp>,
    pub child_ids: Vec<i32>,
}

/// The semantic view tree for the current card (ADR-0008) — mirrors `hypercore::ViewTree`. The
/// flat-node alternate to `RenderList`; the host realizes it as Material widgets. No geometry.
#[derive(uniffi::Record)]
pub struct ViewTree {
    pub stack_name: String,
    pub card_name: String,
    pub card_index: i32,
    pub card_count: i32,
    /// Root container arrangement ("column"/"row") + padding (ADR-0014).
    pub layout: String,
    pub padding: f32,
    pub root_ids: Vec<i32>,
    pub nodes: Vec<ViewNode>,
}

impl From<hypercore::Prop> for ViewProp {
    fn from(p: hypercore::Prop) -> Self {
        ViewProp {
            key: p.key,
            value: p.value,
        }
    }
}

impl From<hypercore::ViewNode> for ViewNode {
    fn from(n: hypercore::ViewNode) -> Self {
        ViewNode {
            id: n.id as i32,
            kind: n.kind,
            props: n.props.into_iter().map(ViewProp::from).collect(),
            child_ids: n.child_ids.into_iter().map(|i| i as i32).collect(),
        }
    }
}

impl From<hypercore::ViewTree> for ViewTree {
    fn from(t: hypercore::ViewTree) -> Self {
        ViewTree {
            stack_name: t.stack_name,
            card_name: t.card_name,
            card_index: t.card_index as i32,
            card_count: t.card_count as i32,
            layout: t.layout,
            padding: t.padding,
            root_ids: t.root_ids.into_iter().map(|i| i as i32).collect(),
            nodes: t.nodes.into_iter().map(ViewNode::from).collect(),
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

/// An object's editable properties for the inspector — mirrors `hypercore::ObjectProps`.
#[derive(uniffi::Record)]
pub struct ObjectProps {
    pub id: i32,
    pub kind: String,
    pub name: String,
    pub title: String,
    pub style: String,
    pub text: String,
    pub locked: bool,
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

impl From<hypercore::ObjectProps> for ObjectProps {
    fn from(p: hypercore::ObjectProps) -> Self {
        ObjectProps {
            id: p.id as i32,
            kind: p.kind,
            name: p.name,
            title: p.title,
            style: p.style,
            text: p.text,
            locked: p.locked,
            checked: p.checked,
            x: p.x,
            y: p.y,
            w: p.w,
            h: p.h,
            text_font: p.text_font,
            text_size: p.text_size,
            text_style: p.text_style,
            text_align: p.text_align,
        }
    }
}

impl From<ObjectProps> for hypercore::ObjectProps {
    fn from(p: ObjectProps) -> Self {
        hypercore::ObjectProps {
            id: p.id as u32,
            kind: p.kind,
            name: p.name,
            title: p.title,
            style: p.style,
            text: p.text,
            locked: p.locked,
            checked: p.checked,
            x: p.x,
            y: p.y,
            w: p.w,
            h: p.h,
            text_font: p.text_font,
            text_size: p.text_size,
            text_style: p.text_style,
            text_align: p.text_align,
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

    /// The semantic view tree for the current card — the native (Material) render target
    /// (ADR-0008), beside `render_current_card`. Host picks one per render mode.
    pub fn render_view_tree(&self) -> ViewTree {
        self.inner.lock().unwrap().render_view_tree().into()
    }

    /// Id-addressed semantic dispatch (ADR-0008) — the view-tree analogue of `dispatch_touch`.
    /// A native widget fires `message` (e.g. `mouseUp`) at node `id`; runs the same message path.
    pub fn dispatch(&self, id: i32, message: String, args: Vec<String>) -> DispatchResult {
        self.inner
            .lock()
            .unwrap()
            .dispatch_by_id(id as u32, &message, &args)
            .into()
    }

    /// Fire the current card's `openCard` handler (run after navigation).
    pub fn open_card(&self) -> DispatchResult {
        self.inner.lock().unwrap().open_current_card().into()
    }

    /// The current card's 0-based index. The host persists this as *session* view state
    /// (ADR-0013) — it is not part of the stack document.
    pub fn current_card_index(&self) -> i32 {
        self.inner.lock().unwrap().card_index() as i32
    }

    /// Navigate to a 0-based card index (clamped to the stack's range; negatives → 0) and fire
    /// its `openCard`. Used by the host to restore the last-viewed card on reopen (ADR-0013).
    pub fn open_card_at(&self, index: i32) -> DispatchResult {
        self.inner
            .lock()
            .unwrap()
            .goto_card(index.max(0) as usize)
            .into()
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

    /// Read an object's editable properties as a typed record, or null if it doesn't exist.
    pub fn object_props(&self, object_id: i32) -> Option<ObjectProps> {
        self.inner
            .lock()
            .unwrap()
            .object_props(object_id as u32)
            .map(ObjectProps::from)
    }

    /// Apply a typed property record to its object; true if the object was found.
    pub fn set_object_props(&self, props: ObjectProps) -> bool {
        self.inner.lock().unwrap().apply_object_props(&props.into())
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
