# HyperCatalog

A small, HyperCard-inspired app for Android: flip through **cards**, tap **buttons**, type into
**fields**, and — because every object can carry a little script — make them *do* things. It's a
modern take on the classic "stack" idea, where simple interactive screens are easy to use and easy
to build.

> 🚧 **Work in progress.** HyperCatalog is under active development. Things are usable but
> evolving, screens and behavior may change, and some features are still on the way. Expect rough
> edges.

## What you can do today

- **Browse a stack.** The app opens to a *Productivity* stack with ready-to-use cards:
  - ✅ **To-Do** — a checklist with tap-to-check boxes
  - 🔢 **Counters** — tally things up and down (great for habits)
  - 💸 **Tip Split** — bill + tip % + people → tip, total, per person
  - ➗ **Calculator** — a simple two-number calculator
  - 🌡️ **Temperature** — Celsius ↔ Fahrenheit
  - 📏 **Length** — centimeters ↔ inches

  Whatever you type or tally is remembered the next time you open the app.

- **Switch stacks.** Tap **Stacks** to open another bundled stack — there's a *Welcome* demo and a
  *Gesture Demo* alongside *Productivity*. Each stack remembers its own edits.

- **Touch, not just tap.** Beyond a tap, a stack's scripts can react to **swipes**, **long-press**,
  and **double-tap** — e.g. the Gesture Demo swipes between cards and long-presses a button.

- **Make your own.** Tap **Edit** to switch into authoring mode, where you can:
  - add buttons and fields, then **drag to move** and **drag a corner to resize** them
  - set an object's name, label, style, and other properties
  - write a short script that runs when the object is tapped
  - delete what you don't need

  Tap **Done** to go back to using your cards.

## What's coming

HyperCatalog is growing toward a friendlier, more capable authoring experience and a richer set of
building blocks. The plan — and the bigger long-term vision — lives in
[`doc/roadmap.md`](doc/roadmap.md).

A few things that **aren't there yet**: multi-line/scrolling text, themed/Material-style controls,
and a full library of widgets. The app remembers your content but not which card you were last on
(it reopens at the start).

## Trying it out

This is an Android app. If you have a development setup, you can build and install the debug
version:

```sh
./gradlew :app:assembleDebug
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

It runs on a phone or an emulator.

## For the curious

Under the hood, HyperCatalog has its own little scripting language (a HyperTalk dialect) and a
platform-agnostic core. If you want the technical story, see:

- [`doc/roadmap.md`](doc/roadmap.md) — where the project is headed
- [`doc/adr/`](doc/adr/) — the design decisions behind it
- [`rust/README.md`](rust/README.md) — the scripting engine and supported commands
- [`CLAUDE.md`](CLAUDE.md) — architecture notes for contributors

---

*HyperCatalog is an early-stage, in-progress project. Feedback and patience both welcome.* 🙂
