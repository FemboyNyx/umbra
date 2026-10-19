# Call signaling (v1)

Via core `send_signal(peer_id, call_id, kind, payload)` and `CoreEvent::Signal`.
Local peer id from `self_info()`.

## Flow
`Ringing` → `Accept` | `Busy` | `Reject` → `Offer` → `Answer` → `IceCandidate`* → `Hangup`

## Transport notes
- `send_signal` is E2E encrypted but **not** queued. Unreachable peer → error → treat call as **failed**.
- Payload cap: **64 KiB**.

## E2E / injection rule (threat model)
Call media negotiation (SDP Offer/Answer and ICE candidates) is only end-to-end
confidential and authentic if those payloads are accepted **solely** from
`CoreEvent::Signal` (core's encrypted channel).

- App bridge must call `CallController.handleCoreSignal(...)` for every
  `CoreEvent::Signal`.
- Do **not** apply Offer/Answer/ICE from the UI, deep links, push notifications,
  clipboard, QR extras, or any plaintext/unauthenticated source.
- `CallController` drops Offer/Answer/ICE unless they match the active
  `callId` and `remotePeerId`.
- `CallSession.applyRemoteSignal` is for the controller only — not for UI code.
