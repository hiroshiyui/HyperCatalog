package org.ghostsinthelab.app.hypercatalog

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import uniffi.hyperffi.DispatchResult
import uniffi.hyperffi.HyperStack
import uniffi.hyperffi.ViewNode

/**
 * Native (Material 3) render target for the current card (ADR-0008) — the additive alternative to
 * the Canvas [CardView]. It consumes the core's semantic [uniffi.hyperffi.ViewTree] and realizes
 * each node as a real Compose widget; reconciliation is free via recomposition keyed by node id.
 *
 * Events are **id-addressed**: a widget calls [HyperStack.dispatch], re-entering the *same* message
 * path (object → card → background → stack) a Canvas tap would. Script side effects surface through
 * [onEffects], reusing the host's existing effect handling. The classic [CardView] is untouched.
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

    Column(
        Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(12.dp),
    ) {
        for (id in tree.rootIds) {
            val node = tree.nodes.firstOrNull { it.id == id } ?: continue
            // Stable identity within a card; resets across navigation (cardIndex changes).
            key(tree.cardIndex, node.id) {
                ViewNodeView(node, stack, ::handle)
            }
        }
    }
}

@Composable
private fun ViewNodeView(
    node: ViewNode,
    stack: HyperStack,
    onResult: (DispatchResult) -> Unit,
) {
    fun prop(k: String): String = node.props.firstOrNull { it.key == k }?.value ?: ""
    if (prop("visible") == "false") return

    when (node.kind) {
        "button" -> {
            val label = prop("title")
            val click = { onResult(stack.dispatch(node.id, "mouseUp", emptyList())) }
            // Abstract style value → Material widget; unknown styles fall back to outlined.
            when (prop("style")) {
                "rectangle" -> Button(onClick = click, modifier = Modifier.fillMaxWidth()) { Text(label) }
                "transparent" -> TextButton(onClick = click, modifier = Modifier.fillMaxWidth()) { Text(label) }
                else -> OutlinedButton(onClick = click, modifier = Modifier.fillMaxWidth()) { Text(label) }
            }
        }

        "field" -> {
            val locked = prop("locked") == "true"
            // Locked fields are script-driven: bind directly to the (freshly fetched) tree value
            // so a mutation shows on the next rev. Unlocked fields hold local edit state.
            var draft by remember(node.id) { mutableStateOf(prop("text")) }
            OutlinedTextField(
                value = if (locked) prop("text") else draft,
                onValueChange = {
                    draft = it
                    stack.setFieldText(node.id, it)
                },
                readOnly = locked,
                modifier = Modifier.fillMaxWidth(),
            )
        }

        else -> {
            // Unknown kind → graceful no-op (degrade, never crash — ADR-0008 guardrail).
        }
    }
}
