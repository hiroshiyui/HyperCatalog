package org.ghostsinthelab.app.hypercatalog

import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.RectF
import android.graphics.Typeface
import android.util.AttributeSet
import android.view.GestureDetector
import android.view.MotionEvent
import android.view.View
import org.json.JSONObject
import kotlin.math.abs

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
    val textFont: String,
    val textSize: Float,
    val textStyle: String,
    val textAlign: String,
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

        /** Open the script editor for [objectId] (invoked from the host's edit palette). */
        fun onEditScript(objectId: Int)

        /** The edit-mode selection changed; [objectId] is -1 when nothing is selected. */
        fun onSelectionChanged(objectId: Int)
    }

    var handle: Long = 0L
    var callbacks: Callbacks? = null

    /** When true, taps select objects and drags move/resize them, instead of running scripts. */
    var editMode: Boolean = false
        set(value) {
            field = value
            if (!value) clearSelection() else invalidate()
        }

    /** Currently selected object id in edit mode, or -1. */
    var selectedId: Int = -1
        private set

    private enum class Drag { NONE, MOVE, RESIZE }
    private var drag = Drag.NONE
    private var grabDx = 0f // pointer offset within object (card units), for MOVE
    private var grabDy = 0f
    /** Live rect during a drag, in card coords (left,top,right,bottom); null when not dragging. */
    private var draft: RectF? = null
    private val handleHalfPx = 16f // resize-handle half-size, in view pixels
    private val minObject = 12f // minimum object size in card units (mirrors core clamp)

    // Browse-mode gesture state.
    private val swipeMinPx = 48f // minimum fling travel (view px) to count as a swipe
    /** True once a long-press/swipe/double-tap handled the current touch, so ACTION_UP
     *  doesn't also fire a tap. Reset at each ACTION_DOWN. */
    private var gestureConsumed = false

    private var cardW = 360f
    private var cardH = 540f
    private var items: List<DrawItem> = emptyList()

    // card→view mapping (letterboxed); see CardTransform for the pure math.
    private var tf = CardTransform(1f, 0f, 0f)

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
    private val selectStroke = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.parseColor("#1565C0"); style = Paint.Style.STROKE; strokeWidth = 3f
    }
    private val handleFill = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.parseColor("#1565C0")
    }

    init {
        setBackgroundColor(Color.parseColor("#B0BEC5"))
    }

    /** Select an object by id (e.g. a freshly created one) and repaint. */
    fun selectObject(id: Int) {
        selectedId = id
        invalidate()
        callbacks?.onSelectionChanged(id)
    }

    /** Clear any edit-mode selection. */
    fun clearSelection() {
        drag = Drag.NONE
        draft = null
        if (selectedId != -1) {
            selectedId = -1
            callbacks?.onSelectionChanged(-1)
        }
        invalidate()
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
                textFont = it.optString("text_font"),
                textSize = it.optDouble("text_size", 16.0).toFloat(),
                textStyle = it.optString("text_style"),
                textAlign = it.optString("text_align"),
            )
        }
    }

    override fun onSizeChanged(w: Int, h: Int, oldw: Int, oldh: Int) {
        recomputeTransform(w.toFloat(), h.toFloat())
    }

    private fun recomputeTransform(viewW: Float, viewH: Float) {
        tf = CardTransform.fit(viewW, viewH, cardW, cardH)
    }

    override fun onDraw(canvas: Canvas) {
        recomputeTransform(width.toFloat(), height.toFloat())
        // card paper
        canvas.drawRect(
            tf.cardToViewX(0f), tf.cardToViewY(0f),
            tf.cardToViewX(cardW), tf.cardToViewY(cardH), cardPaint,
        )

        for (item in items) {
            if (!item.visible) continue
            val dragging = editMode && item.id == selectedId && draft != null
            val r = if (dragging) cardRectToView(draft!!) else toView(item)
            when (item.kind) {
                "button" -> drawButton(canvas, item, r)
                "field" -> drawField(canvas, item, r)
            }
            if (editMode && item.id == selectedId) drawSelection(canvas, r)
        }
    }

    private fun drawSelection(canvas: Canvas, r: RectF) {
        canvas.drawRect(r, selectStroke)
        canvas.drawRect(
            r.right - handleHalfPx, r.bottom - handleHalfPx,
            r.right + handleHalfPx, r.bottom + handleHalfPx, handleFill,
        )
    }

    private fun drawButton(canvas: Canvas, item: DrawItem, r: RectF) {
        when (item.style) {
            "transparent" -> {}
            "rectangle" -> {
                canvas.drawRect(r, buttonFill)
                canvas.drawRect(r, buttonStroke)
            }
            else -> { // rounded
                val radius = 12f * tf.scale
                canvas.drawRoundRect(r, radius, radius, buttonFill)
                canvas.drawRoundRect(r, radius, radius, buttonStroke)
            }
        }
        applyTextStyle(buttonText, item) // labels stay centered (buttonText keeps Align.CENTER)
        val baseline = r.centerY() - (buttonText.descent() + buttonText.ascent()) / 2f
        canvas.drawText(item.text, r.centerX(), baseline, buttonText)
    }

    private fun drawField(canvas: Canvas, item: DrawItem, r: RectF) {
        if (!item.locked) {
            canvas.drawRect(r, fieldFill)
        }
        canvas.drawRect(r, fieldStroke)
        applyTextStyle(textPaint, item)
        // single-line text, vertically centered, horizontally aligned per textAlign
        val baseline = r.centerY() - (textPaint.descent() + textPaint.ascent()) / 2f
        val pad = 6f * tf.scale
        val (x, align) = when (item.textAlign.lowercase()) {
            "center" -> r.centerX() to Paint.Align.CENTER
            "right" -> (r.right - pad) to Paint.Align.RIGHT
            else -> (r.left + pad) to Paint.Align.LEFT
        }
        textPaint.textAlign = align
        canvas.save()
        canvas.clipRect(r)
        canvas.drawText(item.text, x, baseline, textPaint)
        canvas.restore()
    }

    /** Configure a text paint from an item's font/size/style attributes (scaled to view px). */
    private fun applyTextStyle(paint: Paint, item: DrawItem) {
        paint.textSize = (if (item.textSize > 0f) item.textSize else 16f) * tf.scale
        val s = item.textStyle.lowercase()
        val flags = (if ("bold" in s) Typeface.BOLD else 0) or
            (if ("italic" in s) Typeface.ITALIC else 0)
        val family = when (item.textFont.lowercase()) {
            "serif" -> Typeface.SERIF
            "monospace", "mono" -> Typeface.MONOSPACE
            "", "sans-serif", "sans", "default" -> Typeface.SANS_SERIF
            else -> Typeface.create(item.textFont, Typeface.NORMAL)
        }
        paint.typeface = Typeface.create(family, flags)
        paint.isUnderlineText = "underline" in s
    }

    private fun toView(item: DrawItem): RectF = RectF(
        tf.cardToViewX(item.x),
        tf.cardToViewY(item.y),
        tf.cardToViewX(item.x + item.w),
        tf.cardToViewY(item.y + item.h),
    )

    /** Map a card-space rect (left,top,right,bottom) to view coordinates. */
    private fun cardRectToView(c: RectF): RectF = RectF(
        tf.cardToViewX(c.left),
        tf.cardToViewY(c.top),
        tf.cardToViewX(c.right),
        tf.cardToViewY(c.bottom),
    )

    override fun onTouchEvent(event: MotionEvent): Boolean {
        if (handle == 0L) return true
        if (editMode) return handleEditTouch(event)
        return handleBrowseTouch(event)
    }

    /**
     * Browse-mode touch. Every event is fed to [gestureDetector], which fires the
     * touchscreen gestures (long-press, double-tap, swipe). A plain completed tap still
     * dispatches `mouseUp` here (the post-WIMP "click"); [gestureConsumed] suppresses that
     * tap when a richer gesture already handled the sequence.
     */
    private fun handleBrowseTouch(event: MotionEvent): Boolean {
        // Reset the flag at sequence start *before* feeding the detector — a double-tap is
        // recognized within this very DOWN, and must not be cleared afterward.
        if (event.action == MotionEvent.ACTION_DOWN) gestureConsumed = false
        gestureDetector.onTouchEvent(event)

        return when (event.action) {
            MotionEvent.ACTION_DOWN -> true
            MotionEvent.ACTION_UP -> {
                if (!gestureConsumed) {
                    // A tap first commits any field being edited (HyperCard-style), so the
                    // tapped script sees up-to-date field contents.
                    callbacks?.commitPendingEdit()
                    val cx = tf.viewToCardX(event.x)
                    val cy = tf.viewToCardY(event.y)
                    applyDispatchResult(JSONObject(NativeBridge.nativeDispatchTouch(handle, cx, cy, "up")))
                }
                true
            }
            else -> false
        }
    }

    /** Recognizes touchscreen gestures and routes them to the core as named messages. */
    private val gestureDetector =
        GestureDetector(context, object : GestureDetector.SimpleOnGestureListener() {
            override fun onDown(e: MotionEvent): Boolean = true // so later callbacks fire

            override fun onLongPress(e: MotionEvent) {
                gestureConsumed = true
                dispatchGestureAt(e.x, e.y, "longPress")
            }

            override fun onDoubleTap(e: MotionEvent): Boolean {
                gestureConsumed = true
                dispatchGestureAt(e.x, e.y, "doubleTap")
                return true
            }

            override fun onFling(
                e1: MotionEvent?,
                e2: MotionEvent,
                velocityX: Float,
                velocityY: Float,
            ): Boolean {
                val gesture = flingGesture(e1, e2) ?: return false
                gestureConsumed = true
                // Target the object the swipe began on (start point), then bubble.
                val start = e1 ?: e2
                dispatchGestureAt(start.x, start.y, gesture)
                return true
            }
        })

    /** Classify a fling into a swipe message, or null if it's too short to count. */
    private fun flingGesture(e1: MotionEvent?, e2: MotionEvent): String? {
        val down = e1 ?: return null
        val dx = e2.x - down.x
        val dy = e2.y - down.y
        if (abs(dx) < swipeMinPx && abs(dy) < swipeMinPx) return null
        return if (abs(dx) > abs(dy)) {
            if (dx > 0) "swipeRight" else "swipeLeft"
        } else {
            if (dy > 0) "swipeDown" else "swipeUp"
        }
    }

    /** Map a view-space gesture point to card coords and dispatch it to the core. */
    private fun dispatchGestureAt(viewX: Float, viewY: Float, gesture: String) {
        callbacks?.commitPendingEdit()
        val cx = tf.viewToCardX(viewX)
        val cy = tf.viewToCardY(viewY)
        applyDispatchResult(JSONObject(NativeBridge.nativeDispatchGesture(handle, cx, cy, gesture)))
    }

    /**
     * Post-process a DispatchResult: surface effects/errors, open the field editor when
     * asked (tap path only; gestures never set `focus_field`), and repaint — running the new
     * card's `openCard` on a navigation. Shared by the tap and gesture paths.
     */
    private fun applyDispatchResult(result: JSONObject) {
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
            NativeBridge.nativeOpenCard(handle)
            refresh()
        } else if (result.optBoolean("needs_redraw")) {
            refresh()
        }
    }

    private fun parseEffects(arr: org.json.JSONArray?): List<HostEffect> {
        if (arr == null) return emptyList()
        return (0 until arr.length()).map { i ->
            val o = arr.getJSONObject(i)
            HostEffect(o.optString("type"), o.optString("text"))
        }
    }

    /**
     * Edit-mode touch: DOWN selects (or grabs the resize handle), MOVE drags a draft rect
     * locally (no bridge calls), UP commits the new rect to the core once.
     */
    private fun handleEditTouch(event: MotionEvent): Boolean {
        val cx = tf.viewToCardX(event.x)
        val cy = tf.viewToCardY(event.y)
        when (event.action) {
            MotionEvent.ACTION_DOWN -> {
                callbacks?.commitPendingEdit()
                val sel = items.firstOrNull { it.id == selectedId }
                if (sel != null && inHandle(sel, cx, cy)) {
                    drag = Drag.RESIZE
                    draft = RectF(sel.x, sel.y, sel.x + sel.w, sel.y + sel.h)
                } else {
                    val id = NativeBridge.nativeObjectAt(handle, cx, cy)
                    if (id >= 0) {
                        if (id != selectedId) selectObject(id)
                        val obj = items.first { it.id == id }
                        drag = Drag.MOVE
                        grabDx = cx - obj.x
                        grabDy = cy - obj.y
                        draft = RectF(obj.x, obj.y, obj.x + obj.w, obj.y + obj.h)
                    } else {
                        clearSelection()
                    }
                }
                invalidate()
            }

            MotionEvent.ACTION_MOVE -> {
                val d = draft ?: return true
                when (drag) {
                    Drag.MOVE -> {
                        val w = d.width()
                        val h = d.height()
                        val nx = cx - grabDx
                        val ny = cy - grabDy
                        d.set(nx, ny, nx + w, ny + h)
                    }
                    Drag.RESIZE -> d.set(
                        d.left, d.top,
                        maxOf(cx, d.left + minObject), maxOf(cy, d.top + minObject),
                    )
                    Drag.NONE -> {}
                }
                invalidate()
            }

            MotionEvent.ACTION_UP -> {
                val d = draft
                if (d != null && drag != Drag.NONE && selectedId >= 0) {
                    NativeBridge.nativeSetObjectRect(
                        handle, selectedId, d.left, d.top, d.width(), d.height(),
                    )
                    refresh()
                }
                drag = Drag.NONE
                draft = null
                invalidate()
            }
        }
        return true
    }

    /** True if (cx,cy) in card coords is within the selected object's bottom-right handle. */
    private fun inHandle(item: DrawItem, cx: Float, cy: Float): Boolean {
        val hx = item.x + item.w
        val hy = item.y + item.h
        val s = handleHalfPx / tf.scale
        return cx in (hx - s)..(hx + s) && cy in (hy - s)..(hy + s)
    }
}
