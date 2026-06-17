package org.ghostsinthelab.app.hypercatalog

import java.io.File
import kotlin.math.abs

/**
 * Framework-free host helpers, extracted from [CardView]/[MainActivity] so the logic is
 * unit-testable on the JVM — no Android types, no UniFFI bridge. See `HostLogicTest`.
 */

/**
 * Classify a fling delta (`dx`, `dy`, in view px) into a swipe message, or `null` if it travelled
 * less than [minTravel] on both axes. The dominant axis wins; an exact tie goes vertical (matching
 * the original `flingGesture`).
 */
fun swipeDirection(dx: Float, dy: Float, minTravel: Float): String? {
    if (abs(dx) < minTravel && abs(dy) < minTravel) return null
    return if (abs(dx) > abs(dy)) {
        if (dx > 0) "swipeRight" else "swipeLeft"
    } else {
        if (dy > 0) "swipeDown" else "swipeUp"
    }
}

/**
 * Best-effort display name for a stack file's [content] — the first top-level `name:` (YAML) or
 * `"name":` (pretty JSON) value — falling back to [fallback]. Pure (no `org.json`): the stack name
 * is the first such line in the file, before any nested object.
 */
fun stackNameFrom(content: String, fallback: String): String {
    val m = Regex("""(?m)^\s*"?name"?\s*:\s*(.+?)\s*$""").find(content) ?: return fallback
    val raw = m.groupValues[1].trim().removeSuffix(",").trim().trim('"', '\'')
    return raw.ifEmpty { fallback }
}

/**
 * Atomically replace [target]'s contents with [text]. Writes a sibling `<name>.tmp` then renames
 * it over the target — `rename(2)` is atomic on the same filesystem, so a reader (or a crash
 * mid-write) sees either the old complete file or the new one, never a truncated half. Plain
 * `writeText` truncates-then-writes and would leave a corrupt stack if the process died mid-save.
 * The fallback covers filesystems whose `rename` won't clobber an existing target.
 */
fun writeFileAtomically(target: File, text: String) {
    target.parentFile?.mkdirs()
    val tmp = File(target.parentFile, "${target.name}.tmp")
    tmp.writeText(text)
    if (tmp.renameTo(target)) return
    // POSIX rename clobbers atomically; some filesystems require the target gone first.
    target.delete()
    if (!tmp.renameTo(target)) {
        tmp.copyTo(target, overwrite = true)
        tmp.delete()
    }
}

/**
 * DataStore preference key for a stack's last-viewed card index. Namespaced per stack key so each
 * stack restores independently (ADR-0013). Pure (kept here so it's covered by `HostLogicTest`).
 */
fun cardIndexPrefKey(stackKey: String): String = "card_index/$stackKey"
