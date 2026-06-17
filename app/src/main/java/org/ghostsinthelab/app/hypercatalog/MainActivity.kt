package org.ghostsinthelab.app.hypercatalog

import android.graphics.Color
import android.graphics.RectF
import android.media.AudioManager
import android.media.ToneGenerator
import android.os.Bundle
import android.text.InputType
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputMethodManager
import android.widget.ArrayAdapter
import android.widget.Button
import android.widget.CheckBox
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.Spinner
import android.widget.TextView
import android.widget.Toast
import androidx.activity.enableEdgeToEdge
import androidx.appcompat.app.AlertDialog
import androidx.appcompat.app.AppCompatActivity
import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.platform.ComposeView
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import kotlinx.coroutines.runBlocking
import java.io.File

/**
 * Hosts the [CardView] for the active stack. Reopens the last-used stack (or the bundled
 * default), wires script side effects to platform UI (dialogs/beeps/toasts), provides an
 * [EditText] overlay for editing fields, and saves each stack's edits to its own copy on
 * pause/switch (see [stacksDir]). Stack switching is driven from HyperTalk: `show stacks`
 * opens the picker ([showStackPicker]) and `go to stack "Name"` jumps directly — there is no
 * host chrome button for it.
 */
class MainActivity : AppCompatActivity(), CardView.Callbacks {

    /** The active stack via the typed UniFFI bridge (ADR-0012); null until one is loaded. */
    private var stack: uniffi.hyperffi.HyperStack? = null
    private lateinit var root: FrameLayout
    private lateinit var cardView: CardView
    private lateinit var editor: EditText
    private lateinit var editToggle: Button
    private lateinit var renderToggle: Button
    private lateinit var composeView: ComposeView
    /** Render mode: false = classic Canvas [CardView], true = native Compose (ADR-0008). */
    private var nativeMode = false
    private lateinit var palette: LinearLayout
    private lateinit var propsBtn: Button
    private lateinit var scriptBtn: Button
    private lateinit var delBtn: Button
    private var editingFieldId: Int = -1

    /** Key (asset/file basename, no `.json`) of the stack currently loaded. */
    private var currentKey: String = ""

    /** Per-stack working copies: `filesDir/stacks/<key>.yaml` (legacy `.json` still read). Each
     *  stack persists its own edits, so switching never clobbers another stack. */
    private val stacksDir: File by lazy { File(filesDir, "stacks") }

    /** Host-owned session view state — last-used stack + per-stack card index (ADR-0013). */
    private val prefs: StackPrefs by lazy { StackPrefs(this) }

    /** Legacy plain-text `last_stack` file from before [prefs]; migrated on first run. */
    private val legacyLastStackFile: File by lazy { File(filesDir, "last_stack") }

    /** Legacy single-slot save from before per-stack copies; migrated on first run. */
    private val legacyStack: File by lazy { File(filesDir, "stack.json") }

    companion object {
        private const val DEFAULT_STACK = "productivity"

        /** Supported stack-asset extensions, in load-preference order (YAML is the authoring
         *  format; JSON remains for compatibility and runtime saves). See ADR-0011. */
        private val STACK_ASSET_EXTS = listOf("yaml", "yml", "json")
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()

        root = FrameLayout(this)
        root.id = View.generateViewId()

        cardView = CardView(this).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT,
            )
            callbacks = this@MainActivity
        }
        root.addView(cardView)

        editor = EditText(this).apply {
            visibility = View.GONE
            setBackgroundColor(Color.WHITE)
            setTextColor(Color.BLACK)
            inputType = InputType.TYPE_CLASS_TEXT
            imeOptions = EditorInfo.IME_ACTION_DONE
            gravity = Gravity.CENTER_VERTICAL
            setOnEditorActionListener { _, actionId, _ ->
                if (actionId == EditorInfo.IME_ACTION_DONE) {
                    commitEdit()
                    true
                } else {
                    false
                }
            }
        }
        root.addView(editor)

        // Authoring palette (create/inspect/delete), shown only in edit mode.
        palette = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            visibility = View.GONE
            setBackgroundColor(Color.parseColor("#ECEFF1"))
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
                Gravity.BOTTOM,
            )
        }
        palette.addView(paletteButton("+Btn") { createObject("button") })
        palette.addView(paletteButton("+Fld") { createObject("field") })
        propsBtn = paletteButton("Props") { withSelection(::showInspector) }
        scriptBtn = paletteButton("Script") { withSelection(::onEditScript) }
        delBtn = paletteButton("Del") { withSelection(::deleteObject) }
        palette.addView(propsBtn)
        palette.addView(scriptBtn)
        palette.addView(delBtn)
        onSelectionChanged(-1) // start with the selection-dependent buttons disabled
        root.addView(palette)

        // Authoring toggle: flip taps between "run the script" and "select/move objects".
        editToggle = Button(this).apply {
            text = "Edit"
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
                Gravity.TOP or Gravity.END,
            )
            setOnClickListener {
                commitPendingEdit()
                val on = !cardView.editMode
                cardView.editMode = on // setter clears any selection when turning off
                text = if (on) "Done" else "Edit"
                palette.visibility = if (on) View.VISIBLE else View.GONE
            }
        }
        root.addView(editToggle)

        // Native (Compose) render surface — ADR-0008. Hidden until the render toggle turns it on;
        // the classic CardView stays the default. Authoring (edit mode) remains Canvas-only.
        composeView = ComposeView(this).apply {
            visibility = View.GONE
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT,
            )
        }
        root.addView(composeView)

        // Render-mode toggle: classic Canvas ⇄ native Material (Compose).
        renderToggle = Button(this).apply {
            text = "Native"
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
                Gravity.TOP or Gravity.START,
            )
            setOnClickListener { setNativeMode(!nativeMode) }
        }
        root.addView(renderToggle)

        setContentView(root)
        ViewCompat.setOnApplyWindowInsetsListener(root) { v, insets ->
            val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars())
            v.setPadding(bars.left, bars.top, bars.right, bars.bottom)
            insets
        }

        loadInitialStack()
    }

    // --- render mode (ADR-0008) ---

    /** Switch between the classic Canvas [CardView] and the native Compose [NativeCardScreen]. */
    private fun setNativeMode(on: Boolean) {
        commitPendingEdit()
        nativeMode = on
        if (on && cardView.editMode) { // authoring is Canvas-only; leave edit mode
            cardView.editMode = false
            editToggle.text = "Edit"
            palette.visibility = View.GONE
        }
        renderToggle.text = if (on) "Classic" else "Native"
        editToggle.visibility = if (on) View.GONE else View.VISIBLE
        cardView.visibility = if (on) View.GONE else View.VISIBLE
        composeView.visibility = if (on) View.VISIBLE else View.GONE
        if (on) {
            bindNativeContent()
        } else {
            composeView.setContent {} // release composition; refresh Canvas from current model
            cardView.refresh()
        }
    }

    /** (Re)bind the Compose surface to the current stack — called on toggle and after a load. */
    private fun bindNativeContent() {
        val s = stack ?: return
        composeView.setContent {
            MaterialTheme {
                NativeCardScreen(s) { effects, error -> onEffects(effects, error) }
            }
        }
    }

    // --- stack loading & switching ---

    /** On launch, reopen the last-used stack (or the default), migrating any legacy save. */
    private fun loadInitialStack() {
        migrateLegacySavedStack()
        migrateLegacyLastStack()
        val remembered = runBlocking { prefs.lastStack() }
        val key = (remembered ?: DEFAULT_STACK).let { if (stackContentFor(it) != null) it else DEFAULT_STACK }
        loadStackKey(key)
    }

    /** One-time migration: the old plain-text `last_stack` file becomes a [prefs] entry. */
    private fun migrateLegacyLastStack() {
        if (!legacyLastStackFile.exists()) return
        val key = runCatching { legacyLastStackFile.readText().trim() }.getOrNull()
        if (!key.isNullOrEmpty() && runBlocking { prefs.lastStack() } == null) {
            runBlocking { prefs.setLastStack(key) }
        }
        runCatching { legacyLastStackFile.delete() }
    }

    /** Load the stack named [key]: its saved working copy (YAML; legacy JSON accepted) if
     *  present, else the bundled asset (YAML authoring format, or legacy JSON). Frees the
     *  previous stack and remembers [key] as the stack to reopen. */
    private fun loadStackKey(key: String) {
        val saved = savedCopyFor(key)
        val loaded: uniffi.hyperffi.HyperStack? = if (saved != null) {
            loadContent(runCatching { saved.readText() }.getOrDefault(""), saved.name)
        } else {
            val asset = assetFileFor(key) ?: run {
                Toast.makeText(this, "No stack \"$key\"", Toast.LENGTH_LONG).show()
                return
            }
            val content = runCatching {
                assets.open(asset).bufferedReader().use { it.readText() }
            }.getOrNull().orEmpty()
            loadContent(content, asset)
        }
        if (loaded == null) {
            Toast.makeText(this, "Failed to load stack \"$key\"", Toast.LENGTH_LONG).show()
            return
        }
        stack?.destroy()
        stack = loaded
        currentKey = key
        cardView.stack = loaded
        // Restore the last-viewed card (ADR-0013); openCardAt(0) == openCard for a fresh stack.
        loaded.openCardAt(runBlocking { prefs.cardIndex(key) })
        cardView.refresh()
        if (nativeMode) bindNativeContent() // rebind Compose to the newly-loaded stack
        runBlocking { prefs.setLastStack(key) }
    }

    /** Save the current stack to its own YAML working copy (atomically, so a crash mid-write
     *  can't truncate it), so edits survive a switch or restart. Also persists the current card
     *  index as host-owned session state (ADR-0013). Supersedes any legacy JSON copy. */
    private fun saveCurrentStack() {
        val s = stack ?: return
        if (currentKey.isEmpty()) return
        runCatching {
            writeFileAtomically(File(stacksDir, "$currentKey.yaml"), s.toYaml())
            File(stacksDir, "$currentKey.json").delete()
        }
        runBlocking { prefs.setCardIndex(currentKey, s.currentCardIndex()) }
    }

    /** The saved working copy for [key] — YAML preferred, legacy JSON accepted — or null. */
    private fun savedCopyFor(key: String): File? =
        listOf("$key.yaml", "$key.json").map { File(stacksDir, it) }.firstOrNull { it.exists() }

    /** Load [content] into a [HyperStack] using the parser implied by [filename]'s extension
     *  (YAML vs legacy JSON); null if it fails to parse. */
    private fun loadContent(content: String, filename: String): uniffi.hyperffi.HyperStack? =
        runCatching {
            if (filename.endsWith(".yaml") || filename.endsWith(".yml")) {
                uniffi.hyperffi.HyperStack.loadYaml(content)
            } else {
                uniffi.hyperffi.HyperStack.loadJson(content)
            }
        }.getOrNull()

    private fun showStackPicker() {
        commitPendingEdit()
        val keys = availableStackKeys()
        if (keys.isEmpty()) return
        val labels = keys.map { key ->
            stackDisplayName(key) + if (key == currentKey) "  (current)" else ""
        }.toTypedArray()
        AlertDialog.Builder(this)
            .setTitle("Open stack")
            .setItems(labels) { _, which -> switchToStack(keys[which]) }
            .setNegativeButton(android.R.string.cancel, null)
            .show()
    }

    /** Fulfil a script's `go to stack "Name"`: match a stack by its `name` (case-insensitive)
     *  and switch to it, or toast if no such stack exists. */
    private fun goToStackByName(name: String) {
        val key = availableStackKeys().firstOrNull { stackDisplayName(it).equals(name, ignoreCase = true) }
        if (key == null) {
            Toast.makeText(this, "No stack \"$name\"", Toast.LENGTH_LONG).show()
            return
        }
        switchToStack(key) // no-ops if it's already the current stack
    }

    private fun switchToStack(key: String) {
        if (key == currentKey) return
        commitPendingEdit()
        if (cardView.editMode) { // leave edit mode for a clean switch
            cardView.editMode = false
            editToggle.text = "Edit"
            palette.visibility = View.GONE
        }
        saveCurrentStack()
        loadStackKey(key)
    }

    /** Bundled asset stacks (JSON or YAML) unioned with any saved working copies, by key. */
    private fun availableStackKeys(): List<String> {
        val fromAssets = runCatching {
            assets.list("")
                ?.filter { name -> STACK_ASSET_EXTS.any { name.endsWith(".$it") } }
                ?.map { it.substringBeforeLast('.') }
        }.getOrNull().orEmpty()
        val fromSaved = (
            stacksDir.listFiles { f -> f.name.endsWith(".yaml") || f.name.endsWith(".json") }
                ?: emptyArray()
            ).map { it.name.substringBeforeLast('.') }
        return (fromAssets + fromSaved).distinct().sorted()
    }

    /** The bundled asset filename for [key], preferring YAML, or null if none ships. */
    private fun assetFileFor(key: String): String? {
        val names = runCatching { assets.list("")?.toSet() }.getOrNull().orEmpty()
        return STACK_ASSET_EXTS.map { "$key.$it" }.firstOrNull { it in names }
    }

    /** Raw stack content for [key] — saved working copy if present, else the bundled asset
     *  (any format). Used to read the display name; [loadStackKey] does the actual loading. */
    private fun stackContentFor(key: String): String? {
        savedCopyFor(key)?.let { f -> runCatching { return f.readText() } }
        val asset = assetFileFor(key) ?: return null
        return runCatching { assets.open(asset).bufferedReader().use { it.readText() } }.getOrNull()
    }

    /** A stack's `name` for the picker, falling back to its key (see [stackNameFrom]). */
    private fun stackDisplayName(key: String): String =
        stackContentFor(key)?.let { stackNameFrom(it, key) } ?: key

    /** One-time migration: the old single `stack.json` becomes the default stack's copy. */
    private fun migrateLegacySavedStack() {
        if (!legacyStack.exists()) return
        stacksDir.mkdirs()
        val dest = File(stacksDir, "$DEFAULT_STACK.json")
        if (!dest.exists()) runCatching { legacyStack.copyTo(dest) }
        runCatching { legacyStack.delete() }
    }

    // --- CardView.Callbacks ---

    override fun onEffects(effects: List<HostEffect>, error: String?) {
        error?.let { Toast.makeText(this, "Script error: $it", Toast.LENGTH_LONG).show() }
        for (e in effects) {
            when (e.type) {
                "beep" -> beep()
                "answer" -> AlertDialog.Builder(this)
                    .setMessage(e.text)
                    .setPositiveButton(android.R.string.ok, null)
                    .show()
                "message" -> Toast.makeText(this, e.text, Toast.LENGTH_SHORT).show()
                "gostack" -> goToStackByName(e.text) // `go to stack "Name"`
                "showstacks" -> showStackPicker() // `show stacks`
            }
        }
    }

    override fun onEditField(fieldId: Int, viewRect: RectF, currentText: String) {
        editingFieldId = fieldId
        editor.layoutParams = FrameLayout.LayoutParams(
            viewRect.width().toInt().coerceAtLeast(1),
            viewRect.height().toInt().coerceAtLeast(1),
        ).apply {
            leftMargin = viewRect.left.toInt()
            topMargin = viewRect.top.toInt()
        }
        editor.setText(currentText)
        editor.setSelection(currentText.length)
        editor.visibility = View.VISIBLE
        editor.requestFocus()
        val imm = getSystemService(INPUT_METHOD_SERVICE) as InputMethodManager
        imm.showSoftInput(editor, InputMethodManager.SHOW_IMPLICIT)
    }

    override fun commitPendingEdit() {
        if (editor.visibility == View.VISIBLE) commitEdit()
    }

    override fun onEditScript(objectId: Int) {
        val s = stack ?: return
        val current = s.objectScript(objectId)
        val input = EditText(this).apply {
            setText(current)
            setSelection(text.length)
            inputType = InputType.TYPE_CLASS_TEXT or
                InputType.TYPE_TEXT_FLAG_MULTI_LINE or
                InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
            gravity = Gravity.TOP or Gravity.START
            minLines = 6
            setHorizontallyScrolling(false)
        }
        val dialog = AlertDialog.Builder(this)
            .setTitle("Script · object #$objectId")
            .setView(input)
            .setPositiveButton("Save", null) // overridden below to validate before dismiss
            .setNegativeButton(android.R.string.cancel, null)
            .create()
        dialog.setOnShowListener {
            dialog.getButton(AlertDialog.BUTTON_POSITIVE).setOnClickListener {
                val src = input.text.toString()
                val err = uniffi.hyperffi.checkScript(src)
                if (err.isNotEmpty()) {
                    // Keep the dialog open so the user can fix the source.
                    Toast.makeText(this, "Parse error: $err", Toast.LENGTH_LONG).show()
                } else {
                    stack?.setObjectScript(objectId, src)
                    cardView.refresh()
                    dialog.dismiss()
                }
            }
        }
        dialog.show()
    }

    override fun onSelectionChanged(objectId: Int) {
        val has = objectId >= 0
        propsBtn.isEnabled = has
        scriptBtn.isEnabled = has
        delBtn.isEnabled = has
    }

    // --- authoring palette ---

    private fun paletteButton(label: String, onClick: () -> Unit): Button =
        Button(this).apply {
            text = label
            layoutParams = LinearLayout.LayoutParams(
                0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f,
            )
            setOnClickListener { onClick() }
        }

    /** Run [action] with the current selection, if any. */
    private inline fun withSelection(action: (Int) -> Unit) {
        val id = cardView.selectedId
        if (id >= 0) action(id)
    }

    private fun createObject(kind: String) {
        val s = stack ?: return
        val id = s.addObject(kind)
        if (id >= 0) {
            cardView.refresh()
            cardView.selectObject(id)
        }
    }

    private fun deleteObject(objectId: Int) {
        val s = stack ?: return
        s.deleteObject(objectId)
        cardView.clearSelection()
        cardView.refresh()
    }

    private fun showInspector(objectId: Int) {
        val s = stack ?: return
        val props = s.objectProps(objectId) ?: return // typed ObjectProps (ADR-0012)
        val kind = props.kind

        val container = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(48, 16, 48, 0)
        }
        fun label(t: String): TextView = TextView(this).apply {
            text = t
            setPadding(0, 16, 0, 0)
        }
        fun textField(value: String): EditText = EditText(this).apply {
            setText(value)
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
        }

        val nameInput = textField(props.name)
        container.addView(label("Name"))
        container.addView(nameInput)

        var titleInput: EditText? = null
        var styleSpinner: Spinner? = null
        var textInput: EditText? = null
        var lockedCheck: CheckBox? = null
        var checkedCheck: CheckBox? = null

        if (kind == "button" || kind == "switch") {
            titleInput = textField(props.title)
            container.addView(label(if (kind == "switch") "Label" else "Title"))
            container.addView(titleInput)

            if (kind == "switch") {
                checkedCheck = CheckBox(this).apply {
                    text = "Checked"
                    isChecked = props.checked
                }
                container.addView(checkedCheck)
            } else {
                val styles = listOf("rounded", "rectangle", "transparent")
                styleSpinner = Spinner(this).apply {
                    adapter = ArrayAdapter(
                        this@MainActivity,
                        android.R.layout.simple_spinner_dropdown_item,
                        styles,
                    )
                    setSelection(styles.indexOf(props.style).coerceAtLeast(0))
                }
                container.addView(label("Style"))
                container.addView(styleSpinner)
            }
        } else {
            textInput = textField(props.text)
            container.addView(label("Text"))
            container.addView(textInput)

            lockedCheck = CheckBox(this).apply {
                text = "Locked"
                isChecked = props.locked
            }
            container.addView(lockedCheck)
        }

        // --- text styling (applies to both buttons and fields) ---
        val sizeNow = props.textSize
        val sizeInput = textField(
            if (sizeNow == sizeNow.toLong().toFloat()) sizeNow.toLong().toString() else sizeNow.toString(),
        ).apply { inputType = InputType.TYPE_CLASS_NUMBER or InputType.TYPE_NUMBER_FLAG_DECIMAL }
        container.addView(label("Text size"))
        container.addView(sizeInput)

        val fonts = listOf("default", "sans-serif", "serif", "monospace")
        val fontSpinner = Spinner(this).apply {
            adapter = ArrayAdapter(
                this@MainActivity, android.R.layout.simple_spinner_dropdown_item, fonts,
            )
            val cur = props.textFont.ifEmpty { "default" }
            setSelection(fonts.indexOf(cur).coerceAtLeast(0))
        }
        container.addView(label("Font"))
        container.addView(fontSpinner)

        val aligns = listOf("left", "center", "right")
        val alignSpinner = Spinner(this).apply {
            adapter = ArrayAdapter(
                this@MainActivity, android.R.layout.simple_spinner_dropdown_item, aligns,
            )
            val cur = props.textAlign.ifEmpty { "left" }
            setSelection(aligns.indexOf(cur).coerceAtLeast(0))
        }
        container.addView(label("Align"))
        container.addView(alignSpinner)

        val styleNow = props.textStyle.lowercase()
        val boldCheck = CheckBox(this).apply { text = "Bold"; isChecked = "bold" in styleNow }
        val italicCheck = CheckBox(this).apply { text = "Italic"; isChecked = "italic" in styleNow }
        val underlineCheck =
            CheckBox(this).apply { text = "Underline"; isChecked = "underline" in styleNow }
        container.addView(boldCheck)
        container.addView(italicCheck)
        container.addView(underlineCheck)

        AlertDialog.Builder(this)
            .setTitle("Properties · object #$objectId")
            .setView(ScrollView(this).apply { addView(container) })
            .setPositiveButton("Save") { _, _ ->
                // Rebuild the typed record: edited fields from the inputs, everything else
                // (geometry, the other kind's fields) passed through unchanged from `props`.
                val out = props.copy(
                    name = nameInput.text.toString(),
                    title = titleInput?.text?.toString() ?: props.title,
                    style = (styleSpinner?.selectedItem as? String) ?: props.style,
                    text = textInput?.text?.toString() ?: props.text,
                    locked = lockedCheck?.isChecked ?: props.locked,
                    checked = checkedCheck?.isChecked ?: props.checked,
                    textSize = sizeInput.text.toString().toFloatOrNull() ?: 16f,
                    textFont = (fontSpinner.selectedItem as String).let { if (it == "default") "" else it },
                    textAlign = alignSpinner.selectedItem as String,
                    textStyle = buildList {
                        if (boldCheck.isChecked) add("bold")
                        if (italicCheck.isChecked) add("italic")
                        if (underlineCheck.isChecked) add("underline")
                    }.joinToString(","),
                )
                stack?.setObjectProps(out)
                cardView.refresh()
            }
            .setNegativeButton(android.R.string.cancel, null)
            .show()
    }

    private fun commitEdit() {
        if (editingFieldId >= 0) {
            stack?.setFieldText(editingFieldId, editor.text.toString())
        }
        editingFieldId = -1
        editor.visibility = View.GONE
        val imm = getSystemService(INPUT_METHOD_SERVICE) as InputMethodManager
        imm.hideSoftInputFromWindow(editor.windowToken, 0)
        cardView.refresh()
    }

    private fun beep() {
        runCatching {
            ToneGenerator(AudioManager.STREAM_MUSIC, 80)
                .startTone(ToneGenerator.TONE_PROP_BEEP, 150)
        }
    }

    override fun onPause() {
        super.onPause()
        if (editor.visibility == View.VISIBLE) commitEdit()
        saveCurrentStack()
    }

    override fun onDestroy() {
        super.onDestroy()
        stack?.destroy()
        stack = null
    }
}
