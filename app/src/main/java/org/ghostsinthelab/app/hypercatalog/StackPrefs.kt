package org.ghostsinthelab.app.hypercatalog

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.intPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import kotlinx.coroutines.flow.first

/** App-process singleton Preferences DataStore for host session state (`filesDir/datastore/`). */
private val Context.sessionStore: DataStore<Preferences> by preferencesDataStore(name = "session")

/**
 * Host-owned **session view state** — which stack to reopen, and each stack's last-viewed card
 * index — backed by a Preferences DataStore. This is deliberately *not* part of the stack
 * document (ADR-0013): view position lives with the viewer, so a shared/exported stack opens to
 * its first card and the portable YAML stays cursor-free. (The stack *content* is still saved as
 * YAML files; see [MainActivity.saveCurrentStack].)
 *
 * Reads/writes are `suspend`. Callers on the UI thread wrap the few startup/teardown calls in
 * `runBlocking` (tiny local reads, equivalent to the file IO they replaced) so that view state is
 * durably persisted by the time `onPause`/stack-switch returns.
 */
class StackPrefs(private val context: Context) {

    /** The stack key to reopen on launch, or null if none was recorded yet. */
    suspend fun lastStack(): String? =
        context.sessionStore.data.first()[LAST_STACK]?.takeIf { it.isNotEmpty() }

    suspend fun setLastStack(key: String) {
        context.sessionStore.edit { it[LAST_STACK] = key }
    }

    /** [stackKey]'s last-viewed card index (0 if never saved). */
    suspend fun cardIndex(stackKey: String): Int =
        context.sessionStore.data.first()[intPreferencesKey(cardIndexPrefKey(stackKey))] ?: 0

    suspend fun setCardIndex(stackKey: String, index: Int) {
        context.sessionStore.edit { it[intPreferencesKey(cardIndexPrefKey(stackKey))] = index }
    }

    companion object {
        private val LAST_STACK = stringPreferencesKey("last_stack")
    }
}
