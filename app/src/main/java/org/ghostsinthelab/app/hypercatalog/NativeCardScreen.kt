package org.ghostsinthelab.app.hypercatalog

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import uniffi.hyperffi.DispatchResult
import uniffi.hyperffi.HyperStack
import uniffi.hyperffi.ViewNode
import uniffi.hyperffi.ViewTree

/**
 * Native (Material 3) render target for the current card (ADR-0008/0014) — the additive
 * alternative to the Canvas [CardView]. Consumes the core's semantic [ViewTree] and realizes each
 * node as a real Compose widget; **group** nodes become nested `Column`/`Row` containers so a
 * card's layout overlay reflows into a real grid. Reconciliation is free via recomposition keyed
 * by node id.
 *
 * Events are **id-addressed**: a widget calls [HyperStack.dispatch], re-entering the *same* message
 * path a Canvas tap would. Side effects surface through [onEffects]. The classic [CardView] is
 * untouched.
 */
@Composable
fun NativeCardScreen(
    stack: HyperStack,
    onEffects: (List<HostEffect>, String?) -> Unit,
) {
    // Bumped after a dispatch so the tree is re-fetched and the card re-renders.
    var rev by remember { mutableIntStateOf(0) }
    val tree = remember(rev) { stack.renderViewTree() }

    fun surface(result: DispatchResult) {
        val effects = hostEffectsOf(result.hostCmds)
        if (effects.isNotEmpty() || result.error != null) onEffects(effects, result.error)
    }

    fun handle(result: DispatchResult) {
        surface(result)
        if (result.cardChanged) {
            // Run the new card's openCard and surface its effects too (parity with CardView).
            surface(stack.openCard())
        }
        rev++
    }

    // The root container: scrollable so tall cards don't clip. Nested groups don't scroll.
    Container(
        mode = tree.layout,
        padding = tree.padding,
        childIds = tree.rootIds,
        tree = tree,
        stack = stack,
        onResult = ::handle,
        modifier = Modifier.fillMaxSize(),
        scroll = true,
    )
}

/** A `column`/`row` container (root or a group node) that lays out [childIds] of [tree]. */
@Composable
private fun Container(
    mode: String,
    padding: Float,
    childIds: List<Int>,
    tree: ViewTree,
    stack: HyperStack,
    onResult: (DispatchResult) -> Unit,
    modifier: Modifier = Modifier,
    scroll: Boolean = false,
) {
    val base = modifier.padding(padding.dp)
    if (mode == "row") {
        Row(base) {
            for (cid in childIds) {
                val node = tree.nodes.firstOrNull { it.id == cid } ?: continue
                val w = node.weight()
                // Horizontal flex: weighted cells share width; unweighted wrap to content.
                val cell = if (w > 0f) Modifier.weight(w) else Modifier
                key(tree.cardIndex, cid) { RenderNode(node, tree, stack, onResult, cell) }
            }
        }
    } else {
        Column(if (scroll) base.verticalScroll(rememberScrollState()) else base) {
            for (cid in childIds) {
                val node = tree.nodes.firstOrNull { it.id == cid } ?: continue
                val w = node.weight()
                // Vertical weight can't coexist with verticalScroll; in a scrollable column always
                // fill width at natural height. In a fixed column, honor a vertical weight.
                val cell = if (!scroll && w > 0f) Modifier.weight(w) else Modifier.fillMaxWidth()
                key(tree.cardIndex, cid) { RenderNode(node, tree, stack, onResult, cell) }
            }
        }
    }
}

/** Render one node into the [modifier] its parent container computed for it (carries weight/size). */
@Composable
private fun RenderNode(
    node: ViewNode,
    tree: ViewTree,
    stack: HyperStack,
    onResult: (DispatchResult) -> Unit,
    modifier: Modifier,
) {
    if (node.prop("visible") == "false") return

    when (node.kind) {
        "group" -> Container(
            mode = node.prop("mode"),
            padding = node.prop("padding").toFloatOrNull() ?: 0f,
            childIds = node.childIds,
            tree = tree,
            stack = stack,
            onResult = onResult,
            modifier = modifier,
        )

        "button" -> {
            val label = node.prop("title")
            val click = { onResult(stack.dispatch(node.id, "mouseUp", emptyList())) }
            // Abstract style value → Material widget; unknown styles fall back to outlined.
            when (node.prop("style")) {
                "rectangle" -> Button(onClick = click, modifier = modifier) { Text(label) }
                "transparent" -> TextButton(onClick = click, modifier = modifier) { Text(label) }
                else -> OutlinedButton(onClick = click, modifier = modifier) { Text(label) }
            }
        }

        "switch" -> {
            // A switch toggles in the core (auto-toggle before mouseUp); dispatch and re-read.
            val checked = node.prop("checked") == "true"
            Row(modifier, verticalAlignment = Alignment.CenterVertically) {
                Text(node.prop("title"), modifier = Modifier.weight(1f))
                Switch(
                    checked = checked,
                    onCheckedChange = { onResult(stack.dispatch(node.id, "mouseUp", emptyList())) },
                )
            }
        }

        "field" -> {
            val locked = node.prop("locked") == "true"
            // Locked fields are script-driven: bind to the freshly-fetched tree value so a mutation
            // shows on the next rev. Unlocked fields hold local edit state.
            var draft by remember(node.id) { mutableStateOf(node.prop("text")) }
            OutlinedTextField(
                value = if (locked) node.prop("text") else draft,
                onValueChange = {
                    draft = it
                    stack.setFieldText(node.id, it)
                },
                readOnly = locked,
                modifier = modifier,
            )
        }

        else -> {
            // Unknown kind → graceful no-op (degrade, never crash — ADR-0008 guardrail).
        }
    }
}

private fun ViewNode.prop(key: String): String = props.firstOrNull { it.key == key }?.value ?: ""

private fun ViewNode.weight(): Float = prop("weight").toFloatOrNull() ?: 0f
