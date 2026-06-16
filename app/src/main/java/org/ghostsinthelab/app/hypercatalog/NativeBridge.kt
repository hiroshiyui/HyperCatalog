package org.ghostsinthelab.app.hypercatalog

/**
 * Kotlin-facing bridge to the Rust core (`libhyperffi.so`). A loaded stack is represented
 * by an opaque `handle` (a native `Box<Session>` pointer). Structured data crosses the
 * boundary as JSON strings; see the Rust `session` module for the schemas.
 */
object NativeBridge {
    init {
        System.loadLibrary("hyperffi")
    }

    /** Load a stack from JSON. Returns a handle, or 0 on error. */
    external fun nativeLoad(json: String): Long

    /** Fire the current card's `openCard` handler. Returns a DispatchResult JSON. */
    external fun nativeOpenCard(handle: Long): String

    /** Render the current card. Returns a RenderList JSON. */
    external fun nativeRender(handle: Long): String

    /** Dispatch a touch. `phase` is "up" for a completed tap. Returns DispatchResult JSON. */
    external fun nativeDispatchTouch(handle: Long, x: Float, y: Float, phase: String): String

    /** Set a field's text by id (host-edited). Returns true if a field changed. */
    external fun nativeSetFieldText(handle: Long, fieldId: Int, text: String): Boolean

    /** Serialize the current stack to JSON (for saving). */
    external fun nativeToJson(handle: Long): String

    /** Release a handle. Safe to call once per handle. */
    external fun nativeFree(handle: Long)
}
