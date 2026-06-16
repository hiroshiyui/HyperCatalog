//! Android JNI surface for `hypercore::Session`.
//!
//! The Kotlin side (`org.ghostsinthelab.app.hypercatalog.NativeBridge`) holds an opaque
//! `long` handle that is really a `Box<Session>` pointer. All structured data crosses the
//! boundary as JSON strings, matching the small, event-driven bridge in the design.

use jni::JNIEnv;
use jni::objects::{JClass, JString};
use jni::sys::{JNI_FALSE, JNI_TRUE, jboolean, jfloat, jint, jlong, jstring};

use hypercore::Session;

/// Reconstitute the `Session` behind a handle. The handle must come from `nativeLoad`
/// and not yet have been freed.
unsafe fn session<'a>(handle: jlong) -> Option<&'a mut Session> {
    if handle == 0 {
        None
    } else {
        Some(unsafe { &mut *(handle as *mut Session) })
    }
}

fn rust_string(env: &mut JNIEnv, s: &JString) -> String {
    env.get_string(s).map(|js| js.into()).unwrap_or_default()
}

/// Build a Java string, returning null on failure.
fn java_string(env: &mut JNIEnv, s: &str) -> jstring {
    match env.new_string(s) {
        Ok(js) => js.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Load a stack from JSON. Returns a handle, or 0 on error.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_ghostsinthelab_app_hypercatalog_NativeBridge_nativeLoad(
    mut env: JNIEnv,
    _class: JClass,
    json: JString,
) -> jlong {
    let json = rust_string(&mut env, &json);
    match Session::load_from_json(&json) {
        Ok(s) => Box::into_raw(Box::new(s)) as jlong,
        Err(_) => 0,
    }
}

/// Fire the current card's `openCard` handler; returns a DispatchResult JSON.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_ghostsinthelab_app_hypercatalog_NativeBridge_nativeOpenCard(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jstring {
    let Some(s) = (unsafe { session(handle) }) else {
        return java_string(&mut env, "{}");
    };
    let r = s.open_current_card();
    java_string(&mut env, &serde_json::to_string(&r).unwrap_or_default())
}

/// Render the current card; returns a RenderList JSON.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_ghostsinthelab_app_hypercatalog_NativeBridge_nativeRender(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jstring {
    let Some(s) = (unsafe { session(handle) }) else {
        return java_string(&mut env, "{}");
    };
    let rl = s.render_current_card();
    java_string(&mut env, &serde_json::to_string(&rl).unwrap_or_default())
}

/// Dispatch a touch at (x, y) with the given phase; returns a DispatchResult JSON.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_ghostsinthelab_app_hypercatalog_NativeBridge_nativeDispatchTouch(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    x: jfloat,
    y: jfloat,
    phase: JString,
) -> jstring {
    let phase = rust_string(&mut env, &phase);
    let Some(s) = (unsafe { session(handle) }) else {
        return java_string(&mut env, "{}");
    };
    let r = s.dispatch_touch(x, y, &phase);
    java_string(&mut env, &serde_json::to_string(&r).unwrap_or_default())
}

/// Set a field's text by id. Returns true if a field was updated.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_ghostsinthelab_app_hypercatalog_NativeBridge_nativeSetFieldText(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    field_id: jint,
    text: JString,
) -> jboolean {
    let text = rust_string(&mut env, &text);
    let Some(s) = (unsafe { session(handle) }) else {
        return JNI_FALSE;
    };
    if s.set_field_text(field_id as u32, &text) {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

/// Serialize the current stack to JSON (for saving).
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_ghostsinthelab_app_hypercatalog_NativeBridge_nativeToJson(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jstring {
    let Some(s) = (unsafe { session(handle) }) else {
        return java_string(&mut env, "{}");
    };
    let json = s.to_json();
    java_string(&mut env, &json)
}

/// Release the handle. Safe to call once per handle.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_ghostsinthelab_app_hypercatalog_NativeBridge_nativeFree(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    if handle != 0 {
        unsafe {
            drop(Box::from_raw(handle as *mut Session));
        }
    }
}
