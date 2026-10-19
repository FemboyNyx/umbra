# Call signaling (v1)

All messages travel **only** on core’s E2E encrypted channel as application payloads.

Suggested core envelope (for Elite / Copilot to agree):

```json
{
  "ns": "umbra.call.v1",
  "body": { /* CallSignalMessage.toJson() */ }
}
```

## Types
| `type` | Direction | Notes |
|--------|-----------|--------|
| `call.invite` | caller → callee | rings; `media` = `audio` \| `av` |
| `call.accept` | callee → caller | caller then sends SDP offer |
| `call.reject` | callee → caller | optional `reason` |
| `call.hangup` | either | ends in any state |
| `call.sdp.offer` | caller → callee | after accept |
| `call.sdp.answer` | callee → caller | |
| `call.ice` | either | trickle ICE |

## FFI needs from `core/`
- Stable local peer id string
- `send_app_message(peer_id, ns, body_bytes_or_json)`
- Inbound callback/stream for `ns == umbra.call.v1`
- Prefer not blocking the UI isolate

## Privacy (threat model)
- Call **media** cannot traverse Tor; ICE may expose peer IPs.
- App must show [preCallIpWarning] before first call (and when Tor mode is on).
- **Relay only** sets `iceTransportPolicy: relay` and requires user-configured TURN.
