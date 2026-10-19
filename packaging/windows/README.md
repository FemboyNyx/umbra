# Windows packaging for Umbra

App ID / identity: `io.github.femboynyx.umbra`
Display name: `Umbra`

Copilot owns `app/`. Apply identity snippets there; CI (Cali) should call `build-zip.ps1`.

## Zip (default release artifact)

After `flutter build windows --release`, the runner output is:

```
app/build/windows/x64/runner/Release/
```

Package it:

```powershell
packaging/windows/build-zip.ps1 -BundleDir app/build/windows/x64/runner/Release -OutFile umbra-0.1.0-windows-x64.zip
```

Parameters:

| Param | Required | Description |
|-------|----------|-------------|
| `-BundleDir` | yes | Path to Flutter Windows Release folder |
| `-OutFile` | no | Output zip path (default: `umbra-<version>-windows-x64.zip` in cwd) |
| `-Version` | no | Version string for default name (default `0.0.0-dev`, or `$env:UMBRA_VERSION`) |

## MSIX (optional)

MSIX needs the [`msix`](https://pub.dev/packages/msix) pub package configured in `app/pubspec.yaml` (snippet in the packaging README) **and** a code-signing certificate.

GitHub secrets:

| Secret | Purpose |
|--------|---------|
| `WINDOWS_CERT_BASE64` | Base64-encoded `.pfx` |
| `WINDOWS_CERT_PASSWORD` | PFX password |

If either secret is missing, **ship zip only**. Do not publish an unsigned MSIX as the official Windows build.

CI sketch:

```powershell
if ($env:WINDOWS_CERT_BASE64 -and $env:WINDOWS_CERT_PASSWORD) {
  $pfx = Join-Path $env:RUNNER_TEMP "umbra.pfx"
  [IO.File]::WriteAllBytes($pfx, [Convert]::FromBase64String($env:WINDOWS_CERT_BASE64))
  # Point msix_config.certificate_path / certificate_password at $pfx and the secret,
  # then: flutter pub run msix:create
} else {
  Write-Warning "WINDOWS_CERT_* secrets absent; shipping zip only"
}
```

`*.pfx` is gitignored at the repo root.

## Window title / Runner identity

See packaging README snippets for `Runner.rc`, `main.cpp`, and `msix_config`.
