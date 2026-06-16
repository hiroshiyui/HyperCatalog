---
name: commit-and-push
description: Stage, commit, and (if a remote exists) push HyperCatalog changes with a well-formed commit message.
---

When committing changes, follow these steps:

1. **Verify the tree state.** Confirm generated artifacts are not staged: `rust/target/` and
   `app/src/main/jniLibs/` are gitignored (the `.so` is rebuilt from `rust/` on every Gradle
   build), as are `/build`, `.gradle`, and `local.properties`. If a Rust source change is part of
   the commit, do **not** also commit the regenerated `.so`.

2. **Stage deliberately** with `git add` — only files related to the current topic. Avoid blind
   `git add -A` when unrelated changes are present. Rust and Kotlin changes that form one logical
   change (e.g. a new bridge call touching `session.rs`, `android.rs`, `NativeBridge.kt`,
   `CardView.kt`) belong together.

3. **Commit** with a clear message following [Conventional Commits](https://www.conventionalcommits.org/),
   scoped to the area touched — e.g. `feat(hypertalk): support 'the long name of'`,
   `fix(ffi): guard null handle in nativeRender`, `fix(android): commit field edit before tap`,
   `chore(gradle): bump compileSdk to 37`. Explain *why*, not just *what*.

4. **Push** to the current branch's remote **only if one is configured** (`git remote`). This
   repository may be local-only; if there is no remote, stop after committing and say so rather
   than failing on `git push`.

5. **Verify**: after a commit, `git status` is clean; after a push, the remote is in sync with the
   local branch.
