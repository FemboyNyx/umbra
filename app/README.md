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

Chat and contact detail have a Start call button. It uses `CallController.startCall` only after an audio input and output device are present. WebRTC's audio device starts on that first call, not at process launch. If none is found, or opening it fails, the app stays idle and says "No microphone or audio device found" — no ringing signal is sent. If the device disappears after the peer was invited, the call is hung up so they are not left ringing. The button is disabled while Tor has no TURN relay, while a call is already active, and while a safety-number change is pending.

A contact whose identity key changed shows a safety-number warning. Accept trusts the new key and clears verified. Reject keeps the old key.

In Tor mode the call stack uses `setTorMode(true)`. Calls stay relay-only and are blocked until a TURN server exists. A missed call while Tor has no relay shows "Someone tried to call".

`start_network` failures are shown as "couldn't start network". The raw error, which can include listen addresses, is debug-only.

## App lock and screen security

Both locks are off until you turn them on in Settings. A PIN or passphrase (4–128 characters) is stored only as an HMAC-SHA256 verifier. That is a UI check, not the Argon2id wrap from `docs/message-protection.md`.

Soft lock waits until Umbra has been in the background for 1 minute, then hides the UI. The core keeps running. Biometrics can open a soft lock when the device offers them (Android). They never open a hard lock. While the UI is locked, the only traffic notice is "New message", with no sender and no text.

Hard lock runs after 1 hour without use, or from Lock now. It calls core `lock()`. That method is not in the FFI: the real backend only stops the network, so nothing new arrives, and the database stays open. The mock does the same and drops events. Unlocking brings the network back. The passphrase is required. Biometrics are refused.

Screen security is off unless you turn it on. The label is "screen security". It is not described as screenshot-proof, and a request for it is a request, not a guarantee. Disappearing chats and the view-once viewer would also turn it on; those screens are not in the app yet. Android sets `FLAG_SECURE` (and registers the API 34 screen-capture callback without showing a notice). Windows uses `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)`. Linux cannot block screenshots. The app says so, and covers the window when it loses focus.

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
