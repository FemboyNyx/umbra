# Umbra threat model

Status: reviewed against `core/` at `119c53b`. `app/` and `calls/` haven't
landed yet, so anything that depends on them is marked **(app)** or
**(calls)** and is a requirement, not something that's been checked. The
design is described in [architecture.md](architecture.md).

## What we protect

| Asset | Why it matters |
| --- | --- |
| Message contents | The core promise of the app |
| Call audio and video | Same as messages |
| Private keys (libp2p Ed25519, Olm account, sessions) and `db_key` | Whoever has them can read and impersonate |
| The social graph (who talks to whom) | Often as sensitive as content |
| IP address and location | Can identify the person and where they are |
| Activity timing | Reveals patterns of life |
| Local history (`messages`, `contacts`, `outbox`) | Exposed if a device is lost or seized |

## Who we defend against

1. **A passive network observer**, such as an ISP, public Wi-Fi, or a tap on
   a link.
2. **An active network attacker** who can intercept, change, drop, or replay
   traffic, or impersonate a peer.
3. **Infrastructure peers** we don't control, meaning DHT nodes, circuit
   relays, TURN servers, and Tor relays.
4. **A malicious contact.**
5. **A stranger** who knows a `PeerId`.
6. **Someone who gets the device**, locked or unlocked.
7. **A supply-chain attacker** who goes after dependencies, CI, or releases.

**Out of scope:** a compromised OS or malware on the device, a global passive
adversary, someone forced to unlock their device, and what the recipient does
with messages after they're decrypted.

## Threats, mitigations, and what's left

### Content

| Threat | Mitigation in code | What's left |
| --- | --- | --- |
| A network attacker reads or changes messages | Olm v1 (vodozemac) inside Noise/yamux or QUIC TLS. Tampered ciphertext fails to decrypt without breaking the session (tested) | None known, assuming vodozemac is correct |
| Ciphertext sent to the wrong peer | `Payload.to` is checked against our PeerId after decryption | None |
| MITM when adding a contact | `validate_card` checks that `ed` derives `peer`, and keys are pinned at `add_contact` | An `umbra://` link pasted through an untrusted channel can be swapped wholesale. **(app)** should prefer QR codes and push people to compare safety numbers |
| A key gets swapped after the contact is added | Changed keys are refused in `add_contact`, in `Hello` (`Malformed`), and in one-time-key responses | **The user isn't told.** A `Hello` with changed keys only logs a warning, and there's no `CoreEvent`. Recommendation: emit a `KeyChanged` event so **(app)** can show it |
| `ik` isn't signed by `ed` | `ik` is only accepted from the card or from a Noise session authenticated as that PeerId | Fine for now. Signing `ik` with `ed` in the card would let any copy of the card be checked on its own |
| One-time key abuse | Only contacts can claim OTKs. Each claim makes a fresh OTK (falling back to the fallback key), and the response's `identity_key` has to match the pinned one | A malicious contact can claim over and over. vodozemac keeps only a limited number of OTKs, so that could push out a key another contact claimed but hasn't used yet, and that contact's first message would fail with `DecryptFailed` and then retry. Rate-limit claims per peer |
| Replay | Olm message indices and ratchet state | None for Olm. The app has to treat message IDs as unique too. Megolm (servers) doesn't reject replays itself, so core has to keep a replay record ([message-protection.md](message-protection.md#anti-replay)) |
| Compromise of current session keys | The Double Ratchet gives forward secrecy and heals after a compromise | The newest 4 sessions per peer are kept |
| Call media MITM | SDP only travels as an Olm `Signal` payload | **(calls)** has to reject SDP from any other source, and that's the most important invariant for calls |
| Oversized input | Text and signals capped at 64 KiB, `call_id` at 128 bytes, request timeouts | The CBOR and JSON decoders run on peer input and should be fuzzed |

### Metadata

| Threat | Mitigation in code | What's left |
| --- | --- | --- |
| A network observer sees who you talk to (clearnet) | None. Direct TCP/QUIC reveals both IPs | **High.** Tor-only mode is the mitigation, and it's opt-in |
| The same in Tor-only mode | SOCKS5 (`tor_socks`) or embedded arti. No QUIC, no listening, no mDNS, no DCUtR, Kad in client mode, listen addresses hidden from identify | The relay sees both PeerIds and timing. Calls aren't covered |
| DHT nodes learn interest in a PeerId | Only used to find peers that moved, and contacts' known addresses go first. Umbra has its own Kad protocol name | **High.** `get_closest_peers(peer)` shows who is looking for whom |
| PeerIds never change, so they're trackable | None | **High.** Every relay and DHT node sees the same ID across networks. Rotating routing IDs is future work |
| mDNS broadcasts on the LAN | Off unless `enable_mdns`, and always off with Tor | **(app)** should keep it off by default and explain what it does |
| A peer learns your IP during a call | None in core | **(calls)/(app)** have to warn before a call or force TURN, especially in Tor mode |
| Size and timing analysis | None. No padding | Messages leak their length. Padding buckets and timing toggles are proposed in [message-protection.md](message-protection.md#padding) |
| A community-server host logs IPs (servers, not built yet) | `umbra-host` keeps no IP or connection logs by default. Logging is an explicit operator opt-in with a warning and short rotation, and members see it in the host info ([servers.md](servers.md#operator-duties-legal-driven-phase-1), [legal.md §4.4](legal.md#44-data-retention-and-law-enforcement-requests)) | A host that opts in, or a modified host. Tor and onion hosts hide IPs either way |
| Strangers probe whether you're online | A `Hello` from a non-contact gets `NotAContact` | Even a refusal confirms the peer is online, and every probe makes a `ContactRequest` event |

### Availability and abuse

| Threat | Mitigation in code | What's left |
| --- | --- | --- |
| A flood of contact requests | Non-contacts can't claim keys or have messages decrypted | Unlimited `ContactRequest` events. **(app)** needs a rate limit or collapse repeats, and core should rate-limit per PeerId |
| Eclipse or Sybil attacks on the DHT | Contacts' addresses from cards and `Hello` don't depend on the DHT | A determined attacker can hide peers that have moved |
| A relay operator drops traffic | Several `relay_addrs` can be used | Tor-only users depend on relays to be reachable at all |
| Undelivered messages | Outbox with ordered retries | Messages expire after 7 days. **(app)** has to show expired messages as failed. The receiver isn't told today; [servers.md](servers.md#dms-and-the-outbox) proposes `Payload` v2 with `seq`, `prev` and `skipped` so gaps are visible |
| A host drops or withholds stored data (servers, not built yet) | Designed in [servers.md](servers.md#host-sanity-checks): hash chains, signed heads, host receipts and cross-checks raise alerts | Detection only. A host can still deny service, and can keep data past expiry without anyone knowing, including under a legal preservation hold (ciphertext only). Operator refusals (denylists, takedowns) come with signed receipts and show as "refused by host operator", not as withholding |

### The device

| Threat | Mitigation in code | What's left |
| --- | --- | --- |
| A stolen or seized locked device | SQLCipher database, plus pickles encrypted with an HKDF-derived key | Only as strong as the OS keystore that holds `db_key` **(app)**. Linux without Secret Service needs a passphrase fallback (Argon2id) |
| A stolen unlocked device | None in core | **(app)**: optional app lock (soft and hard lock, see [message-protection.md](message-protection.md#app-lock)). Disappearing messages are designed in [servers.md](servers.md#disappearing-messages) (best effort: recipients can still screenshot, copy or run a modified client) |
| Backups copy the database | The database is encrypted | **(app)**: Android `allowBackup="false"` and data extraction rules, so `db_key` and the database aren't backed up together. A restored old copy could resurrect deleted messages and reuse ratchet state; detection via peers' sequence numbers is in [message-protection.md](message-protection.md#anti-rollback) |
| Keys left in memory | `Zeroizing` on the `db_key` pragma, device key bytes, and the pickle key | The Olm account and sessions are in memory while running. Dart strings with plaintext can't be wiped |
| Plaintext in logs | Core logs PeerIds and events, not message bodies | Needs a check that no `Debug` print of a `Payload` reaches logs |

### Supply chain

| Threat | Mitigation | What's left |
| --- | --- | --- |
| A malicious dependency | `Cargo.lock` committed, CI builds with `--locked`, frb pinned to `=2.13.0` | Transitive dependencies are large (libp2p, arti). Adding `cargo audit` and `cargo deny` to CI is recommended |
| A CI compromise or leaked secrets | Signing secrets only in GitHub secrets, release builds only on tags | Pinning third-party actions to commit SHAs is recommended |
| Tampered downloads | `SHA256SUMS` on releases, Android APK signing | Unsigned checksums. Signing them (minisign or Sigstore) is recommended |

## Community servers

Servers (Megolm groups, hosts, invites, history sharing, host sanity checks
and disappearing messages) are design only. Their threats and invariants
S1 to S11 are in [servers.md](servers.md#threat-model-deltas), and they get
merged here once core implements them.

## Security invariants

These are checked against `119c53b`.

| # | Invariant | Status |
| --- | --- | --- |
| 1 | No message content goes on the wire unless Olm encrypted it | **Holds.** Only `Hello` (public card) and `KeysRequest` are in the clear inside Noise |
| 2 | Private keys never cross to Dart | **Holds.** Only `db_key` crosses, going into core |
| 3 | Messages, OTK responses, and `Hello` with keys that don't match the pins are rejected | **Holds**, but the user isn't told (no event) |
| 4 | Tor-only mode means no QUIC, listening, mDNS, DCUtR, or clearnet dials | **Holds** by how the transport is built. Needs a test that checks no non-Tor dial happens |
| 5 | Call SDP is only accepted from an Olm `Signal` | **Core holds** (signals only come from decrypted payloads). **(calls)** has to enforce it |
| 6 | Logs never contain plaintext or keys | Not checked by tests |
| 7 | Nothing from a non-contact is decrypted or stored | **Holds.** It's rejected with `NotAContact` |

## Recommended follow-ups (in priority order)

1. **core:** emit `CoreEvent::KeyChanged { peer_id }` when a `Hello` or
   one-time-key response has keys that don't match the pins.
2. **calls:** only take SDP and ICE from `CoreEvent::Signal`, and warn about
   the IP leak before a call in Tor mode.
3. **core:** rate-limit `Hello` and `ContactRequest` per PeerId.
4. **ci:** add `cargo audit`/`cargo deny`, pin actions to SHAs, and sign
   `SHA256SUMS`.
5. **core:** add a test that Tor-only mode never dials outside Tor.
6. **core:** padding buckets for `Payload`, and fuzzing for the CBOR and JSON
   decoders.
7. **core:** consider signing `ik` with `ed` in the card.
8. **core/app:** the message-protection items ranked in
   [message-protection.md](message-protection.md#ranked-value-versus-cost),
   starting with anti-replay, padding and anti-rollback.

## Before calling it secure

An external review of `core/` and the call signaling path, fuzzing, a tested
key-change UX, and an update to this document to match the code.
