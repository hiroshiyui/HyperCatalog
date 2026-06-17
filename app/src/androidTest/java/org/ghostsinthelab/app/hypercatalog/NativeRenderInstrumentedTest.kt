package org.ghostsinthelab.app.hypercatalog

import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.isToggleable
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

    /** A switch (ADR-0015): a button with `checked`. Tapping it auto-toggles in the core and the
     *  handler reads the new state. */
    private val switchYaml = """
        name: SwitchITest
        width: 100
        height: 100
        cards:
          - id: 1
            name: One
            fields:
              - { id: 10, name: out, rect: { x: 0, y: 0, w: 100, h: 20 }, text: "off", locked: true }
            buttons:
              - { id: 20, name: wifi, rect: { x: 0, y: 30, w: 100, h: 20 }, title: "Wi-Fi", checked: false,
                  script: "on mouseUp\n  if the checked of me then put \"on\" into field \"out\" else put \"off\" into field \"out\"\nend mouseUp" }
    """.trimIndent()

    @Test
    fun tapping_a_switch_toggles_and_runs_its_handler() {
        val stack = uniffi.hyperffi.HyperStack.loadYaml(switchYaml)
        try {
            compose.setContent { MaterialTheme { NativeCardScreen(stack) { _, _ -> } } }
            compose.onNodeWithText("off").assertIsDisplayed()
            compose.onNode(isToggleable()).performClick() // the Material Switch
            compose.onNodeWithText("on").assertIsDisplayed()
        } finally {
            stack.destroy()
        }
    }

    /** Material roles + theme (ADR-0018): role-mapped buttons render under a seeded theme. */
    private val themedYaml = """
        name: ThemedITest
        accent_color: "#6750A4"
        theme: dark
        cards:
          - id: 1
            name: One
            buttons:
              - { id: 20, name: a, rect: { x: 0, y: 0, w: 10, h: 10 }, title: "Filled", role: filled }
              - { id: 21, name: b, rect: { x: 0, y: 0, w: 10, h: 10 }, title: "Tonal", role: tonal }
              - { id: 22, name: c, rect: { x: 0, y: 0, w: 10, h: 10 }, title: "Fab", role: fab,
                  script: "on mouseUp\n  set the name of this stack to \"tapped\"\nend mouseUp" }
            layout: { mode: column, children: [20, 21, 22] }
    """.trimIndent()

    @Test
    fun roles_render_and_dispatch_under_theme() {
        val stack = uniffi.hyperffi.HyperStack.loadYaml(themedYaml)
        try {
            compose.setContent { MaterialTheme { NativeCardScreen(stack) { _, _ -> } } }
            compose.onNodeWithText("Filled").assertIsDisplayed()
            compose.onNodeWithText("Tonal").assertIsDisplayed()
            compose.onNodeWithText("Fab").assertIsDisplayed().performClick()
        } finally {
            stack.destroy()
        }
    }

    /** A `free` (absolute) layout (ADR-0017): objects placed by rect; dispatch still works. */
    private val freeYaml = """
        name: FreeITest
        width: 200
        height: 200
        cards:
          - id: 1
            name: One
            fields:
              - { id: 10, name: out, rect: { x: 10, y: 10, w: 180, h: 30 }, text: "x", locked: true }
            buttons:
              - { id: 20, name: a, rect: { x: 10, y: 60, w: 180, h: 30 }, title: "Go",
                  script: "on mouseUp\n  put \"done\" into field \"out\"\nend mouseUp" }
            layout:
              mode: free
              children: [10, 20]
    """.trimIndent()

    @Test
    fun free_layout_renders_and_dispatches() {
        val stack = uniffi.hyperffi.HyperStack.loadYaml(freeYaml)
        try {
            compose.setContent { MaterialTheme { NativeCardScreen(stack) { _, _ -> } } }
            compose.onNodeWithText("x").assertIsDisplayed()
            compose.onNodeWithText("Go").performClick()
            compose.onNodeWithText("done").assertIsDisplayed()
        } finally {
            stack.destroy()
        }
    }

    /** A grid layout (ADR-0016): 4 buttons in a 2-column grid; each still dispatches by id. */
    private val gridYaml = """
        name: GridITest
        width: 100
        height: 100
        cards:
          - id: 1
            name: One
            fields:
              - { id: 10, name: out, rect: { x: 0, y: 0, w: 100, h: 20 }, text: "x", locked: true }
            buttons:
              - { id: 20, name: a, rect: { x: 0, y: 0, w: 10, h: 10 }, title: "A",
                  script: "on mouseUp\n  put \"hit-A\" into field \"out\"\nend mouseUp" }
              - { id: 21, name: b, rect: { x: 0, y: 0, w: 10, h: 10 }, title: "B" }
              - { id: 22, name: c, rect: { x: 0, y: 0, w: 10, h: 10 }, title: "C" }
              - { id: 23, name: d, rect: { x: 0, y: 0, w: 10, h: 10 }, title: "D" }
            layout:
              mode: grid
              columns: 2
              children: [10, 20, 21, 22, 23]
    """.trimIndent()

    @Test
    fun grid_layout_renders_and_dispatches() {
        val stack = uniffi.hyperffi.HyperStack.loadYaml(gridYaml)
        try {
            compose.setContent { MaterialTheme { NativeCardScreen(stack) { _, _ -> } } }
            compose.onNodeWithText("A").assertIsDisplayed()
            compose.onNodeWithText("D").assertIsDisplayed()
            compose.onNodeWithText("A").performClick()
            compose.onNodeWithText("hit-A").assertIsDisplayed()
        } finally {
            stack.destroy()
        }
    }

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
