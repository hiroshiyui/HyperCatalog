package org.ghostsinthelab.app.hypercatalog

/**
 * The letterbox mapping between a card's logical coordinate space and on-screen view
 * pixels. The card is scaled uniformly to fit the view and centered (so it never distorts),
 * which lets us project card rects onto the canvas and map taps back to card coordinates.
 *
 * This is deliberately pure float math with **no Android dependencies**, so the coordinate
 * logic that backs touch hit-testing is unit-testable on the JVM without an emulator.
 */
data class CardTransform(
    val scale: Float,
    val offsetX: Float,
    val offsetY: Float,
) {
    fun cardToViewX(x: Float): Float = offsetX + x * scale
    fun cardToViewY(y: Float): Float = offsetY + y * scale
    fun viewToCardX(x: Float): Float = (x - offsetX) / scale
    fun viewToCardY(y: Float): Float = (y - offsetY) / scale

    companion object {
        /**
         * Uniform letterbox fit of a [cardW]×[cardH] card into a [viewW]×[viewH] view: the
         * larger axis gets equal margins on both sides so the card stays centered.
         */
        fun fit(viewW: Float, viewH: Float, cardW: Float, cardH: Float): CardTransform {
            val s = minOf(viewW / cardW, viewH / cardH)
            return CardTransform(s, (viewW - cardW * s) / 2f, (viewH - cardH * s) / 2f)
        }
    }
}
