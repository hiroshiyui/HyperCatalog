# ADR-0011 — YAML as the authoring format for stack files

- Status: **Accepted** — decision recorded; not yet implemented.
- Date: 2026-06-17
- Related: [ADR-0002](0002-json-string-jni-bridge.md) (the JSON bridge — explicitly *not* changed
  here), [ADR-0003](0003-player-first-json-authored-stacks.md) (JSON-authored stacks, which this
  revises), and [ADR-0008](0008-native-view-rendering.md) (where any *bridge*-format change belongs).

## Context

Stacks are persisted as serde-serialized JSON. JSON is fine for machines, but it reads and diffs
badly for the most important hand-authored content in a stack — **multi-line HyperTalk scripts and
field paragraphs**, which become escape soup:

```json
"script": "on mouseUp\n  add 1 to field \"counter\"\n  if it > 10 then beep\nend mouseUp"
```

A script's structure is invisible behind `\n` escapes and quoting, which makes authoring sample
stacks and reviewing diffs painful.

Two facts make this cheap to address:
- The document model (`model.rs`) is **plain serde structs**, so the *on-disk format* is
  independent of the in-memory model and of the JNI bridge.
- The stack file is read by the **host as a string** and handed to the core (`nativeLoad` →
  `Session::load_from_json`); only the core parses it. So a new file format is a Rust-side change.

The companion question — "should the *bridge* move off JSON too?" — is deliberately **out of
scope here** (see Non-goals). Readability is a human concern; the bridge is machine-to-machine.

## Decision

Adopt **YAML as the authoring/source format for stack files**, processed in `hypercore` with the
**`yaml_serde`** crate (the [`yaml/yaml-serde`](https://github.com/yaml/yaml-serde) project — the
maintained successor to the now-deprecated `serde_yaml`, exposing the same serde
`from_str`/`to_string` API and aliasable as `serde_yaml`). The existing model structs are **reused
unchanged**; only the (de)serialization entry points gain a YAML path. The same stack in YAML:

```yaml
buttons:
  - name: Inc
    rect: { x: 10, y: 100, w: 80, h: 40 }
    script: |
      on mouseUp
        add 1 to field "counter"
        if it > 10 then beep
      end mouseUp
```

Policy for what uses which format:

- **Authoring / source (bundled assets, hand-written stacks): YAML.** This is where readability
  matters; block scalars (`|`) render scripts and paragraphs as real indented text.
- **The JNI bridge: JSON, unchanged.** ADR-0002's host contract (serde ↔ `android.rs` ↔ `org.json`)
  is untouched — RenderList/DispatchResult/effects stay JSON. YAML never crosses the bridge.
- **Runtime persistence (saved working copies in `filesDir`): JSON, unchanged.** These are
  machine-written and never hand-read, so `to_json` and the per-stack save path (ADR's picker work)
  stay as-is. Authored YAML is the *source*; runtime saves remain JSON.

Mechanics:

- `hypercore` adds `yaml_serde` and a `Session::load_from_yaml` (keeping `load_from_json`); or a
  single entry point that **routes by the host-supplied format**. Bundled assets become `*.yaml`.
- **Route by file extension** (`.yaml`/`.yml` vs `.json`), not "parse everything as YAML." JSON is
  a subset of YAML, but typed deserialization plus explicit routing avoids YAML coercion footguns
  (the "Norway problem", unquoted-string surprises) on existing JSON inputs.
- The Android host changes minimally: it already reads the asset/file as a string; it just needs to
  recognize `.yaml` assets in the stack picker and pass the format hint. The bridge is not touched.

### Choices and their reasons

- **YAML over TOML or Markdown.** YAML's block scalars directly solve the multi-line script/
  paragraph pain, and a nested `cards → buttons/fields` tree maps onto YAML naturally while reusing
  the serde model (near-zero model change). TOML is awkward for this array-of-tables hierarchy;
  Markdown is not a serialization format — it would be a bespoke authoring *language* (a parser/
  emitter to build and maintain), which is a separate, larger initiative, not a format swap.
- **`yaml_serde` specifically.** `serde_yaml` (dtolnay) is archived/unmaintained; `yaml_serde` is
  the maintained successor with a drop-in serde API and an alias-as-`serde_yaml` migration path —
  the lowest-friction adoption. Pin and re-verify the exact version/maintenance at implementation.
- **JSON kept for bridge and runtime saves.** Changing the wire/save format would add cost without
  serving readability (nothing there is hand-read). It also keeps this change small and reversible.
- **Authoring-only (load), not full round-trip (save).** YAML serializers don't guarantee
  block-scalar *emission*, so machine-written YAML can be uglier than hand-written; keeping saves in
  JSON sidesteps that. Full YAML round-trip is a possible later extension, not a requirement.

## Consequences

- **Positive:** hand-authored stacks and their git diffs become readable — scripts as literal
  blocks, far less punctuation. The model, the bridge, the host effects, and the existing tests are
  unchanged; YAML is purely an added deserialization path. Backward compatible — existing JSON
  stacks (and JSON working copies) still load.
- **Positive:** revises ADR-0003's "JSON-authored" stance toward readable authoring without
  disturbing ADR-0002. The in-app editors operate on the model, not the raw file, so they are
  unaffected.
- **Negative / caveats:** a new dependency (`yaml_serde`) and a modest `.so` size increase; YAML's
  indentation sensitivity and type-coercion footguns (mitigated by typed deserialization and
  extension-based routing). Two on-disk formats now coexist — YAML for authoring, JSON for runtime
  saves and the bridge — a small conceptual cost to document in `CLAUDE.md`.
- **Migration:** bundled assets can be mechanically converted to `*.yaml`; JSON loading is retained
  for compatibility and for runtime working copies. A round-trip test (YAML → model → JSON →
  model) should assert both formats yield the same `Stack`.
- **Explicitly deferred:** the broader "move the *bridge* off JSON" question (typeshare-generated
  Kotlin types, protobuf, or UniFFI, plus a possible async channel) remains a separate decision
  tied to [ADR-0008](0008-native-view-rendering.md)'s native-view rendering work — not part of this.

## Non-goals

- **Not** changing the JNI bridge format (ADR-0002 stands).
- **Not** a Markdown/DSL authoring language (possible future, separate ADR).
- **Not** full YAML round-trip persistence (saves stay JSON for now).
