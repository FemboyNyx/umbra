# Call signaling (v1)

Via core `send_signal(peer_id, call_id, kind, payload)` and `CoreEvent::Signal`.
Local peer id from `self_info()`.

## Flow
`Ringing` → `Accept` | `Busy` | `Reject` → `Offer` → `Answer` → `IceCandidate`* → `Hangup`

## Transport notes
- `send_signal` is E2E encrypted but **not** queued. Unreachable peer → error → treat call as **failed**.
- Payload cap: **64 KiB**.
