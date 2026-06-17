package org.ghostsinthelab.app.hypercatalog

import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Instrumented test for the native (Compose) render target (ADR-0008). Drives [NativeCardScreen]
 * with the real UniFFI bridge (`.so` via JNA) and Compose, proving the contract end-to-end:
 * a button node tapped by id → `dispatch(id, "mouseUp")` → same handler a Canvas tap runs →
 * tree re-fetch → the field node reflects the mutation. The Compose analogue of
 * [HyperStackBridgeTest].
 */
@RunWith(AndroidJUnit4::class)
class NativeRenderInstrumentedTest {

    @get:Rule
    val compose = createComposeRule()

    private val yaml = """
        name: NativeITest
        width: 100
        height: 100
        cards:
          - id: 1
            name: One
            fields:
              - { id: 10, name: out, rect: { x: 0, y: 0, w: 100, h: 20 }, text: "before", locked: true }
            buttons:
              - { id: 20, name: Go, rect: { x: 0, y: 30, w: 100, h: 20 }, title: "Go",
                  script: "on mouseUp\n  put \"after\" into field \"out\"\nend mouseUp" }
    """.trimIndent()

    @Test
    fun tapping_a_button_node_updates_a_field_node() {
        val stack = uniffi.hyperffi.HyperStack.loadYaml(yaml)
        try {
            compose.setContent { MaterialTheme { NativeCardScreen(stack) { _, _ -> } } }

            // Locked output field starts at "before"; the button writes "after".
            compose.onNodeWithText("before").assertIsDisplayed()
            compose.onNodeWithText("Go").performClick()
            compose.onNodeWithText("after").assertIsDisplayed()
        } finally {
            stack.destroy()
        }
    }

    /** A card with a layout overlay (ADR-0014): a row group nests a field + button; the button
     *  still dispatches by id through the nesting. */
    private val groupedYaml = """
        name: GroupedITest
        width: 100
        height: 100
        cards:
          - id: 1
            name: One
            fields:
              - { id: 10, name: out, rect: { x: 0, y: 0, w: 50, h: 20 }, text: "before", locked: true, weight: 2 }
            buttons:
              - { id: 20, name: Go, rect: { x: 0, y: 0, w: 50, h: 20 }, title: "Go", weight: 1,
                  script: "on mouseUp\n  put \"after\" into field \"out\"\nend mouseUp" }
            layout:
              mode: column
              padding: 8
              children:
                - { mode: row, padding: 4, children: [10, 20] }
    """.trimIndent()

    @Test
    fun grouped_layout_renders_nested() {
        val stack = uniffi.hyperffi.HyperStack.loadYaml(groupedYaml)
        try {
            compose.setContent { MaterialTheme { NativeCardScreen(stack) { _, _ -> } } }

            // Both nested cells render, and a button inside the row group still dispatches by id.
            compose.onNodeWithText("before").assertIsDisplayed()
            compose.onNodeWithText("Go").performClick()
            compose.onNodeWithText("after").assertIsDisplayed()
        } finally {
            stack.destroy()
        }
    }
}
