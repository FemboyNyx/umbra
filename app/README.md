# Umbra

Flutter client for Umbra. License: AGPL-3.0-or-later (see the repository `LICENSE`).

The UI talks to a thin core client. With the default mock backend it shows a fake contact and echo replies without a native build. Real networking uses `flutter_rust_bridge` 2.13.0 against `../core`.

## Run the mock

```bash
cd app
flutter pub get
flutter analyze
flutter test
flutter run
```

Mock is the default (`CoreConfig.mock: true`). Settings can switch to the real backend; that choice is applied on the next launch because `init` keeps the first configuration for a data directory.

Onboarding asks for a display name. Contacts lists the pending Mock Bot request. Accept adds it with `verified: false`. Chat sends text and shows the echo. Contact detail shows the safety number and the verify toggle.

## Database key

The app generates 32 random bytes once and stores them in the OS keystore (Android Keystore, Windows DPAPI, Linux libsecret). That exact key is passed to `init` and then wiped from Dart. Private identity keys stay in Rust.

If libsecret is missing, Linux writes `{supportDir}/db_key.bin` mode `0600` and reports the backend as `file`.

## Call signals

Every incoming `CoreEvent::Signal` is forwarded only through `handleCoreSignal`. That is the only calls entry point. The app does not branch on Offer, Answer, or ICE.

WebRTC's audio device starts on the first call, not at process launch. If no audio device is available the call fails with "No audio device is available" instead of aborting.

A contact whose identity key changed shows a safety-number warning. Accept trusts the new key and clears verified. Reject keeps the old key.

In Tor mode the call stack uses `setTorMode(true)`. Calls stay relay-only and are blocked until a TURN server exists. A missed call while Tor has no relay shows "Someone tried to call".

`start_network` failures are shown as "couldn't start network". The raw error, which can include listen addresses, is debug-only.

## Codegen

Bindings are already generated under `lib/src/rust/` and `core/src/frb_generated.rs`. Regenerate with codegen **2.13.0** after the Rust API changes:

```bash
# from app/, with flutter_rust_bridge_codegen 2.13.0 on PATH
flutter_rust_bridge_codegen generate
```

`flutter_rust_bridge.yaml` pins `rust_input: crate::api` and `rust_root: ../core`.

## Native build

`rust_builder` compiles `../core` with cargokit when `cargo` is on `PATH` and `core/Cargo.toml` exists. Linux and Windows CI images do not install Rust; those builds link a stub `libumbra_core` and the Dart mock still runs. Android CI has Rust when `core/` is present, so the release/debug APK links the real library.

## Android signing

If `android/key.properties` exists, release builds use it. Otherwise Gradle prints a warning and signs with the debug keystore. See `android/key.properties.example`. Do not commit keystores or `key.properties`.

ABI splits (`armeabi-v7a`, `arm64-v8a`, `x86_64`, plus a universal APK) apply to release tasks.
