# ADR-0013 — Persistence layering: document files vs. session view state

- Status: **Accepted** — implemented.
- Date: 2026-06-17
- Related: [ADR-0011](0011-yaml-stack-files.md) (YAML stack files — the *document* format this
  builds on), [ADR-0012](0012-uniffi-bridge.md) (the typed bridge — extended here with two
  card-index methods).

## Context

HyperCatalog persists each stack as a whole YAML document in the host's internal storage
(`filesDir/stacks/<key>.yaml`, written on pause/switch via `Session::to_yaml`), and remembers the
last-used stack in a hand-rolled one-line text file (`filesDir/last_stack`). The current card
index was **not** persisted at all — reopening a stack always landed on card 1.

A HyperCard stack is a **document** (loaded whole, mutated in memory, saved whole), so file
serialization is the right mechanism — not a database, which would shred the
`Stack → Card → Button/Field` tree into tables and solve a query problem we don't have while
losing the readable YAML of ADR-0011. But the existing approach had two real seams:

1. **Non-atomic writes.** `File.writeText` truncates-then-writes. If the process is killed
   mid-write (low memory, force-stop, device shutdown), the stack file is left **truncated** — a
   corrupt YAML document that fails to load on next launch. This is data loss.
2. **Settings stored as documents.** `last_stack` is a *preference* (tiny key-value), but it was a
   bespoke text file. The missing card index is the same shape. Hand-rolling more such files (one
   per concern) is the thing the platform's preference store exists to avoid.

The card-index gap also forces a design question the previous code never had to answer:
**which layer owns "where the user was looking"?**

## Decision

Split persistence explicitly into two layers, by *what the data is*:

- **Document content → YAML files, written atomically.** The stack itself (cards, objects,
  scripts, field text) stays in `filesDir/stacks/<key>.yaml`. Saving now goes through
  `writeFileAtomically` (write a sibling `<name>.tmp`, then `rename(2)` over the target — atomic on
  the same filesystem), so a crash mid-save leaves either the old complete file or the new one,
  never a truncated half.
- **Session view state → a Preferences DataStore**, host-owned, never in the document. This holds
  the last-used stack key (`last_stack`) and **each stack's last-viewed card index**
  (`card_index/<key>`). `androidx.datastore:datastore-preferences` replaces the hand-rolled
  `last_stack` text file (migrated on first run) and adds the card index.

**The card index is view state, not document content** (Option B over "add `current_card` to the
`Stack` model"). Rationale:

- HyperCard itself treated "current card" as transient session state, not stack content — a shared
  stack opened to its first card. Storing the cursor in the portable YAML would leak one user's
  position into a shared/exported stack.
- It keeps the document format (ADR-0011) pure and cursor-free, and keeps the split clean: the
  *viewer* owns where you were looking; the *document* owns what's on the cards.
- The trade-off: a non-Android host (`hyper-desktop`) won't share this state. Acceptable — view
  position is inherently host-specific, and the desktop harness drives card navigation explicitly.

### Bridge support (ADR-0012)

Two thin methods were added to `HyperStack`, both delegating to existing `Session` logic:

- `current_card_index() -> i32` — read the position to save (wraps `Session::card_index`).
- `open_card_at(index: i32) -> DispatchResult` — restore it on load, clamped to range and firing
  the card's `openCard` (wraps `Session::goto_card`; negatives → 0, overflow → last card).

The host reads the index in `saveCurrentStack` and restores it in `loadStackKey`
(`openCardAt(prefs.cardIndex(key))`), replacing the unconditional `openCard()`.

### Mechanics

- `writeFileAtomically(target, text)` and `cardIndexPrefKey(stackKey)` live in the framework-free
  `HostLogic.kt`, unit-tested on the JVM (`HostLogicTest`) — `java.io.File` rename semantics and
  the key namespacing are covered without a device.
- `StackPrefs` wraps the DataStore, isolating the `suspend`/Flow API behind plain accessors. The
  few startup/teardown calls run under `runBlocking` (tiny local reads/writes, equivalent to the
  file IO they replace), so view state is durable by the time `onPause`/switch returns.

## Consequences

- **Positive:** stack saves are crash-safe (no more truncation data loss). Card position survives
  relaunch and stack switching, per stack. Preferences move to the platform-standard store, and the
  document format stays cursor-free and shareable.
- **Positive:** the bridge change is two small read/navigate methods over existing, already-tested
  `Session` logic; no model change, no new format.
- **Negative / caveats:** a new dependency (`datastore-preferences` + its coroutines transitive),
  and a small `runBlocking`-on-main usage at startup/pause (bounded, local, no worse than the
  synchronous file IO it sits beside). Two persistence mechanisms now coexist by design — files for
  documents, DataStore for session state — documented in `CLAUDE.md`.
- **Migration:** the legacy `filesDir/last_stack` text file is read once into the DataStore and
  deleted; legacy `stack.json` migration (ADR pre-0011) is unchanged.

## Non-goals

- **Not** moving stack documents into a database or DataStore — they remain whole YAML files.
- **Not** persisting card index in the stack document (deliberately host-side view state).
- **Not** full async/coroutine restructuring of `MainActivity` — only the persistence calls touch
  DataStore, behind `StackPrefs`.
