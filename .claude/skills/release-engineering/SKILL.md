---
name: release-engineering
description: Manage the HyperCatalog release process — version bumps (Cargo + Gradle), changelog, build/test verification, Git tags, and (if a remote exists) GitHub releases.
---

When performing release engineering, follow these steps:

1. **Determine the release type** — review all commits since the last tag and classify the release
   as `major`, `minor`, or `patch` per [Semantic Versioning](https://semver.org/). Present the
   recommendation and confirm with the user before proceeding.

2. **Verify the build is green** — do not continue if anything fails:
   ```bash
   cd rust && cargo test -p hypercore && cargo clippy --workspace --all-targets && cargo fmt --all --check
   cd .. && ./gradlew :app:assembleRelease
   ```
   `assembleRelease` runs the `cargoNdkBuild` cross-compile and packages the APK.

3. **Bump the version in both places** (keep them consistent):
   - `rust/Cargo.toml` → `[workspace.package] version` (applies to all three crates).
   - `app/build.gradle.kts` → `versionName` (the marketing version) and increment `versionCode`
     (a monotonic integer; required for any distributed Android build).

4. **Update `CHANGELOG.md`** (create it if absent) — add a new entry at the top following
   [Keep a Changelog](https://keepachangelog.com/), grouping notable changes under `Added`,
   `Changed`, `Fixed`, `Removed`, or `Security` since the previous release.

5. **Commit the release** — stage `rust/Cargo.toml`, `app/build.gradle.kts`, and `CHANGELOG.md`
   together: `chore: release vX.Y.Z`.

6. **Tag** — annotated tag `git tag -a vX.Y.Z -m "vX.Y.Z"`. Push the tag only if a remote is
   configured (`git remote`); this repo may be local-only.

7. **GitHub release** — only if a GitHub remote exists: `gh release create vX.Y.Z` with that
   version's `CHANGELOG.md` section as the body, attaching the release APK
   (`app/build/outputs/apk/release/app-release*.apk`) if a signed/usable artifact is produced.
   If there is no remote, stop after tagging and report that the release is local-only.
