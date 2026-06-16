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
import android.widget.Button
import android.widget.EditText
import android.widget.FrameLayout
import android.widget.Toast
import androidx.activity.enableEdgeToEdge
import androidx.appcompat.app.AlertDialog
import androidx.appcompat.app.AppCompatActivity
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import java.io.File

/**
 * Hosts the [CardView] for the active stack. Loads a stack (a previously saved one if
 * present, otherwise the bundled productivity stack), wires script side effects to platform UI
 * (dialogs/beeps/toasts), provides an [EditText] overlay for editing fields, and saves the
 * stack on pause.
 */
class MainActivity : AppCompatActivity(), CardView.Callbacks {

    private var handle: Long = 0L
    private lateinit var root: FrameLayout
    private lateinit var cardView: CardView
    private lateinit var editor: EditText
    private lateinit var editToggle: Button
    private var editingFieldId: Int = -1

    private val savedStack: File by lazy { File(filesDir, "stack.json") }

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

        // Authoring toggle: flip taps between "run the script" and "edit the script".
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
                cardView.editMode = on
                text = if (on) "Done" else "Edit"
            }
        }
        root.addView(editToggle)

        setContentView(root)
        ViewCompat.setOnApplyWindowInsetsListener(root) { v, insets ->
            val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars())
            v.setPadding(bars.left, bars.top, bars.right, bars.bottom)
            insets
        }

        loadStack()
    }

    private fun loadStack() {
        val json = if (savedStack.exists()) {
            runCatching { savedStack.readText() }.getOrNull()
        } else {
            null
        } ?: assets.open("productivity.json").bufferedReader().use { it.readText() }

        handle = NativeBridge.nativeLoad(json)
        if (handle == 0L) {
            Toast.makeText(this, "Failed to load stack", Toast.LENGTH_LONG).show()
            return
        }
        cardView.handle = handle
        NativeBridge.nativeOpenCard(handle)
        cardView.refresh()
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
        if (handle != 0L) {
            runCatching { savedStack.writeText(NativeBridge.nativeToJson(handle)) }
        }
    }

    override fun onDestroy() {
        super.onDestroy()
        if (handle != 0L) {
            NativeBridge.nativeFree(handle)
            handle = 0L
        }
    }
}
