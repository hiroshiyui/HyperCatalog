package org.ghostsinthelab.app.hypercatalog

/**
 * Flatten the typed bridge [uniffi.hyperffi.HostEffect]s of a dispatch into the host's
 * `(type, text)` [HostEffect] form. Shared by both render targets — the Canvas [CardView] and the
 * Compose [NativeCardScreen] — so script side effects surface identically regardless of renderer.
 * (Bridge-coupled, so it lives here rather than in the framework-free `HostLogic`.)
 */
fun hostEffectsOf(cmds: List<uniffi.hyperffi.HostEffect>): List<HostEffect> = cmds.map { e ->
    when (e) {
        is uniffi.hyperffi.HostEffect.Answer -> HostEffect("answer", e.text)
        is uniffi.hyperffi.HostEffect.Message -> HostEffect("message", e.text)
        is uniffi.hyperffi.HostEffect.Beep -> HostEffect("beep", "")
        is uniffi.hyperffi.HostEffect.GoStack -> HostEffect("gostack", e.name)
        is uniffi.hyperffi.HostEffect.ShowStacks -> HostEffect("showstacks", "")
    }
}
