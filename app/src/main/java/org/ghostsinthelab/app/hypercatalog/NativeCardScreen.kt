package org.ghostsinthelab.app.hypercatalog

import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import android.graphics.BitmapFactory
import android.os.Build
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.material3.AssistChip
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.ElevatedButton
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Slider
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
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

    // Stack-level Material theme + seed color (ADR-0018) wrap the whole card.
    MaterialTheme(colorScheme = schemeFor(tree.theme, tree.accentColor)) {
        // The root container: scrollable so tall cards don't clip. Nested groups don't scroll.
        Container(
            mode = tree.layout,
            padding = tree.padding,
            childIds = tree.rootIds,
            tree = tree,
            stack = stack,
            onResult = ::handle,
            // Top inset clears the floating Classic/Edit toggle row; small side padding for breathing room.
            modifier = Modifier.fillMaxSize().padding(top = 52.dp, start = 4.dp, end = 4.dp),
            scroll = true,
            columns = tree.columns,
        )
    }
}

/** Build a Material 3 [ColorScheme] from the stack's `theme` (light/dark/system/dynamic) and an
 *  optional seed `accent` color (hex). `dynamic` uses Material You on Android 12+, else the seed. */
@Composable
private fun schemeFor(theme: String, accent: String): ColorScheme {
    val context = LocalContext.current
    val dark = when (theme.lowercase()) {
        "dark" -> true
        "light", "" -> false
        else -> androidx.compose.foundation.isSystemInDarkTheme() // "system" / "dynamic"
    }
    if (theme.equals("dynamic", ignoreCase = true) && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
        return if (dark) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)
    }
    val base = if (dark) darkColorScheme() else lightColorScheme()
    val seed = runCatching { android.graphics.Color.parseColor(accent) }.getOrNull() ?: return base
    return seededScheme(base, seed, dark)
}

/** Derive a cohesive [ColorScheme] from a single seed color (ADR-0022): primary from the seed,
 *  with secondary/tertiary/containers rotated/desaturated from its hue. A lightweight stand-in for
 *  a full Material tonal palette (which needs material-color-utilities); surfaces stay neutral. */
private fun seededScheme(base: ColorScheme, seed: Int, dark: Boolean): ColorScheme {
    val hsv = FloatArray(3).also { android.graphics.Color.colorToHSV(seed, it) }
    fun c(hue: Float, sat: Float, value: Float) = Color(
        android.graphics.Color.HSVToColor(
            floatArrayOf(((hue % 360f) + 360f) % 360f, sat.coerceIn(0f, 1f), value.coerceIn(0f, 1f)),
        ),
    )
    return base.copy(
        primary = Color(seed),
        onPrimary = if (hsv[2] > 0.6f) Color.Black else Color.White,
        primaryContainer = c(hsv[0], hsv[1] * 0.35f, if (dark) 0.30f else 0.90f),
        secondary = c(hsv[0], hsv[1] * 0.55f, hsv[2]),
        tertiary = c(hsv[0] + 60f, hsv[1], hsv[2]),
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
    columns: Int = 0,
) {
    val base = modifier.padding(padding.dp)
    when (mode) {
        "row" -> Row(
            base,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            for (cid in childIds) {
                val node = tree.nodes.firstOrNull { it.id == cid } ?: continue
                val w = node.weight()
                // Horizontal flex: weighted cells share width; unweighted wrap to content.
                val cell = if (w > 0f) Modifier.weight(w) else Modifier
                key(tree.cardIndex, cid) { RenderNode(node, tree, stack, onResult, cell) }
            }
        }

        "free" -> {
            // Absolute placement (ADR-0017): scale card units → dp to fit, place each by its rect.
            BoxWithConstraints(base.fillMaxSize()) {
                val cardW = tree.width.coerceAtLeast(1f)
                val cardH = tree.height.coerceAtLeast(1f)
                val scale = minOf(maxWidth.value / cardW, maxHeight.value / cardH)
                for (cid in childIds) {
                    val node = tree.nodes.firstOrNull { it.id == cid } ?: continue
                    fun px(k: String) = (node.prop(k).toFloatOrNull() ?: 0f) * scale
                    key(tree.cardIndex, cid) {
                        RenderNode(
                            node, tree, stack, onResult,
                            Modifier.offset(px("x").dp, px("y").dp).size(px("w").dp, px("h").dp),
                        )
                    }
                }
            }
        }

        "grid" -> {
            // Chunk children into rows of `columns` equal-width cells (ADR-0016) — no LazyGrid.
            val cols = columns.coerceAtLeast(1)
            Column(
                if (scroll) base.verticalScroll(rememberScrollState()) else base,
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                childIds.chunked(cols).forEach { rowIds ->
                    Row(
                        Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        for (cid in rowIds) {
                            val node = tree.nodes.firstOrNull { it.id == cid } ?: continue
                            key(tree.cardIndex, cid) {
                                RenderNode(node, tree, stack, onResult, Modifier.weight(1f))
                            }
                        }
                        // Pad a short final row so cells keep their column width.
                        repeat(cols - rowIds.size) { Spacer(Modifier.weight(1f)) }
                    }
                }
            }
        }

        else -> Column(
            if (scroll) base.verticalScroll(rememberScrollState()) else base,
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
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

/** Render one node into the [baseModifier] its parent container computed for it (carries
 *  weight/size), augmented with the node's accessibility semantics (ADR-0022). */
@Composable
private fun RenderNode(
    node: ViewNode,
    tree: ViewTree,
    stack: HyperStack,
    onResult: (DispatchResult) -> Unit,
    baseModifier: Modifier,
) {
    if (node.prop("visible") == "false") return
    val context = LocalContext.current
    val modifier = baseModifier.then(node.semanticsModifier())

    when (node.kind) {
        "group" -> Container(
            mode = node.prop("mode"),
            padding = node.prop("padding").toFloatOrNull() ?: 0f,
            childIds = node.childIds,
            tree = tree,
            stack = stack,
            onResult = onResult,
            modifier = modifier,
            columns = node.prop("columns").toIntOrNull() ?: 0,
        )

        "button" -> {
            val click = { onResult(stack.dispatch(node.id, "mouseUp", emptyList())) }
            // Material role (ADR-0018) wins; else fall back to the abstract style → role mapping.
            val role = node.prop("role").ifEmpty { styleToRole(node.prop("style")) }
            val pad = CompactButtonPadding
            when (role) {
                "filled" -> Button(click, modifier, contentPadding = pad) { ButtonLabel(node) }
                "tonal" -> FilledTonalButton(click, modifier, contentPadding = pad) { ButtonLabel(node) }
                "elevated" -> ElevatedButton(click, modifier, contentPadding = pad) { ButtonLabel(node) }
                "text" -> TextButton(click, modifier, contentPadding = pad) { ButtonLabel(node) }
                "fab" -> ExtendedFloatingActionButton(onClick = click, modifier = modifier) { ButtonLabel(node) }
                else -> OutlinedButton(click, modifier, contentPadding = pad) { ButtonLabel(node) }
            }
        }

        // Boolean controls (ADR-0015/0021): the core auto-toggles `checked` before mouseUp, so we
        // just dispatch and re-read. Switch trails its label; checkbox/radio lead (Material idiom).
        // A boolean control's title is an *optional* label. When empty (e.g. a To-Do checkbox
        // paired with its own task field), render just the control — no weighted Text, which would
        // otherwise greedily eat the row's width and starve the sibling field.
        "switch" -> {
            val toggle = { onResult(stack.dispatch(node.id, "mouseUp", emptyList())) }
            val title = node.prop("title")
            Row(modifier, verticalAlignment = Alignment.CenterVertically) {
                if (title.isNotEmpty()) {
                    Text(title, style = node.nodeTextStyle(), modifier = Modifier.weight(1f))
                }
                Switch(checked = node.prop("checked") == "true", onCheckedChange = { toggle() })
            }
        }

        "checkbox" -> {
            val toggle = { onResult(stack.dispatch(node.id, "mouseUp", emptyList())) }
            val title = node.prop("title")
            Row(modifier, verticalAlignment = Alignment.CenterVertically) {
                Checkbox(checked = node.prop("checked") == "true", onCheckedChange = { toggle() })
                if (title.isNotEmpty()) {
                    Text(title, style = node.nodeTextStyle(), modifier = Modifier.weight(1f))
                }
            }
        }

        "radio" -> {
            val toggle = { onResult(stack.dispatch(node.id, "mouseUp", emptyList())) }
            val title = node.prop("title")
            Row(modifier, verticalAlignment = Alignment.CenterVertically) {
                RadioButton(selected = node.prop("checked") == "true", onClick = toggle)
                if (title.isNotEmpty()) {
                    Text(title, style = node.nodeTextStyle(), modifier = Modifier.weight(1f))
                }
            }
        }

        // Numeric controls (ADR-0021). Slider pushes its value to the core during the drag and
        // dispatches mouseUp on release (the field-edit commit pattern); progress is read-only.
        "slider" -> {
            var v by remember(node.id) { mutableFloatStateOf(node.prop("value").toFloatOrNull() ?: 0f) }
            Column(modifier) {
                node.prop("title").takeIf { it.isNotEmpty() }
                    ?.let { Text(it, style = node.nodeTextStyle()) }
                Slider(
                    value = v,
                    onValueChange = { v = it; stack.setValue(node.id, it) },
                    onValueChangeFinished = { onResult(stack.dispatch(node.id, "mouseUp", emptyList())) },
                )
            }
        }

        "progress" -> {
            val p = (node.prop("value").toFloatOrNull() ?: 0f).coerceIn(0f, 1f)
            Column(modifier) {
                node.prop("title").takeIf { it.isNotEmpty() }
                    ?.let { Text(it, style = node.nodeTextStyle()) }
                LinearProgressIndicator(progress = { p }, modifier = Modifier.fillMaxWidth())
            }
        }

        // Content controls (ADR-0021). Image loads a bundled asset by name (remote URLs are Phase
        // 10); chip is a Material filter/assist chip; divider is a thin rule.
        "image" -> {
            val source = node.prop("source")
            if (source.startsWith("http://") || source.startsWith("https://")) {
                // Remote source (ADR-0026): Coil loads it asynchronously off the UI thread.
                coil.compose.AsyncImage(
                    model = source,
                    contentDescription = node.prop("title"),
                    modifier = modifier,
                )
            } else {
                val bitmap = remember(source) { assetImage(context, source) }
                if (bitmap != null) {
                    Image(bitmap = bitmap, contentDescription = node.prop("title"), modifier = modifier)
                } else {
                    Text("[image: $source]", style = node.nodeTextStyle(), modifier = modifier)
                }
            }
        }

        "chip" -> {
            val click = { onResult(stack.dispatch(node.id, "mouseUp", emptyList())) }
            val checked = node.prop("checked")
            if (checked.isNotEmpty()) {
                FilterChip(selected = checked == "true", onClick = click,
                    label = { Text(node.prop("title")) }, modifier = modifier)
            } else {
                AssistChip(onClick = click, label = { Text(node.prop("title")) }, modifier = modifier)
            }
        }

        "divider" -> HorizontalDivider(modifier)

        "field" -> {
            if (node.prop("locked") == "true") {
                // A locked field is display text (label/title/readout) — render plain Text, not an
                // input box, honoring its text styling so it matches the Canvas target.
                Text(text = node.prop("text"), style = node.nodeTextStyle(), modifier = modifier)
            } else {
                // An editable field is a real text input (holds local edit state). A `label`
                // (ADR-0021) renders as the Material floating label inside the outline.
                var draft by remember(node.id) { mutableStateOf(node.prop("text")) }
                val label = node.prop("label")
                OutlinedTextField(
                    value = draft,
                    onValueChange = {
                        draft = it
                        stack.setFieldText(node.id, it)
                    },
                    label = label.takeIf { it.isNotEmpty() }?.let { { Text(it) } },
                    singleLine = true,
                    textStyle = node.nodeTextStyle(),
                    modifier = modifier,
                )
            }
        }

        else -> {
            // Unknown kind → graceful no-op (degrade, never crash — ADR-0008 guardrail).
        }
    }
}

/** Decode a bundled asset image by file name (ADR-0021 `image` control), or null if absent/bad.
 *  Local assets only for now; remote URLs are Phase 10. */
private fun assetImage(context: android.content.Context, name: String): ImageBitmap? {
    if (name.isEmpty()) return null
    return runCatching {
        context.assets.open(name).use { BitmapFactory.decodeStream(it) }?.asImageBitmap()
    }.getOrNull()
}

/** Accessibility semantics for a node (ADR-0022): a TalkBack `contentDescription` and/or a
 *  `liveRegion` (status fields announce changes). [Modifier] (no-op) when neither is set. */
private fun ViewNode.semanticsModifier(): Modifier {
    val cd = prop("contentDescription")
    val live = prop("liveRegion")
    if (cd.isEmpty() && live.isEmpty()) return Modifier
    return Modifier.semantics {
        if (cd.isNotEmpty()) contentDescription = cd
        if (live.isNotEmpty()) {
            liveRegion = if (live == "assertive") LiveRegionMode.Assertive else LiveRegionMode.Polite
        }
    }
}

private fun ViewNode.prop(key: String): String = props.firstOrNull { it.key == key }?.value ?: ""

private fun ViewNode.weight(): Float = prop("weight").toFloatOrNull() ?: 0f

/** Compact button content padding so labels fit narrow (rect-sized) buttons in `free` mode. */
private val CompactButtonPadding = PaddingValues(horizontal = 12.dp, vertical = 6.dp)

/** A button label: honors the node's text styling (size/bold/italic/underline/font) to match the
 *  Canvas target, and stays on one line (ellipsis) so a narrow rect-sized button doesn't wrap. */
@Composable
private fun ButtonLabel(node: ViewNode) {
    Text(
        text = node.prop("title"),
        style = node.nodeTextStyle(),
        maxLines = 1,
        softWrap = false,
        overflow = TextOverflow.Ellipsis,
    )
}

/** Map a field's `align` (`left`/`center`/`right`) to a Compose [TextAlign]; unset → start. */
private fun alignOf(align: String): TextAlign = when (align) {
    "center" -> TextAlign.Center
    "right" -> TextAlign.End
    else -> TextAlign.Start
}

/**
 * The [TextStyle] for a node's text (field contents or button label), so native matches the Canvas
 * target: a Material `textRole` type-scale base (or the node's `size` when no role is set), the
 * `font` family, the comma-list `textStyle` (bold/italic/underline), and `align`.
 */
@Composable
private fun ViewNode.nodeTextStyle(): TextStyle {
    val role = prop("textRole")
    var s = typographyFor(role)
    if (role.isEmpty()) {
        prop("size").toFloatOrNull()?.takeIf { it > 0f }?.let { s = s.copy(fontSize = it.sp) }
    }
    val family = when (prop("font")) {
        "sans-serif" -> FontFamily.SansSerif
        "serif" -> FontFamily.Serif
        "monospace" -> FontFamily.Monospace
        else -> s.fontFamily
    }
    val flags = prop("textStyle")
    return s.copy(
        fontFamily = family,
        fontWeight = if ("bold" in flags) FontWeight.Bold else s.fontWeight,
        fontStyle = if ("italic" in flags) FontStyle.Italic else s.fontStyle,
        textDecoration = if ("underline" in flags) TextDecoration.Underline else s.textDecoration,
        textAlign = alignOf(prop("align")),
    )
}

/** Map a legacy `ButtonStyle` to a Material role, preserving slice-1 appearance when no role is set. */
private fun styleToRole(style: String): String = when (style) {
    "rectangle" -> "filled"
    "transparent" -> "text"
    else -> "outlined" // "rounded"/unknown
}

/** Resolve a Material type-scale token (ADR-0018) to a [TextStyle], or the default if unknown. */
@Composable
private fun typographyFor(role: String): TextStyle {
    val t = MaterialTheme.typography
    return when (role) {
        "displayLarge" -> t.displayLarge
        "displayMedium" -> t.displayMedium
        "displaySmall" -> t.displaySmall
        "headlineLarge" -> t.headlineLarge
        "headlineMedium" -> t.headlineMedium
        "headlineSmall" -> t.headlineSmall
        "titleLarge" -> t.titleLarge
        "titleMedium" -> t.titleMedium
        "titleSmall" -> t.titleSmall
        "bodyLarge" -> t.bodyLarge
        "bodyMedium" -> t.bodyMedium
        "bodySmall" -> t.bodySmall
        "labelLarge" -> t.labelLarge
        "labelMedium" -> t.labelMedium
        "labelSmall" -> t.labelSmall
        else -> androidx.compose.material3.LocalTextStyle.current
    }
}
