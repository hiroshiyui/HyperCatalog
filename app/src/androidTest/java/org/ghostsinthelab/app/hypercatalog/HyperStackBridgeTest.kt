package org.ghostsinthelab.app.hypercatalog

import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Instrumented tests for the real UniFFI [uniffi.hyperffi.HyperStack] bridge — these load the
 * native `.so` (via JNA) on a device, which a JVM unit test can't. They exercise the card-index
 * methods added for ADR-0013 persistence (`openCardAt`/`currentCardIndex`) end to end across the
 * boundary, complementing the Rust-side `goto_card` test.
 */
@RunWith(AndroidJUnit4::class)
class HyperStackBridgeTest {

    /** Minimal three-card stack: `Card` only requires `id`+`name`, the rest serde-default. */
    private val yaml = """
        name: ITest
        width: 100
        height: 100
        cards:
          - id: 1
            name: One
          - id: 2
            name: Two
          - id: 3
            name: Three
    """.trimIndent()

    private inline fun withStack(src: String, body: (uniffi.hyperffi.HyperStack) -> Unit) {
        val s = uniffi.hyperffi.HyperStack.loadYaml(src)
        try {
            body(s)
        } finally {
            s.destroy()
        }
    }

    @Test
    fun open_card_at_round_trips_index_across_the_bridge() = withStack(yaml) { s ->
        assertEquals(0, s.currentCardIndex())
        s.openCardAt(2)
        assertEquals(2, s.currentCardIndex())
        assertEquals("Three", s.renderCurrentCard().cardName)
    }

    @Test
    fun open_card_at_clamps_out_of_range() = withStack(yaml) { s ->
        s.openCardAt(99) // past the end → last card (0-based), not a crash
        assertEquals(2, s.currentCardIndex())
        s.openCardAt(-5) // negative → first card
        assertEquals(0, s.currentCardIndex())
    }

    @Test
    fun yaml_round_trips_through_the_bridge() = withStack(yaml) { s ->
        s.openCardAt(1)
        withStack(s.toYaml()) { reloaded ->
            // Card index is view state, not document content (ADR-0013): a reloaded document
            // starts at its first card regardless of where the saved session was.
            assertEquals(3, reloaded.renderCurrentCard().cardCount)
            assertEquals(0, reloaded.currentCardIndex())
        }
    }

    /** dispatchLifecycle (ADR-0019) runs a stack-level handler and reports `handled`; set_insets
     *  (ADR-0020) is readable from script. Covers the two host-driven bridge methods over the .so. */
    @Test
    fun lifecycle_and_insets_cross_the_bridge() {
        val src = """
            name: LC
            script: "on resume\n  put the safeTop of this card into field \"out\"\nend resume"
            cards:
              - id: 1
                name: One
                fields:
                  - { id: 10, name: out, rect: { x: 0, y: 0, w: 10, h: 10 }, text: "" }
        """.trimIndent()
        val s = uniffi.hyperffi.HyperStack.loadYaml(src)
        try {
            s.setInsets(24f, 0f, 0f, 0f)
            val r = s.dispatchLifecycle("resume", emptyList())
            assertEquals(true, r.handled)
            // The `on resume` handler read the inset and wrote it into the field.
            val out = s.objectProps(10)!!
            assertEquals("24", out.text)
            // No handler for this one → not handled.
            assertEquals(false, s.dispatchLifecycle("backPressed", emptyList()).handled)
        } finally {
            s.destroy()
        }
    }

    /** dispatchMessage (ADR-0024) injects a top-level message with typed string args over the .so —
     *  the host→core re-entrant delivery point (e.g. a Phase-10 `get url` completion). The handler
     *  binds the arg by position. */
    @Test
    fun dispatch_message_with_args_crosses_the_bridge() {
        val src = """
            name: AM
            cards:
              - id: 1
                name: One
                script: "on responseReceived data\n  put data into field \"out\"\nend responseReceived"
                fields:
                  - { id: 10, name: out, rect: { x: 0, y: 0, w: 10, h: 10 }, text: "" }
        """.trimIndent()
        val s = uniffi.hyperffi.HyperStack.loadYaml(src)
        try {
            val r = s.dispatchMessage("responseReceived", listOf("payload"))
            assertEquals(true, r.handled)
            assertEquals("payload", s.objectProps(10)!!.text)
        } finally {
            s.destroy()
        }
    }
}
