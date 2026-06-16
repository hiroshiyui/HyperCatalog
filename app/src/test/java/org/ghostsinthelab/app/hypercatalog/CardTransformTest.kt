package org.ghostsinthelab.app.hypercatalog

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * JVM unit tests for the letterbox coordinate mapping. This is the pure math behind
 * touch hit-testing in [CardView]; it needs no emulator, so it runs as a local test.
 */
class CardTransformTest {

    private val eps = 1e-4f

    @Test
    fun exactFitHasUnitScaleAndNoMargins() {
        val tf = CardTransform.fit(360f, 540f, 360f, 540f)
        assertEquals(1f, tf.scale, eps)
        assertEquals(0f, tf.offsetX, eps)
        assertEquals(0f, tf.offsetY, eps)
    }

    @Test
    fun viewWiderThanCardLetterboxesHorizontally() {
        // Card 360x540 into a 800x540 view: height is the binding axis (scale 1), so the
        // extra width is split into equal left/right margins.
        val tf = CardTransform.fit(800f, 540f, 360f, 540f)
        assertEquals(1f, tf.scale, eps)
        assertEquals((800f - 360f) / 2f, tf.offsetX, eps)
        assertEquals(0f, tf.offsetY, eps)
    }

    @Test
    fun viewTallerThanCardLetterboxesVertically() {
        // Card 360x540 into a 360x1080 view: width binds (scale 1), extra height centered.
        val tf = CardTransform.fit(360f, 1080f, 360f, 540f)
        assertEquals(1f, tf.scale, eps)
        assertEquals(0f, tf.offsetX, eps)
        assertEquals((1080f - 540f) / 2f, tf.offsetY, eps)
    }

    @Test
    fun fitPicksTheSmallerScaleSoTheCardNeverOverflows() {
        // 720 wide / 360 = 2.0, but 540 tall / 540 = 1.0; the smaller (1.0) must win.
        val tf = CardTransform.fit(720f, 540f, 360f, 540f)
        assertEquals(1f, tf.scale, eps)
        assertEquals((720f - 360f) / 2f, tf.offsetX, eps)
        assertEquals(0f, tf.offsetY, eps)
    }

    @Test
    fun uniformUpscaleMapsCardPointsToViewPoints() {
        // Double-size view: scale 2, no margins.
        val tf = CardTransform.fit(720f, 1080f, 360f, 540f)
        assertEquals(2f, tf.scale, eps)
        assertEquals(20f, tf.cardToViewX(10f), eps)
        assertEquals(30f, tf.cardToViewY(15f), eps)
    }

    @Test
    fun viewToCardInvertsCardToView() {
        val tf = CardTransform.fit(800f, 540f, 360f, 540f) // scale 1, offsetX 220
        // A tap at the card's top-left corner maps back to (0,0)...
        assertEquals(0f, tf.viewToCardX(tf.cardToViewX(0f)), eps)
        assertEquals(0f, tf.viewToCardY(tf.cardToViewY(0f)), eps)
        // ...and an arbitrary interior point round-trips.
        assertEquals(123.5f, tf.viewToCardX(tf.cardToViewX(123.5f)), eps)
        assertEquals(456.25f, tf.viewToCardY(tf.cardToViewY(456.25f)), eps)
    }

    @Test
    fun conversionsHonorScaleAndOffsetTogether() {
        val tf = CardTransform(scale = 2f, offsetX = 100f, offsetY = 50f)
        assertEquals(120f, tf.cardToViewX(10f), eps) // 100 + 10*2
        assertEquals(70f, tf.cardToViewY(10f), eps) //  50 + 10*2
        assertEquals(10f, tf.viewToCardX(120f), eps)
        assertEquals(10f, tf.viewToCardY(70f), eps)
    }

    @Test
    fun cardStaysCenteredWithSymmetricMargins() {
        // The right/bottom margins must equal the left/top ones.
        val tf = CardTransform.fit(800f, 1000f, 360f, 540f)
        val rightMargin = 800f - tf.cardToViewX(360f)
        val bottomMargin = 1000f - tf.cardToViewY(540f)
        assertEquals(tf.offsetX, rightMargin, eps)
        assertEquals(tf.offsetY, bottomMargin, eps)
    }
}
