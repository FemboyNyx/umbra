# CI and releases

Guide to Umbra's GitHub Actions workflows, secrets, and local packaging.
Workflow YAML lives under `.github/workflows/` (owned by Cali). Packaging
scripts and identity snippets live under [`packaging/`](../packaging/README.md).

> Never commit real keystores, PFX files, or secret values. Examples below use
> placeholders only.

## Workflows overview

### `ci.yml` — continuous integration

**Triggers**

- `push` to `main` or `backup`
- `pull_request` targeting `main`

Concurrency group `ci-${{ github.workflow }}-${{ github.ref }}` cancels older
runs on the same ref.

**Jobs**

| Job | When it runs | What it does | Artifacts |
|-----|--------------|--------------|-----------|
| `detect` | always | Sets `has_core` / `has_app` from `core/Cargo.toml` and `app/pubspec.yaml` | — |
| `rust-core` | `has_core == true` | Rust from `core/rust-toolchain.toml` via `rustup toolchain install`, then `cargo fmt --check`, `cargo clippy --locked …`, `cargo test --locked --all-features` in `core/` (+ Swatinem/rust-cache) | — |
| `flutter-analyze` | `has_app == true` | `flutter pub get`, enforce `flutter_rust_bridge` `=2.13.0` when present in pubspec, then `analyze` / `test` | — |
| `build-android` | `has_app == true` | Java 17, NDK r26d, Android targets from `core/rust-toolchain.toml`, `perl`+`make` (SQLCipher/OpenSSL), signing or debug fallback, `flutter build apk` | `umbra-android-apk` |
| `build-windows` | `has_app == true` | `flutter build windows --release`, then `packaging/windows/build-zip.ps1`; optional MSIX when cert secrets + `msix` in pubspec | `umbra-windows` (`dist/*.zip`, optional `dist/*.msix`) |
| `build-linux` | `has_app == true` | GTK/clang/ninja + fuse deps, install `appimagetool`, `flutter build linux --release`, then `packaging/linux/build-tarball.sh` and `build-appimage.sh` | `umbra-linux` (`dist/*.tar.gz`, `dist/*.AppImage`) |
| `ci-gate` | always | Fails the workflow if any non-skipped package job failed; skipped jobs (missing `core/` or `app/`) are OK | — |

**Skip-when-missing behavior**

A dedicated `detect` job checks for `core/Cargo.toml` and `app/pubspec.yaml`.
Downstream jobs use `if: needs.detect.outputs.has_core == 'true'` (or
`has_app`). Until those trees land, Rust/Flutter jobs are **skipped**, not
failed, and `ci-gate` still passes.

**Version strings in CI artifacts**

- Linux / Windows packaging uses the short git SHA (`github.sha` truncated to 7
  chars) as `UMBRA_VERSION` / `-Version`, e.g. `umbra-abc1234-linux-x64.tar.gz`,
  `Umbra-abc1234-x86_64.AppImage`, `umbra-abc1234-windows-x64.zip` (CI writes the
  zip as `dist/umbra-windows-x64.zip` but passes the short SHA as `-Version`).

**Rust toolchain (`core/rust-toolchain.toml` only)**

The channel, components (`clippy`, `rustfmt`), and Android targets are pinned
**only** in [`core/rust-toolchain.toml`](../core/rust-toolchain.toml) (currently
**1.99.1**). When `core/` exists, CI/Release run `rustup toolchain install` from
`core/` — rustup 1.28+ no longer auto-installs on `rustup show`. Do not hardcode
the version in workflow YAML. Always use rustup-managed `rustc`, never the
system compiler.


**Cargo workspace and `--locked`**

`core/` is a Cargo **workspace**. `core/Cargo.lock` is **committed**. The
`rust-core` job (and Release's rust tests) run cargo with **`--locked`**
(`cargo clippy --locked …`, `cargo test --locked …`), so the lockfile must
match the manifests. After `cargo update` (or any dependency change that
rewrites the lockfile), **commit `Cargo.lock`** or CI will fail.

**Android NDK, Rust targets, and SQLCipher deps (FRB / cargokit)**

Both `ci.yml` and `release.yml` prepare `build-android` the same way (after
Java 17, before Flutter):

1. **NDK r26d** — always, via `nttld/setup-ndk@v1` with `ndk-version: r26d` and
   `link-to-sdk: true`. A follow-up step exports `ANDROID_NDK_HOME` and
   `ANDROID_NDK_ROOT` from `steps.setup-ndk.outputs.ndk-path` into `$GITHUB_ENV`
   so cargokit can find the NDK.
2. **Rust for Android** — only when `has_core == true`:
   - `rustup toolchain install` in `core/` (picks channel/components/targets from `rust-toolchain.toml`)
   - `Swatinem/rust-cache@v2` with `workspaces: core`
   - `rustup target add` for `aarch64-linux-android`, `armv7-linux-androideabi`,
     `x86_64-linux-android`, and `i686-linux-android`
3. **`perl` and `make`** — always installed (`sudo apt-get install -y perl make`).
   SQLCipher builds OpenSSL as part of the native stack and needs both.

The host `rust-core` job uses the same `rustup toolchain install` path; `clippy`/`rustfmt` come from the toml `components` list.

**flutter_rust_bridge pin (=2.13.0)**

Workflows set `FRB_VERSION: "2.13.0"`. In `ci.yml`, the `flutter-analyze` job
runs a **Check flutter_rust_bridge pin (=2.13.0)** step after `flutter pub get`:

- If `flutter_rust_bridge` is **absent** from `app/pubspec.yaml`, the check
  skips cleanly.
- If it is present, the line must match `flutter_rust_bridge:` with version
  `=2.13.0` (optional `=` before the version). Anything else fails the job.

Keep the Dart package, codegen, and Rust FRB crates on that exact version
(Elite's pin).

**Pending / not wired yet**

- MSIX still needs Copilot to add the `msix` pub dependency + `msix_config` in
  `app/pubspec.yaml`; without that, Windows stays zip-only even if cert secrets
  exist.

---

### `release.yml` — tagged releases

**Triggers**

- `push` of tags matching `v*` (e.g. `v0.1.0`)

**Permissions:** `contents: write` (create/update the GitHub Release).

**Jobs**

| Job | When | What |
|-----|------|------|
| `detect` | always | Same `has_core` / `has_app` detection as CI |
| `rust-core` | `has_core` | Rust from `core/rust-toolchain.toml`, then `cargo test --locked --all-features` (no fmt/clippy on the release path) |
| `build-android` | `has_app` | Same NDK / rust-toolchain.toml targets / perl+make / signing + APK build as CI; artifact `umbra-android-apk` |
| `build-windows` | `has_app` | Same `build-zip.ps1` + optional MSIX as CI; version from tag (`v0.1.0` → `0.1.0`); artifact `umbra-windows` |
| `build-linux` | `has_app` | Same `appimagetool` + `build-tarball.sh` / `build-appimage.sh` as CI; version from tag with leading `v` stripped; artifact `umbra-linux` |
| `publish` | after builds (always, if detect succeeded) | Download artifacts, flatten into `release-assets/`, write `SHA256SUMS`, create GitHub Release via `softprops/action-gh-release@v2` |

**Release assets**

Copied into the release when present:

- `*.apk`
- `*.zip`
- `*.tar.gz`
- `*.AppImage`
- `*.msix` (only if optional MSIX step produced one)
- `SHA256SUMS` (checksums of everything in `release-assets/`)

Typical filenames for tag `v0.1.0`:

- `umbra-0.1.0-linux-x64.tar.gz`
- `Umbra-0.1.0-x86_64.AppImage`
- `umbra-0.1.0-windows-x64.zip`
- Flutter APK name(s) under `app/build/app/outputs/flutter-apk/`
- optional `*.msix`

If `app/` is missing and no binaries were uploaded, the publish job attaches a
placeholder `README-release.txt` plus `SHA256SUMS` so the tag still gets a
notes-only release.

---

## Cutting a release

1. Ensure `main` (or the commit you intend to ship) is green on CI once `app/`
   / `core/` exist.
2. Create and push a tag matching `v*`:

   ```bash
   git tag -a v0.1.0 -m "Umbra v0.1.0"
   git push origin v0.1.0
   ```

3. The **Release** workflow runs on that tag push.
4. When builds succeed, GitHub creates **Umbra vX.Y.Z** with generated notes
   and attaches platform artifacts plus **SHA256SUMS**.
5. Verify checksums locally:

   ```bash
   sha256sum -c SHA256SUMS
   ```

Do not attach secrets or keystores to the release.

---

## GitHub secrets

Add secrets under the repo: **Settings → Secrets and variables → Actions →
New repository secret**.

### Required for signed Android release APKs

| Secret | Purpose |
|--------|---------|
| `ANDROID_KEYSTORE_BASE64` | Base64 encoding of the `.jks` / `.keystore` file |
| `ANDROID_KEYSTORE_PASSWORD` | Keystore password |
| `ANDROID_KEY_ALIAS` | Key alias inside the store |
| `ANDROID_KEY_PASSWORD` | Key password |

Both `ci.yml` and `release.yml` decode the keystore to
`app/android/upload-keystore.jks` and write `app/android/key.properties` when
`ANDROID_KEYSTORE_BASE64` is non-empty. Those files must stay gitignored
(`key.properties`, `*.jks`, `*.keystore`).

### Optional — Windows MSIX

| Secret | Purpose |
|--------|---------|
| `WINDOWS_CERT_BASE64` | Base64-encoded `.pfx` for MSIX code signing |
| `WINDOWS_CERT_PASSWORD` | PFX password |

Used by the **Optional MSIX** step in CI and Release. The step exits cleanly
(zip only) when either secret is missing **or** `app/pubspec.yaml` has no
`msix:` dependency line. When both secrets and the pub config are present, the
workflow writes a PFX under `$RUNNER_TEMP`, sets `MSIX_CERTIFICATE_PATH` /
`MSIX_CERTIFICATE_PASSWORD`, runs `flutter pub run msix:create`, and copies
`*.msix` into `dist/`.

`*.pfx` is gitignored. `GITHUB_TOKEN` is provided automatically for
`softprops/action-gh-release`.

---

## Generate an Android keystore

Run locally (once). Store the outputs in a password manager; put only the
base64 blob and passwords into GitHub secrets.

```bash
keytool -genkey -v \
  -keystore umbra-release.jks \
  -keyalg RSA -keysize 2048 -validity 10000 \
  -alias umbra

# Single-line base64 for ANDROID_KEYSTORE_BASE64 (GNU coreutils):
base64 -w0 umbra-release.jks > umbra-release.jks.b64

# macOS:
# base64 -i umbra-release.jks -o umbra-release.jks.b64
```

Then create the four `ANDROID_*` secrets from the `.b64` file contents and the
passwords/alias you chose. Delete local copies of the keystore when you no
longer need them on disk.

Gradle must read `key.properties` the way Copilot wires
`app/android/app/build.gradle(.kts)`. Ready-to-apply snippets:
[`packaging/android/`](../packaging/android/).

---

## Fallback when secrets are missing

**Android (CI and Release)**

- If `ANDROID_KEYSTORE_BASE64` is unset/empty, the workflow logs that secrets
  are missing and **does not** write `key.properties`.
- The build step then runs `flutter build apk --debug` instead of `--release`.
- Artifacts are still uploaded. Treat debug-signed APKs as **non-official**;
  do not distribute them as production builds.

**Windows MSIX**

- Missing `WINDOWS_CERT_BASE64` or `WINDOWS_CERT_PASSWORD` → log and ship
  **zip only** (job still succeeds).
- Secrets present but no `msix:` entry in `app/pubspec.yaml` → same zip-only
  skip.
- Zip packaging always runs via `packaging/windows/build-zip.ps1`.

---

## Packaging scripts (local and CI)

Identity, icons, desktop/AppStream metadata, and helper scripts:

→ **[`packaging/README.md`](../packaging/README.md)**

CI and Release call these after Flutter builds:

| Script | Invoked as |
|--------|------------|
| `packaging/linux/build-tarball.sh` | `… "$BUNDLE" "dist/umbra-${VER}-linux-x64.tar.gz"` |
| `packaging/linux/build-appimage.sh` | `… "$BUNDLE" "dist/Umbra-${VER}-x86_64.AppImage"` (runner installs `appimagetool` first) |
| `packaging/windows/build-zip.ps1` | `-BundleDir app/build/windows/x64/runner/Release -OutFile dist/… -Version $ver` |

Local examples (after a Flutter release build):

```bash
# Linux tarball
packaging/linux/build-tarball.sh app/build/linux/x64/release/bundle

# Linux AppImage (requires appimagetool on PATH)
packaging/linux/build-appimage.sh app/build/linux/x64/release/bundle
```

```powershell
# Windows zip
packaging/windows/build-zip.ps1 -BundleDir app/build/windows/x64/runner/Release
```

Optional env for the shell scripts: `UMBRA_VERSION`, `DEST_DIR`, `APPIMAGETOOL`,
`ARCH` (see script headers / packaging README).

---

## Troubleshooting

| Symptom | Likely cause | What to try |
|---------|--------------|-------------|
| All Flutter/Rust jobs skipped, gate green | `app/pubspec.yaml` or `core/Cargo.toml` not on the branch yet | Expected early on; land those trees or open the PR that adds them |
| `ci-gate` / release publish fails | A build job `failed` or `cancelled` (not `skipped`) | Open the failed job log; fix compile/test errors |
| Android APK is debug / won't install as update over release | Signing secrets missing or `key.properties` not consumed by Gradle | Set all four `ANDROID_*` secrets; confirm Copilot applied the signing snippet |
| `key.properties` / `.jks` shows up in `git status` | Accidental local signing files | They are gitignored; delete or move them out of the tree — never commit |
| Android native / cargokit build fails looking for NDK | `ANDROID_NDK_HOME` / `ANDROID_NDK_ROOT` not set or wrong NDK | Confirm `nttld/setup-ndk` ran with `r26d` and the export step; local builds should point those env vars at an r26d install |
| `cargo` / FRB fails with edition or vodozemac/libp2p compile errors on CI | Toolchain older than `core/rust-toolchain.toml`, or system `rustc` used instead of rustup | CI runs `rustup toolchain install` in `core/`; locally honor `core/rust-toolchain.toml` and ensure `rustc` is from rustup (`which rustc`) |
| `'toolchain' is a required input` | A `dtolnay/rust-toolchain` step with no `toolchain:` input (stale branch or partial revert) | Replace it with `rustup toolchain install` run in `core/`; the version comes from `core/rust-toolchain.toml` |
| Missing component (`rustfmt` / `clippy`) or target (e.g. `can't find crate for std` on an Android triple) | The component or target is not listed in `core/rust-toolchain.toml` | Add it to `components` / `targets` in `core/rust-toolchain.toml` (not the workflow YAML) and commit |
| No `i686-linux-android` (32-bit x86) native lib | Intentional: `core/rust-toolchain.toml` only lists `aarch64`, `armv7`, and `x86_64` Android targets | Expected; add `i686-linux-android` to the toml `targets` only if 32-bit x86 Android is ever needed |
| `cargo … --locked` fails (lock file out of date / missing) | `core/Cargo.lock` not committed or not updated after dependency edits | Run `cargo update` (or regenerate the lock) in `core/`, commit `Cargo.lock`, re-push |
| Android / cargokit build fails compiling OpenSSL / SQLCipher | Missing `perl` or `make` on the builder | CI installs both; locally `sudo apt-get install -y perl make` (or equivalent) |
| `flutter-analyze` fails on flutter_rust_bridge pin | pubspec has FRB but not `=2.13.0` | Pin `flutter_rust_bridge: =2.13.0` in `app/pubspec.yaml` (or remove the dep until ready) |
| Linux Flutter build fails on missing GTK / clang | Dep install step failed | Job must be `ubuntu-latest` with the `apt-get` deps from the workflow (including `fuse` / `libfuse2` for AppImage) |
| AppImage step fails on `appimagetool` | Download/install step failed or FUSE unavailable | Check the "Install appimagetool" log; confirm `fuse`/`libfuse2` installed |
| Windows zip empty / upload error | `flutter build windows` failed or `build-zip.ps1` path wrong | Confirm `app/build/windows/x64/runner/Release` exists; packaging scripts must be on the branch |
| No MSIX on the release | Cert secrets missing and/or no `msix` in pubspec | Expected zip-only; add `WINDOWS_CERT_*` and Copilot's `msix_config` to get MSIX |
| Tag push did nothing | Tag does not match `v*` | Use `v0.1.0`, not `0.1.0` |
| Release has only `README-release.txt` | No successful app artifacts (e.g. `app/` missing) | Expected until the client builds; re-tag after `app/` lands |
| Checksum mismatch after download | Partial download or corrupt mirror | Re-download; compare to `SHA256SUMS` on the Release page |

For app identity (`io.github.femboynyx.umbra`), icons, and platform snippets, see
[`packaging/README.md`](../packaging/README.md). For optional Tor routing design,
see [`docs/tor.md`](tor.md) when present.
