package org.ghostsinthelab.app.hypercatalog

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Instrumented tests for [StackPrefs] against a real Preferences DataStore on a device — the
 * suspend/Flow store can't run in a JVM unit test. Covers the ADR-0013 session view state:
 * per-stack card index (incl. the switch-and-return scenario) and the last-used stack key.
 *
 * Card-index assertions use synthetic stack keys (`itest-*`) that never match a real stack, so
 * the app's saved positions are left undisturbed; the `last_stack` test captures and restores the
 * real value, since there's a single per-process `session` store.
 */
@RunWith(AndroidJUnit4::class)
class StackPrefsInstrumentedTest {

    private val prefs =
        StackPrefs(InstrumentationRegistry.getInstrumentation().targetContext)

    @Test
    fun card_index_is_namespaced_and_independent_per_stack() = runBlocking {
        prefs.setCardIndex("itest-alpha", 4)
        prefs.setCardIndex("itest-beta", 1)
        assertEquals(4, prefs.cardIndex("itest-alpha"))
        assertEquals(1, prefs.cardIndex("itest-beta"))

        // The switch-and-return case (navigate A, switch to B, come back to A): updating one
        // stack's index must not disturb another's.
        prefs.setCardIndex("itest-alpha", 7)
        assertEquals(7, prefs.cardIndex("itest-alpha"))
        assertEquals(1, prefs.cardIndex("itest-beta"))
    }

    @Test
    fun unset_card_index_defaults_to_zero() = runBlocking {
        assertEquals(0, prefs.cardIndex("itest-never-written"))
    }

    @Test
    fun last_stack_round_trips() = runBlocking {
        val original = prefs.lastStack()
        try {
            prefs.setLastStack("itest-alpha")
            assertEquals("itest-alpha", prefs.lastStack())
        } finally {
            // Leave a real, loadable key behind (the app falls back to the default anyway).
            prefs.setLastStack(original ?: "productivity")
        }
    }
}
