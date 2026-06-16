package org.ghostsinthelab.app.hypercatalog

import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.RectF
import android.util.AttributeSet
import android.view.MotionEvent
import android.view.View
import org.json.JSONObject

/** A draw primitive for one card object, in card coordinates. */
data class DrawItem(
    val kind: String,
    val id: Int,
    val x: Float,
    val y: Float,
    val w: Float,
    val h: Float,
    val text: String,
    val style: String,
    val visible: Boolean,
    val locked: Boolean,
)

/** A host effect emitted by a script (`answer`, `beep`, message-box `put`). */
data class HostEffect(val type: String, val text: String)

/**
 * A HyperCard-style card surface. It renders the current card's [DrawItem]s onto a Canvas
 * (background objects under card objects) with letterboxed scaling, and forwards taps to
 * the Rust core, surfacing results through [callbacks].
 */
class CardView @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null,
) : View(context, attrs) {

    interface Callbacks {
        /** Script side effects to perform (dialog/beep/toast). */
        fun onEffects(effects: List<HostEffect>, error: String?)

        /** An editable field was tapped; open an editor over its on-screen rect. */
        fun onEditField(fieldId: Int, viewRect: RectF, currentText: String)

        /** Commit any in-progress field edit before this tap is handled. */
        fun commitPendingEdit()

        /** Edit-mode tap: the user picked object [objectId] to edit its script. */
        fun onEditScript(objectId: Int)
    }

    var handle: Long = 0L
    var callbacks: Callbacks? = null

    /** When true, a tap selects the object under it for script editing instead of running it. */
    var editMode: Boolean = false

    private var cardW = 360f
    private var cardH = 540f
    private var items: List<DrawItem> = emptyList()

    // card→view mapping (letterboxed)
    private var scale = 1f
    private var offsetX = 0f
    private var offsetY = 0f

    private val cardPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = Color.WHITE }
    private val fieldFill = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = Color.WHITE }
    private val fieldStroke = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.parseColor("#9E9E9E"); style = Paint.Style.STROKE; strokeWidth = 2f
    }
    private val buttonFill = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = Color.parseColor("#E0E0E0") }
    private val buttonStroke = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.parseColor("#424242"); style = Paint.Style.STROKE; strokeWidth = 2f
    }
    private val textPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = Color.BLACK }
    private val buttonText = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.BLACK; textAlign = Paint.Align.CENTER
    }

    init {
        setBackgroundColor(Color.parseColor("#B0BEC5"))
    }

    /** Re-fetch the current card's render list from the core and repaint. */
    fun refresh() {
        if (handle == 0L) return
        parseRender(NativeBridge.nativeRender(handle))
        invalidate()
    }

    private fun parseRender(json: String) {
        val obj = JSONObject(json)
        cardW = obj.optDouble("width", 360.0).toFloat().coerceAtLeast(1f)
        cardH = obj.optDouble("height", 540.0).toFloat().coerceAtLeast(1f)
        val arr = obj.optJSONArray("items") ?: return
        items = (0 until arr.length()).map { i ->
            val it = arr.getJSONObject(i)
            DrawItem(
                kind = it.getString("kind"),
                id = it.getInt("id"),
                x = it.getDouble("x").toFloat(),
                y = it.getDouble("y").toFloat(),
                w = it.getDouble("w").toFloat(),
                h = it.getDouble("h").toFloat(),
                text = it.optString("text"),
                style = it.optString("style"),
                visible = it.optBoolean("visible", true),
                locked = it.optBoolean("locked", false),
            )
        }
    }

    override fun onSizeChanged(w: Int, h: Int, oldw: Int, oldh: Int) {
        recomputeTransform(w.toFloat(), h.toFloat())
    }

    private fun recomputeTransform(viewW: Float, viewH: Float) {
        scale = minOf(viewW / cardW, viewH / cardH)
        offsetX = (viewW - cardW * scale) / 2f
        offsetY = (viewH - cardH * scale) / 2f
    }

    override fun onDraw(canvas: Canvas) {
        recomputeTransform(width.toFloat(), height.toFloat())
        // card paper
        canvas.drawRect(offsetX, offsetY, offsetX + cardW * scale, offsetY + cardH * scale, cardPaint)

        textPaint.textSize = 16f * scale
        buttonText.textSize = 16f * scale

        for (item in items) {
            if (!item.visible) continue
            val r = toView(item)
            when (item.kind) {
                "button" -> drawButton(canvas, item, r)
                "field" -> drawField(canvas, item, r)
            }
        }
    }

    private fun drawButton(canvas: Canvas, item: DrawItem, r: RectF) {
        when (item.style) {
            "transparent" -> {}
            "rectangle" -> {
                canvas.drawRect(r, buttonFill)
                canvas.drawRect(r, buttonStroke)
            }
            else -> { // rounded
                val radius = 12f * scale
                canvas.drawRoundRect(r, radius, radius, buttonFill)
                canvas.drawRoundRect(r, radius, radius, buttonStroke)
            }
        }
        val baseline = r.centerY() - (buttonText.descent() + buttonText.ascent()) / 2f
        canvas.drawText(item.text, r.centerX(), baseline, buttonText)
    }

    private fun drawField(canvas: Canvas, item: DrawItem, r: RectF) {
        if (!item.locked) {
            canvas.drawRect(r, fieldFill)
        }
        canvas.drawRect(r, fieldStroke)
        // single-line text, vertically centered, inset a little
        val baseline = r.centerY() - (textPaint.descent() + textPaint.ascent()) / 2f
        canvas.save()
        canvas.clipRect(r)
        canvas.drawText(item.text, r.left + 6f * scale, baseline, textPaint)
        canvas.restore()
    }

    private fun toView(item: DrawItem): RectF = RectF(
        offsetX + item.x * scale,
        offsetY + item.y * scale,
        offsetX + (item.x + item.w) * scale,
        offsetY + (item.y + item.h) * scale,
    )

    override fun onTouchEvent(event: MotionEvent): Boolean {
        if (event.action == MotionEvent.ACTION_DOWN) return true
        if (event.action != MotionEvent.ACTION_UP) return false
        if (handle == 0L) return true

        // A tap anywhere first commits any field being edited (HyperCard-style), so the
        // tapped script sees the up-to-date field contents.
        callbacks?.commitPendingEdit()

        // view → card coordinates
        val cx = (event.x - offsetX) / scale
        val cy = (event.y - offsetY) / scale

        // Edit mode: select the object under the tap and edit its script — don't run it.
        if (editMode) {
            val id = NativeBridge.nativeObjectAt(handle, cx, cy)
            if (id >= 0) callbacks?.onEditScript(id)
            return true
        }

        val result = JSONObject(NativeBridge.nativeDispatchTouch(handle, cx, cy, "up"))

        val error = if (result.isNull("error")) null else result.optString("error").ifEmpty { null }
        val effects = parseEffects(result.optJSONArray("host_cmds"))
        if (effects.isNotEmpty() || error != null) {
            callbacks?.onEffects(effects, error)
        }

        if (!result.isNull("focus_field")) {
            val id = result.getInt("focus_field")
            items.firstOrNull { it.id == id }?.let { f ->
                callbacks?.onEditField(id, toView(f), f.text)
            }
        }

        if (result.optBoolean("card_changed")) {
            // run the new card's openCard handler, then repaint
            NativeBridge.nativeOpenCard(handle)
            refresh()
        } else if (result.optBoolean("needs_redraw")) {
            refresh()
        }
        return true
    }

    private fun parseEffects(arr: org.json.JSONArray?): List<HostEffect> {
        if (arr == null) return emptyList()
        return (0 until arr.length()).map { i ->
            val o = arr.getJSONObject(i)
            HostEffect(o.optString("type"), o.optString("text"))
        }
    }
}
