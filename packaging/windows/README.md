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

How `ci.yml` and `release.yml` do it: when both secrets are set, it decodes the PFX into
`$env:RUNNER_TEMP`, then runs `dart run msix:create` in `app/` with the cert passed
as **CLI flags** (`--certificate-path`, `--certificate-password`), plus
`--build-windows false` (reuses the release build), `--install-certificate false`,
and `--output-path dist`. It deletes the PFX afterward and fails if no `.msix` was
produced. The `msix` package does not read the certificate from environment
variables, and without a certificate it silently falls back to its self-signed
test cert, so never put `certificate_path`/`certificate_password` in the committed
`msix_config`.

Local equivalent:

```powershell
cd app
flutter build windows --release
dart run msix:create --certificate-path C:\path\to\umbra.pfx `
  --certificate-password <password> --build-windows false --install-certificate false
```

Note: until the `WINDOWS_CERT_*` secrets are added to the repo, both workflows
skip MSIX and ship the zip only, so the signing path stays untested until then.

`*.pfx` is gitignored at the repo root.

## Window title / Runner identity

See packaging README snippets for `Runner.rc`, `main.cpp`, and `msix_config`.
