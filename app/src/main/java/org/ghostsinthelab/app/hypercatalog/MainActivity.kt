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
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import org.json.JSONObject
import java.io.File

/**
 * Hosts the [CardView] for the active stack. Reopens the last-used stack (or the bundled
 * default), offers a "Stacks" picker to switch between bundled assets and their saved working
 * copies, wires script side effects to platform UI (dialogs/beeps/toasts), provides an
 * [EditText] overlay for editing fields, and saves each stack's edits to its own copy on
 * pause/switch (see [stacksDir]).
 */
class MainActivity : AppCompatActivity(), CardView.Callbacks {

    private var handle: Long = 0L
    private lateinit var root: FrameLayout
    private lateinit var cardView: CardView
    private lateinit var editor: EditText
    private lateinit var editToggle: Button
    private lateinit var palette: LinearLayout
    private lateinit var propsBtn: Button
    private lateinit var scriptBtn: Button
    private lateinit var delBtn: Button
    private lateinit var stacksBtn: Button
    private var editingFieldId: Int = -1

    /** Key (asset/file basename, no `.json`) of the stack currently loaded. */
    private var currentKey: String = ""

    /** Per-stack working copies: `filesDir/stacks/<key>.json`. Each stack persists its own
     *  edits, so switching never clobbers another stack. */
    private val stacksDir: File by lazy { File(filesDir, "stacks") }

    /** Remembers which stack to reopen on next launch. */
    private val lastStackFile: File by lazy { File(filesDir, "last_stack") }

    /** Legacy single-slot save from before per-stack copies; migrated on first run. */
    private val legacyStack: File by lazy { File(filesDir, "stack.json") }

    companion object {
        private const val DEFAULT_STACK = "productivity"
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

        // Stack picker, top-start (opposite the Edit toggle).
        stacksBtn = Button(this).apply {
            text = "Stacks"
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                ViewGroup.LayoutParams.WRAP_CONTENT,
                Gravity.TOP or Gravity.START,
            )
            setOnClickListener { showStackPicker() }
        }
        root.addView(stacksBtn)

        setContentView(root)
        ViewCompat.setOnApplyWindowInsetsListener(root) { v, insets ->
            val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars())
            v.setPadding(bars.left, bars.top, bars.right, bars.bottom)
            insets
        }

        loadInitialStack()
    }

    // --- stack loading & switching ---

    /** On launch, reopen the last-used stack (or the default), migrating any legacy save. */
    private fun loadInitialStack() {
        migrateLegacySavedStack()
        val remembered = lastStackFile.takeIf { it.exists() }
            ?.let { runCatching { it.readText().trim() }.getOrNull() }
            ?.takeIf { it.isNotEmpty() }
        val key = (remembered ?: DEFAULT_STACK).let { if (stackJsonFor(it) != null) it else DEFAULT_STACK }
        loadStackKey(key)
    }

    /** Load the stack named [key]: its saved working copy if present, else the bundled asset.
     *  Frees the previous handle. Remembers [key] as the stack to reopen. */
    private fun loadStackKey(key: String) {
        val json = stackJsonFor(key) ?: run {
            Toast.makeText(this, "No stack \"$key\"", Toast.LENGTH_LONG).show()
            return
        }
        val newHandle = NativeBridge.nativeLoad(json)
        if (newHandle == 0L) {
            Toast.makeText(this, "Failed to load stack \"$key\"", Toast.LENGTH_LONG).show()
            return
        }
        if (handle != 0L) NativeBridge.nativeFree(handle)
        handle = newHandle
        currentKey = key
        cardView.handle = handle
        NativeBridge.nativeOpenCard(handle)
        cardView.refresh()
        runCatching { lastStackFile.writeText(key) }
    }

    /** Save the current stack to its own working copy, so edits survive a switch or restart. */
    private fun saveCurrentStack() {
        if (handle != 0L && currentKey.isNotEmpty()) {
            stacksDir.mkdirs()
            runCatching {
                File(stacksDir, "$currentKey.json").writeText(NativeBridge.nativeToJson(handle))
            }
        }
    }

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

    /** Bundled asset stacks unioned with any saved working copies, by key. */
    private fun availableStackKeys(): List<String> {
        val fromAssets = runCatching {
            assets.list("")?.filter { it.endsWith(".json") }?.map { it.removeSuffix(".json") }
        }.getOrNull().orEmpty()
        val fromSaved = (stacksDir.listFiles { f -> f.name.endsWith(".json") } ?: emptyArray())
            .map { it.name.removeSuffix(".json") }
        return (fromAssets + fromSaved).distinct().sorted()
    }

    /** The effective JSON for [key] — the saved working copy if present, else the asset. */
    private fun stackJsonFor(key: String): String? {
        File(stacksDir, "$key.json").takeIf { it.exists() }?.let { f ->
            runCatching { return f.readText() }
        }
        return runCatching {
            assets.open("$key.json").bufferedReader().use { it.readText() }
        }.getOrNull()
    }

    /** A stack's `name` for the picker, falling back to its key. */
    private fun stackDisplayName(key: String): String =
        stackJsonFor(key)
            ?.let { runCatching { JSONObject(it).optString("name") }.getOrNull() }
            ?.takeIf { it.isNotEmpty() }
            ?: key

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
        if (handle == 0L) return
        val current = NativeBridge.nativeGetObjectScript(handle, objectId)
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
                val err = NativeBridge.nativeCheckScript(src)
                if (err.isNotEmpty()) {
                    // Keep the dialog open so the user can fix the source.
                    Toast.makeText(this, "Parse error: $err", Toast.LENGTH_LONG).show()
                } else {
                    NativeBridge.nativeSetObjectScript(handle, objectId, src)
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
        if (handle == 0L) return
        val id = NativeBridge.nativeAddObject(handle, kind)
        if (id >= 0) {
            cardView.refresh()
            cardView.selectObject(id)
        }
    }

    private fun deleteObject(objectId: Int) {
        if (handle == 0L) return
        NativeBridge.nativeDeleteObject(handle, objectId)
        cardView.clearSelection()
        cardView.refresh()
    }

    private fun showInspector(objectId: Int) {
        if (handle == 0L) return
        val json = NativeBridge.nativeGetObjectProps(handle, objectId)
        if (json.isEmpty()) return
        val props = JSONObject(json)
        val kind = props.optString("kind")

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

        val nameInput = textField(props.optString("name"))
        container.addView(label("Name"))
        container.addView(nameInput)

        var titleInput: EditText? = null
        var styleSpinner: Spinner? = null
        var textInput: EditText? = null
        var lockedCheck: CheckBox? = null

        if (kind == "button") {
            titleInput = textField(props.optString("title"))
            container.addView(label("Title"))
            container.addView(titleInput)

            val styles = listOf("rounded", "rectangle", "transparent")
            styleSpinner = Spinner(this).apply {
                adapter = ArrayAdapter(
                    this@MainActivity,
                    android.R.layout.simple_spinner_dropdown_item,
                    styles,
                )
                setSelection(styles.indexOf(props.optString("style")).coerceAtLeast(0))
            }
            container.addView(label("Style"))
            container.addView(styleSpinner)
        } else {
            textInput = textField(props.optString("text"))
            container.addView(label("Text"))
            container.addView(textInput)

            lockedCheck = CheckBox(this).apply {
                text = "Locked"
                isChecked = props.optBoolean("locked")
            }
            container.addView(lockedCheck)
        }

        // --- text styling (applies to both buttons and fields) ---
        val sizeNow = props.optDouble("text_size", 16.0)
        val sizeInput = textField(
            if (sizeNow == sizeNow.toLong().toDouble()) sizeNow.toLong().toString() else sizeNow.toString(),
        ).apply { inputType = InputType.TYPE_CLASS_NUMBER or InputType.TYPE_NUMBER_FLAG_DECIMAL }
        container.addView(label("Text size"))
        container.addView(sizeInput)

        val fonts = listOf("default", "sans-serif", "serif", "monospace")
        val fontSpinner = Spinner(this).apply {
            adapter = ArrayAdapter(
                this@MainActivity, android.R.layout.simple_spinner_dropdown_item, fonts,
            )
            val cur = props.optString("text_font").ifEmpty { "default" }
            setSelection(fonts.indexOf(cur).coerceAtLeast(0))
        }
        container.addView(label("Font"))
        container.addView(fontSpinner)

        val aligns = listOf("left", "center", "right")
        val alignSpinner = Spinner(this).apply {
            adapter = ArrayAdapter(
                this@MainActivity, android.R.layout.simple_spinner_dropdown_item, aligns,
            )
            val cur = props.optString("text_align").ifEmpty { "left" }
            setSelection(aligns.indexOf(cur).coerceAtLeast(0))
        }
        container.addView(label("Align"))
        container.addView(alignSpinner)

        val styleNow = props.optString("text_style").lowercase()
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
                val out = JSONObject().put("name", nameInput.text.toString())
                if (kind == "button") {
                    out.put("title", titleInput!!.text.toString())
                    out.put("style", styleSpinner!!.selectedItem as String)
                } else {
                    out.put("text", textInput!!.text.toString())
                    out.put("locked", lockedCheck!!.isChecked)
                }
                out.put("text_size", sizeInput.text.toString().toDoubleOrNull() ?: 16.0)
                out.put(
                    "text_font",
                    (fontSpinner.selectedItem as String).let { if (it == "default") "" else it },
                )
                out.put("text_align", alignSpinner.selectedItem as String)
                out.put(
                    "text_style",
                    buildList {
                        if (boldCheck.isChecked) add("bold")
                        if (italicCheck.isChecked) add("italic")
                        if (underlineCheck.isChecked) add("underline")
                    }.joinToString(","),
                )
                NativeBridge.nativeSetObjectProps(handle, objectId, out.toString())
                cardView.refresh()
            }
            .setNegativeButton(android.R.string.cancel, null)
            .show()
    }

    private fun commitEdit() {
        if (editingFieldId >= 0 && handle != 0L) {
            NativeBridge.nativeSetFieldText(handle, editingFieldId, editor.text.toString())
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
        if (handle != 0L) {
            NativeBridge.nativeFree(handle)
            handle = 0L
        }
    }
}
