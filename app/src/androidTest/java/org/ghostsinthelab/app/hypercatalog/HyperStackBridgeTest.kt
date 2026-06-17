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
}
