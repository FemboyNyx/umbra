# Umbra packaging

Shared packaging assets and ready-to-apply identity snippets for the Flutter app.
**Do not edit `app/` from this tree** — Copilot owns the client. CI (Cali) should call the scripts documented below.

## App identity

| Field | Value |
|-------|-------|
| Application ID | `io.github.femboynyx.umbra` |
| Display name | `Umbra` |
| Publisher (MSIX / store-facing) | `FemboyNyx` |

Icons under `packaging/icons/` are **placeholders** (crescent / shadow motif). Replace before a branded release.

## Layout

```
packaging/
  README.md                 # this file
  icons/                    # SVG + PNG + mipmaps + umbra.ico
  android/                  # signing snippets, permissions notes
  linux/                    # .desktop, AppStream, AppImage + tarball scripts
  windows/                  # zip script, MSIX notes
```

---

## Snippets for Copilot (`app/`)

### Android — `applicationId` / namespace / label

`app/android/app/build.gradle` (Groovy):

```groovy
android {
    namespace "io.github.femboynyx.umbra"
    defaultConfig {
        applicationId "io.github.femboynyx.umbra"
    }
}
```

`app/android/app/build.gradle.kts` (Kotlin DSL):

```kotlin
android {
    namespace = "io.github.femboynyx.umbra"
    defaultConfig {
        applicationId = "io.github.femboynyx.umbra"
    }
}
```

`AndroidManifest.xml` `<application>`:

```xml
android:label="Umbra"
android:icon="@mipmap/ic_launcher"
```

Full signing + permissions: see [`android/README.md`](android/README.md).

### Windows — window title + MSIX

`app/windows/runner/main.cpp` (window title string):

```cpp
if (!window.Create(L"Umbra", origin, size)) {
```

`app/windows/runner/Runner.rc` — `FileDescription` / `ProductName` / `InternalName` values should say `Umbra`.

`app/pubspec.yaml` MSIX block (requires `msix` pub package):

```yaml
msix_config:
  display_name: Umbra
  publisher_display_name: FemboyNyx
  identity_name: io.github.femboynyx.umbra
  msix_version: 0.1.0.0
  logo_path: ../packaging/icons/umbra-256.png
  capabilities: internetClient, microphone, webcam
```

MSIX signing needs `WINDOWS_CERT_BASE64` + `WINDOWS_CERT_PASSWORD`. Without them, **zip only** — see [`windows/README.md`](windows/README.md).

### Linux — GTK application id

`app/linux/CMakeLists.txt`:

```cmake
set(APPLICATION_ID "io.github.femboynyx.umbra")
```

Desktop + AppStream files live in `packaging/linux/` for installers/AppImage; copy or install them from CI as needed.

---

## CI script interfaces (for Cali)

All scripts take the **Flutter build output path** as the first argument so workflows stay decoupled from packaging.

### Linux tarball

```bash
packaging/linux/build-tarball.sh <flutter-bundle-dir> [output-tar.gz]
```

- Typical `<flutter-bundle-dir>`: `app/build/linux/x64/release/bundle`
- Env: `UMBRA_VERSION` (default `0.0.0-dev`), `DEST_DIR` (default `.`)
- Default output: `${DEST_DIR}/umbra-${UMBRA_VERSION}-linux-x64.tar.gz`

### Linux AppImage

```bash
packaging/linux/build-appimage.sh <flutter-bundle-dir> [output.AppImage]
```

- Same bundle path as above
- Requires `appimagetool` on `PATH` (or `APPIMAGETOOL=/path/to/appimagetool`)
- Env: `UMBRA_VERSION`, `DEST_DIR`, `ARCH` (default `x86_64`)
- Default output: `${DEST_DIR}/Umbra-${UMBRA_VERSION}-${ARCH}.AppImage`

### Windows zip

```powershell
packaging/windows/build-zip.ps1 -BundleDir <release-dir> [-OutFile path.zip] [-Version 0.1.0]
```

- Typical `-BundleDir`: `app/build/windows/x64/runner/Release`
- Env fallback: `$env:UMBRA_VERSION`
- Default output: `umbra-<Version>-windows-x64.zip`

### Android

No wrapper script here — CI decodes the keystore secrets, writes `app/android/key.properties`, and runs `flutter build apk` (see [`android/README.md`](android/README.md)). Use packaging icons under `packaging/icons/mipmap-*-ic_launcher.png` when wiring launcher assets.

---

## Icons

| File | Use |
|------|-----|
| `icons/umbra.svg` | Source (placeholder) |
| `icons/umbra-{16,32,48,64,128,256,512}.png` | Generic / Linux |
| `icons/mipmap-*-ic_launcher.png` | Android densities |
| `icons/umbra.ico` | Windows (multi-size) |

Regenerate from SVG:

```bash
for s in 16 32 48 64 128 256 512; do
  rsvg-convert -w $s -h $s packaging/icons/umbra.svg -o packaging/icons/umbra-$s.png
done
convert packaging/icons/umbra-{16,32,48,64,128,256}.png packaging/icons/umbra.ico
```
