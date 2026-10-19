# Umbra Calls

1:1 voice and video for Umbra, built on `flutter_webrtc` (DTLS-SRTP).

Call **signaling** (invite, accept, reject, hangup, SDP, ICE) is carried by the encrypted `core/` channel. This package never opens its own plaintext control channel.

## Layout
- `lib/src/signaling.dart` — wire types for call messages (JSON-friendly maps)
- `lib/src/ice_config.dart` — STUN defaults + optional user-supplied TURN (no hardcoded private servers)
- `lib/src/call_session.dart` — WebRTC peer connection + media lifecycle
- `lib/src/call_controller.dart` — high-level API the Flutter `app/` wires to UI
- `lib/umbra_calls.dart` — public exports

## Platforms
Android, Windows, Linux (same stack as the rest of Umbra).

## Security notes
- Media is DTLS-SRTP end-to-end between peers once ICE connects.
- Signaling payloads must be sent only through core’s E2E channel.
- TURN credentials, if any, come from the user or their self-hosted config — never baked into the binary.

## Privacy
- Pre-call IP / Tor warning strings live in `call_privacy.dart`.
- `CallController.setRelayOnly(true)` forces TURN-only ICE (`iceTransportPolicy: relay`).
- `CallController.setTorMode(true)` always forces relay-only and never uses STUN. With no TURN server, `startCall`/`acceptIncoming` throw `CallPrivacyException('tor_relay_required')` ("Calls over Tor need a relay server."), even with `skipPrivacyCheck`; check `callsBlockedByTor` to show that in the UI. There is no fallback to direct.
- TURN credentials are never hardcoded — set via `setTurnServers`.
