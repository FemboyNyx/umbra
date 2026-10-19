# Umbra

Peer-to-peer, end-to-end encrypted messaging, voice calls, and video calls for **Android, Windows, and Linux**.

> Status: early development. Not audited, so don't rely on it for high-risk use yet.

## Goals
- E2E encrypted 1:1 and group chat built on well-known, audited primitives (no homemade crypto)
- Direct peer-to-peer connections, with relay fallback only when NAT forces it
- Encrypted voice and video calls (WebRTC, DTLS-SRTP)
- One codebase that ships to Android, Windows, and Linux
- Optional Tor routing for metadata privacy (experimental)

## Layout (planned)
- `app/` cross-platform client
- `core/` crypto, identity, and messaging protocol
- `calls/` voice and video
- `signal-relay/` optional self-hostable rendezvous/relay
- `docs/` architecture, threat model, and protocol spec

## License
AGPL-3.0 (to be confirmed)
