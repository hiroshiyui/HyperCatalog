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

    /** Topmost object id at a card-space point (for edit-mode selection); -1 if none. */
    external fun nativeObjectAt(handle: Long, x: Float, y: Float): Int

    /** Read an object's HyperTalk source by id. Empty string if the object doesn't exist. */
    external fun nativeGetObjectScript(handle: Long, objectId: Int): String

    /** Write an object's HyperTalk source by id. Returns true if an object changed. */
    external fun nativeSetObjectScript(handle: Long, objectId: Int, src: String): Boolean

    /** Validate HyperTalk source. Returns the parser error, or empty string if it parses. */
    external fun nativeCheckScript(src: String): String

    /** Serialize the current stack to JSON (for saving). */
    external fun nativeToJson(handle: Long): String

    /** Release a handle. Safe to call once per handle. */
    external fun nativeFree(handle: Long)
}
