package org.ghostsinthelab.app.hypercatalog

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

/** JVM unit tests for the framework-free host helpers extracted from CardView/MainActivity. */
class HostLogicTest {

    @get:Rule
    val tmp = TemporaryFolder()

    // --- swipeDirection ---

    @Test
    fun swipe_classifies_by_dominant_axis() {
        assertEquals("swipeLeft", swipeDirection(-100f, 10f, 48f))
        assertEquals("swipeRight", swipeDirection(100f, -10f, 48f))
        assertEquals("swipeUp", swipeDirection(10f, -100f, 48f))
        assertEquals("swipeDown", swipeDirection(-10f, 100f, 48f))
    }

    @Test
    fun swipe_is_null_below_threshold_on_both_axes() {
        assertNull(swipeDirection(20f, 20f, 48f))
        assertNull(swipeDirection(0f, 0f, 48f))
        assertNull(swipeDirection(47f, -47f, 48f))
    }

    @Test
    fun swipe_dominant_axis_wins_when_both_exceed_threshold() {
        assertEquals("swipeRight", swipeDirection(100f, 60f, 48f)) // |dx| > |dy|
        assertEquals("swipeDown", swipeDirection(60f, 100f, 48f)) // |dy| > |dx|
        assertEquals("swipeUp", swipeDirection(60f, -100f, 48f))
        // exact tie goes vertical (matches the original flingGesture)
        assertEquals("swipeDown", swipeDirection(60f, 60f, 48f))
    }

    // --- stackNameFrom ---

    @Test
    fun stack_name_from_yaml() {
        assertEquals("Gesture Demo", stackNameFrom("# a comment\nname: Gesture Demo\nwidth: 360", "key"))
        assertEquals("Welcome", stackNameFrom("name: \"Welcome\"\ncards: []\n", "key"))
        assertEquals("Productivity", stackNameFrom("name: Productivity\n", "key"))
    }

    @Test
    fun stack_name_from_pretty_json() {
        val json = "{\n  \"name\": \"Welcome\",\n  \"width\": 360\n}"
        assertEquals("Welcome", stackNameFrom(json, "key"))
    }

    @Test
    fun stack_name_falls_back_to_key() {
        assertEquals("mykey", stackNameFrom("cards: []\nwidth: 360", "mykey"))
        assertEquals("mykey", stackNameFrom("", "mykey"))
        assertEquals("mykey", stackNameFrom("name:   \n", "mykey")) // empty value
    }

    // --- writeFileAtomically ---

    @Test
    fun atomic_write_creates_then_overwrites() {
        val target = File(tmp.root, "stack.yaml")
        writeFileAtomically(target, "name: First\n")
        assertEquals("name: First\n", target.readText())
        writeFileAtomically(target, "name: Second\n")
        assertEquals("name: Second\n", target.readText())
    }

    @Test
    fun atomic_write_leaves_no_temp_file_behind() {
        val target = File(tmp.root, "stack.yaml")
        writeFileAtomically(target, "x")
        assertFalse("temp sibling must be gone", File(tmp.root, "stack.yaml.tmp").exists())
        assertEquals(listOf("stack.yaml"), tmp.root.list()!!.toList())
    }

    @Test
    fun atomic_write_creates_parent_dirs() {
        val target = File(tmp.root, "nested/dir/stack.yaml")
        writeFileAtomically(target, "ok")
        assertTrue(target.exists())
        assertEquals("ok", target.readText())
    }

    // --- cardIndexPrefKey ---

    @Test
    fun card_index_pref_key_is_namespaced_per_stack() {
        assertEquals("card_index/productivity", cardIndexPrefKey("productivity"))
        assertEquals("card_index/gestures", cardIndexPrefKey("gestures"))
    }
}
