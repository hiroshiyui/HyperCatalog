//! Typed UniFFI bridge (ADR-0012, stage 2). A [`HyperStack`] wraps a `hypercore::Session` and
//! exposes the **read path** with typed records, replacing the JSON-string JNI surface. Dispatch
//! and authoring move over in later stages; until then they stay on the hand-written JNI bridge.

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
    pub id: u32,
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

/// The draw list for the current card — mirrors `hypercore::RenderList`. (`card_index`/`count`
/// narrow `usize` → `u32`, which UniFFI supports.)
#[derive(uniffi::Record)]
pub struct RenderList {
    pub stack_name: String,
    pub card_name: String,
    pub card_index: u32,
    pub card_count: u32,
    pub width: f32,
    pub height: f32,
    pub items: Vec<DrawItem>,
}

impl From<hypercore::DrawCmd> for DrawItem {
    fn from(d: hypercore::DrawCmd) -> Self {
        DrawItem {
            kind: d.kind,
            id: d.id,
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
            card_index: r.card_index as u32,
            card_count: r.card_count as u32,
            width: r.width,
            height: r.height,
            items: r.items.into_iter().map(DrawItem::from).collect(),
        }
    }
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
        let session = Session::load_from_yaml(&yaml)
            .map_err(|message| BridgeError::Load { reason: message })?;
        Ok(Arc::new(HyperStack {
            inner: Mutex::new(session),
        }))
    }

    /// Load a stack from JSON (legacy/compat).
    #[uniffi::constructor]
    pub fn load_json(json: String) -> Result<Arc<HyperStack>, BridgeError> {
        let session = Session::load_from_json(&json)
            .map_err(|message| BridgeError::Load { reason: message })?;
        Ok(Arc::new(HyperStack {
            inner: Mutex::new(session),
        }))
    }

    /// The draw list for the current card — the typed replacement for `nativeRender`'s JSON.
    pub fn render_current_card(&self) -> RenderList {
        self.inner.lock().unwrap().render_current_card().into()
    }
}
