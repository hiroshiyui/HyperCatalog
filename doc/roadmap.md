# HyperCatalog Roadmap

HyperCatalog is a HyperCard-like UI player with a HyperTalk dialect implemented directly in
Rust (lexer → parser → AST → interpreter). The Rust core (`hypercore`) is platform-agnostic;
Android is the reference host, driven through a small JSON-over-JNI bridge.

This roadmap states where we are and the order we intend to grow. Decisions that shape the
architecture are recorded as ADRs under [`doc/adr/`](adr/).

## Where we are (shipped)

- **Stack player (browse mode).** Load a stack from JSON, render the current card, hit-test
  taps, run scripts along the HyperCard message path (object → card → background → stack),
  navigate between cards, edit field text through a host overlay, and persist on pause.
- **HyperTalk subset in Rust.** Handlers (`on mouseUp`/`openCard`/…), `put`/`get`/`set`,
  `go`, `answer`/`beep`, `add`/`subtract`/`multiply`/`divide`, `if`/`repeat`, full expression
  precedence, field/button/card/stack property get/set, and `length`/`random`/`trunc`. The
  supported surface is documented in `rust/README.md`.
- **JSON-over-JNI bridge.** Seven calls (`nativeLoad`, `nativeOpenCard`, `nativeRender`,
  `nativeDispatchTouch`, `nativeSetFieldText`, `nativeToJson`, `nativeFree`) exchanging JSON
  strings. See [ADR-0002](adr/0002-json-string-jni-bridge.md).
- **Sample content.** `assets/sample.json` (demo) and `assets/productivity.json` (To-Do,
  Counters, Tip Split, Calculator, Temperature, Length); the latter is the default stack.

## Direction

The arc is **player → script authoring → object authoring → broader HyperTalk**, smallest
useful step first, never breaking the player.

### Phase 1 — In-app script editing *(in progress)*

Let a user edit the HyperTalk on existing objects from the device, not just by hand-editing
JSON. An **edit-mode toggle** switches taps from "run the script" to "select & edit the
script"; a multi-line editor reads the object's current source, validates it through the
parser, and writes it back. No new object creation yet. See
[ADR-0004](adr/0004-in-app-script-editor.md).

*Enables: tweak a button's behavior on-device; immediate parse-error feedback.*

### Phase 2 — Object authoring

Create / delete / move / resize buttons and fields; a minimal tool palette; set name, title,
style, and lock state. This is the larger half of "authoring" and reuses the render-list /
host-command bridge that was designed to accommodate it without a rewrite.

*Enables: build a card from scratch on-device.*

### Phase 3 — Broader HyperTalk coverage

Fill documented interpreter gaps: geometric/text properties (`loc`/`rect`/`textStyle`), the
message box UI, custom-message dispatch up the path (today `Stmt::Send` of an unknown command
is a no-op), `visual effect`, and fuller `repeat`/`pass`/`return` semantics.

*Enables: scripts that move/restyle objects and send their own messages.*

### Phase 4 — Persistence & rendering polish

Persist the current card index (today a stack reopens at card 1). Multi-line fields with wrap
and scrolling (today single-line). A stack picker / multiple stacks instead of one default
asset.

## Non-goals (for now)

Paint tools, networking/sync, and any return to an embedded scripting VM. HyperTalk stays
Rust-native — see [ADR-0001](adr/0001-rust-native-hypertalk.md).

## Decision records

- [ADR-0001 — Rust-native HyperTalk interpreter](adr/0001-rust-native-hypertalk.md)
- [ADR-0002 — JSON-string JNI bridge](adr/0002-json-string-jni-bridge.md)
- [ADR-0003 — Player-first, JSON-authored stacks](adr/0003-player-first-json-authored-stacks.md)
- [ADR-0004 — In-app HyperTalk script editor](adr/0004-in-app-script-editor.md)
