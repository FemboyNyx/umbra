# CI and releases

Guide to Umbra's GitHub Actions workflows, secrets, and local packaging.
Workflow YAML lives under `.github/workflows/` (owned by Cali). Packaging
scripts and identity snippets live under [`packaging/`](../packaging/README.md)
(owned by packaging).

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
| `rust-core` | `has_core == true` | `cargo fmt --check`, `clippy -D warnings`, `cargo test --all-features` in `core/` (stable toolchain + Swatinem/rust-cache) | — |
| `flutter-analyze` | `has_app == true` | `flutter pub get`, `analyze`, `test` in `app/` | — |
| `build-android` | `has_app == true` | Java 17, configure signing from secrets (or debug fallback), `flutter build apk`, upload APK(s) | `umbra-android-apk` |
| `build-windows` | `has_app == true` | `flutter build windows --release`, zip `build/windows/x64/runner/Release/*` | `umbra-windows-zip` (`umbra-windows.zip`) |
| `build-linux` | `has_app == true` | Install GTK/clang/ninja deps, `flutter build linux --release`, `tar` the bundle | `umbra-linux-targz` (`umbra-linux.tar.gz`) |
| `ci-gate` | always | Fails the workflow if any non-skipped package job failed; skipped jobs (missing `core/` or `app/`) are OK | — |

**Skip-when-missing behavior**

A dedicated `detect` job checks for `core/Cargo.toml` and `app/pubspec.yaml`.
Downstream jobs use `if: needs.detect.outputs.has_core == 'true'` (or
`has_app`). Until those trees land, Rust/Flutter jobs are **skipped**, not
failed, and `ci-gate` still passes.

**Pending / not in CI yet**

- No AppImage job (release staging already accepts `*.AppImage` if present).
- No MSIX build; Windows ships as zip only.
- No explicit Android NDK / `flutter_rust_bridge` (cargokit) setup — may be
  required once `core/` is wired into the Flutter build.
- Workflows do **not** call `packaging/linux/build-*.sh` or
  `packaging/windows/build-zip.ps1` (they inline `tar` / `Compress-Archive`).
  See [Packaging scripts](#packaging-scripts-local--ci) below.

---

### `release.yml` — tagged releases

**Triggers**

- `push` of tags matching `v*` (e.g. `v0.1.0`)

**Permissions:** `contents: write` (create/update the GitHub Release).

**Jobs**

| Job | When | What |
|-----|------|------|
| `detect` | always | Same `has_core` / `has_app` detection as CI |
| `rust-core` | `has_core` | `cargo test --all-features` (no fmt/clippy on the release path) |
| `build-android` | `has_app` | Same signing + APK build as CI; artifact `umbra-android-apk` |
| `build-windows` | `has_app` | Same zip as CI; artifact `umbra-windows-zip` |
| `build-linux` | `has_app` | Same tar.gz as CI; artifact `umbra-linux-targz` |
| `publish` | after builds (always, if detect succeeded) | Download artifacts, flatten into `release-assets/`, write `SHA256SUMS`, create GitHub Release via `softprops/action-gh-release@v2` |

**Release assets**

Copied into the release when present:

- `*.apk`
- `*.zip`
- `*.tar.gz`
- `*.AppImage` (accepted if produced; **not built** by the workflow today)
- `*.msix` (accepted if produced; **not built** by the workflow today)
- `SHA256SUMS` (always written over whatever landed in `release-assets/`)

If `app/` is missing and no binaries were uploaded, the publish job attaches a
placeholder `README-release.txt` plus `SHA256SUMS` so the tag still gets a
notes-only release.

**Pending on the release path**

- No AppImage (`appimagetool`) step.
- No MSIX step / no use of `WINDOWS_CERT_*` secrets in YAML.
- Packaging helper scripts under `packaging/` are not invoked.

---

## Cutting a release

1. Ensure `main` (or the commit you intend to ship) is green on CI once `app/`
   / `core/` exist.
2. Create an annotated or lightweight tag matching `v*`:

   ```bash
   git tag -a v0.1.0 -m "Umbra v0.1.0"
   git push origin v0.1.0
   ```

3. The **Release** workflow runs on that tag push.
4. When builds succeed, GitHub creates **Umbra vX.Y.Z** with generated notes
   and attaches whatever platform artifacts were produced, plus **SHA256SUMS**.
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

### Optional (documented for packaging / future MSIX)

| Secret | Purpose |
|--------|---------|
| `WINDOWS_CERT_BASE64` | Base64-encoded `.pfx` for MSIX code signing |
| `WINDOWS_CERT_PASSWORD` | PFX password |

These are **not referenced** by the current workflow YAML. When MSIX is wired
up, missing cert secrets should keep Windows on **zip only** (see
[`packaging/windows/README.md`](../packaging/windows/README.md)).

`GITHUB_TOKEN` is provided automatically for `softprops/action-gh-release`.

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

**Windows cert secrets**

- Unused by current YAML. Packaging docs: without `WINDOWS_CERT_*`, ship zip
  only (no MSIX).

---

## Packaging scripts (local / CI)

Identity, icons, desktop/AppStream metadata, and helper scripts:

→ **[`packaging/README.md`](../packaging/README.md)**

Local examples (after a Flutter release build):

```bash
# Linux tarball (adds .desktop / metainfo / icon into the archive layout)
packaging/linux/build-tarball.sh app/build/linux/x64/release/bundle

# Linux AppImage (requires appimagetool on PATH)
packaging/linux/build-appimage.sh app/build/linux/x64/release/bundle

# Windows zip (PowerShell)
packaging/windows/build-zip.ps1 -BundleDir app/build/windows/x64/runner/Release
```

### CI vs packaging scripts (current mismatch)

As of the workflows in this repo:

| Artifact | Workflow behavior today | Packaging helper |
|----------|-------------------------|------------------|
| Linux tar.gz | Inline `tar -C …/bundle -czf umbra-linux.tar.gz .` | `packaging/linux/build-tarball.sh` (not called) |
| Linux AppImage | Not built | `packaging/linux/build-appimage.sh` |
| Windows zip | Inline `Compress-Archive` → `umbra-windows.zip` | `packaging/windows/build-zip.ps1` (not called) |
| Windows MSIX | Not built | Notes + `msix` pubspec snippet under `packaging/windows/` |

Prefer calling the packaging scripts from CI when Cali next touches the
workflows so release layouts stay consistent with local builds (desktop file,
AppStream metainfo, versioned filenames, AppImage).

---

## Troubleshooting

| Symptom | Likely cause | What to try |
|---------|--------------|-------------|
| All Flutter/Rust jobs skipped, gate green | `app/pubspec.yaml` or `core/Cargo.toml` not on the branch yet | Expected early on; land those trees or open the PR that adds them |
| `ci-gate` / release publish fails | A build job `failed` or `cancelled` (not `skipped`) | Open the failed job log; fix compile/test errors |
| Android APK is debug / won't install as update over release | Signing secrets missing or `key.properties` not consumed by Gradle | Set all four `ANDROID_*` secrets; confirm Copilot applied the signing snippet |
| `key.properties` / `.jks` shows up in `git status` | Accidental local signing files | They are gitignored; delete or move them out of the tree — never commit |
| Linux Flutter build fails on missing GTK / clang | Dep install step failed or ran on wrong OS | Job must be `ubuntu-latest` with the `apt-get` deps from `ci.yml` |
| Windows zip empty / upload error | `flutter build windows` failed or path drifted | Confirm `app/build/windows/x64/runner/Release` exists on the runner |
| Tag push did nothing | Tag does not match `v*` | Use `v0.1.0`, not `0.1.0` |
| Release has only `README-release.txt` | No successful app artifacts (e.g. `app/` missing) | Expected until the client builds; re-tag after `app/` lands |
| Want AppImage / MSIX on the release | Not implemented in workflow YAML yet | Build locally via packaging scripts, or extend `release.yml` (Cali) |
| Checksum mismatch after download | Partial download or mirrored corrupt file | Re-download; compare to `SHA256SUMS` on the Release page |

For app identity (`io.github.femboynyx.umbra`), icons, and platform snippets, see
[`packaging/README.md`](../packaging/README.md). For optional Tor routing design,
see [`docs/tor.md`](tor.md) when present.
