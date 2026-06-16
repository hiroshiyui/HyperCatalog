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

/// Dispatch a touchscreen gesture (`tap`/`doubleTap`/`longPress`/`swipeLeft`/…) at (x, y);
/// returns a DispatchResult JSON. The named message bubbles object → card → background → stack.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_ghostsinthelab_app_hypercatalog_NativeBridge_nativeDispatchGesture(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    x: jfloat,
    y: jfloat,
    gesture: JString,
) -> jstring {
    let gesture = rust_string(&mut env, &gesture);
    let Some(s) = (unsafe { session(handle) }) else {
        return java_string(&mut env, "{}");
    };
    let r = s.dispatch_gesture(x, y, &gesture);
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

/// Topmost object id at a card-space point, for edit-mode selection. Returns the id, or
/// -1 if no object is hit (or the handle is dead).
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_ghostsinthelab_app_hypercatalog_NativeBridge_nativeObjectAt(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    x: jfloat,
    y: jfloat,
) -> jint {
    let Some(s) = (unsafe { session(handle) }) else {
        return -1;
    };
    s.object_at(x, y).map(|id| id as jint).unwrap_or(-1)
}

/// Read an object's HyperTalk source by id. Returns the source, or empty string if the
/// object doesn't exist (or the handle is dead).
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_ghostsinthelab_app_hypercatalog_NativeBridge_nativeGetObjectScript(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    object_id: jint,
) -> jstring {
    let Some(s) = (unsafe { session(handle) }) else {
        return java_string(&mut env, "");
    };
    java_string(
        &mut env,
        &s.get_object_script(object_id as u32).unwrap_or_default(),
    )
}

/// Write an object's HyperTalk source by id. Returns true if an object was updated. The
/// host should validate with `nativeCheckScript` first.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_ghostsinthelab_app_hypercatalog_NativeBridge_nativeSetObjectScript(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    object_id: jint,
    src: JString,
) -> jboolean {
    let src = rust_string(&mut env, &src);
    let Some(s) = (unsafe { session(handle) }) else {
        return JNI_FALSE;
    };
    if s.set_object_script(object_id as u32, &src) {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

/// Validate HyperTalk source without running it. Returns the parser error message, or an
/// empty string if the source parses cleanly. Does not touch the session.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_ghostsinthelab_app_hypercatalog_NativeBridge_nativeCheckScript(
    mut env: JNIEnv,
    _class: JClass,
    src: JString,
) -> jstring {
    let src = rust_string(&mut env, &src);
    java_string(&mut env, &Session::check_script(&src).unwrap_or_default())
}

/// Create a new "button" or "field" on the current card. Returns the new object id, or -1.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_ghostsinthelab_app_hypercatalog_NativeBridge_nativeAddObject(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    kind: JString,
) -> jint {
    let kind = rust_string(&mut env, &kind);
    let Some(s) = (unsafe { session(handle) }) else {
        return -1;
    };
    s.add_object(&kind).map(|id| id as jint).unwrap_or(-1)
}

/// Delete an object by id. Returns true if one was removed.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_ghostsinthelab_app_hypercatalog_NativeBridge_nativeDeleteObject(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    object_id: jint,
) -> jboolean {
    let Some(s) = (unsafe { session(handle) }) else {
        return JNI_FALSE;
    };
    if s.delete_object(object_id as u32) {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

/// Move/resize an object by id (drag commit). Returns true if one was updated.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_ghostsinthelab_app_hypercatalog_NativeBridge_nativeSetObjectRect(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    object_id: jint,
    x: jfloat,
    y: jfloat,
    w: jfloat,
    h: jfloat,
) -> jboolean {
    let Some(s) = (unsafe { session(handle) }) else {
        return JNI_FALSE;
    };
    if s.set_object_rect(object_id as u32, x, y, w, h) {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

/// Read an object's editable properties as JSON. Empty string if the object doesn't exist.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_ghostsinthelab_app_hypercatalog_NativeBridge_nativeGetObjectProps(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    object_id: jint,
) -> jstring {
    let Some(s) = (unsafe { session(handle) }) else {
        return java_string(&mut env, "");
    };
    java_string(
        &mut env,
        &s.get_object_props(object_id as u32).unwrap_or_default(),
    )
}

/// Apply a JSON property blob to an object. Returns true if the object was found.
#[unsafe(no_mangle)]
pub extern "system" fn Java_org_ghostsinthelab_app_hypercatalog_NativeBridge_nativeSetObjectProps(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    object_id: jint,
    props: JString,
) -> jboolean {
    let props = rust_string(&mut env, &props);
    let Some(s) = (unsafe { session(handle) }) else {
        return JNI_FALSE;
    };
    if s.set_object_props(object_id as u32, &props) {
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
