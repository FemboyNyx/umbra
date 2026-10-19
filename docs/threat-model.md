# Umbra threat model

Status: **draft**, written against the design in
[architecture.md](architecture.md) before most of the code exists. Revisit it
whenever the design changes, and before any release that's called "secure".

## What we protect

| Asset | Why it matters |
| --- | --- |
| Message and attachment contents | The core promise of the app |
| Call audio and video | Same as messages |
| Private keys (libp2p, vodozemac account, session state) | Whoever has them can read and impersonate |
| The social graph (who talks to whom) | Often as sensitive as content |
| IP address and location | Can identify the person and where they are |
| Activity timing (when someone's online or sending) | Reveals patterns of life |
| Local message history | Exposed if a device is lost or seized |

## Who we defend against

1. **A passive network observer**, such as an ISP, public Wi-Fi, or a
   government tap on a link. They can see packets, but they can't change them.
2. **An active network attacker** who can intercept, change, drop, or replay
   traffic, or impersonate a peer.
3. **Malicious infrastructure peers**, meaning DHT nodes, circuit relays,
   TURN servers, and Tor relays, which are run by people we don't know.
4. **A malicious or compromised contact**, someone the user chose to talk to.
5. **A stranger** who knows or guesses a `PeerId` and tries to message,
   spam, or probe the user.
6. **Someone who gets the device** while it's locked (lost, stolen, or
   seized) or, as a weaker case, while it's unlocked.
7. **A supply-chain attacker** who goes after dependencies, CI, or release
   artifacts.

### Out of scope

- A fully compromised OS or device, such as malware with root, a keylogger, or
  screen capture. No messenger can defend against that.
- A **global passive adversary** who can correlate traffic across the whole
  internet. Even Tor doesn't promise protection against that.
- Rubber-hose attacks, meaning someone forced to unlock their device.
- What the other person does with messages after they decrypt them
  (screenshots, forwarding).

## Threats and mitigations

### Confidentiality and integrity of content

| Threat | Mitigation | Residual risk |
| --- | --- | --- |
| A network attacker reads or changes messages | vodozemac Olm and Megolm (authenticated encryption) over libp2p Noise or TLS | None known, assuming vodozemac is correct |
| MITM at contact-add time | Keys exchanged by QR code or an invite link through a trusted channel, plus safety-number comparison | Invite links pasted into untrusted channels can be swapped. The UI should push people toward QR codes or verifying |
| A key change gets silently accepted | Pinned keys; any change is a hard warning and needs re-verification | Users click through warnings |
| Replayed messages | Olm and Megolm message indices, and rejecting duplicates | Megolm doesn't give post-compromise security within a session, so rotate it |
| A compromised session key exposes old messages | The Olm ratchet gives forward secrecy | Megolm only gives forward secrecy at rotation boundaries |
| Call media MITM | DTLS fingerprints only travel inside E2E-encrypted signaling | Depends on the SDP never coming from anywhere else, so `calls/` must enforce that |
| A malicious attachment exploits the parser | Size limits, decoding in a sandbox or isolate where possible, and no auto-open | Decoders for images and video are a real attack surface |

### Metadata

| Threat | Mitigation | Residual risk |
| --- | --- | --- |
| A network observer sees who you connect to | Optional Tor transport ([tor.md](tor.md)) | Off by default. Without Tor, IPs are visible |
| DHT nodes learn your `PeerId`, IP, and lookups | Can't be avoided with a public DHT. Contact address gossip cuts down on lookups | **High.** DHT queries for a `PeerId` show interest in it. A private or friends-only DHT mode is a possible later option |
| Relays learn who's talking to whom | Relays only see ciphertext | Relays see both peers' `PeerId`s and timing |
| A peer learns your IP during a call | Warn before calls; optionally force TURN | Calls can't use Tor |
| mDNS broadcasts presence on the LAN | Off by default and forced off with Tor | None when it's off |
| Message size and timing analysis | Padding buckets | Timing isn't hidden. Cover traffic is out of scope for v1 |
| `PeerId`s stay the same forever, so they can be tracked | None in v1 | **High.** Rotating routing identities apart from the vodozemac identity is a future option |

### Availability and abuse

| Threat | Mitigation | Residual risk |
| --- | --- | --- |
| Spam from strangers | Only contacts who've been added can open an Olm session, and everything else gets dropped before it's decrypted | Connection-level floods still use battery and bandwidth, so add rate limits per `PeerId` |
| A Sybil or eclipse attack on the DHT | Kademlia's default protections, and gossiping with contacts | A determined attacker can block discovery |
| Messages lost because both peers are never online together | Outbox and retries | Can't be fully fixed without store-and-forward |

### The device

| Threat | Mitigation | Residual risk |
| --- | --- | --- |
| A stolen or seized locked device | DB encrypted at rest, key in the OS keystore | Depends on how strong the OS keystore is, which varies a lot on Linux |
| A stolen unlocked device | Optional app lock (PIN or biometric), and messages that disappear on a timer | An unlocked device with the app open is game over |
| Cloud or adb backups leaking the DB | Backups disabled on Android | Desktop backup tools may still copy the data dir, so document it |
| Plaintext in logs or crash reports | No content or keys in logs, no third-party crash reporting, and `Debug` impls that redact | Needs review and tests |
| Keys left in memory | `zeroize` on key material in `core/` | Dart strings with plaintext can't be reliably wiped |

### Supply chain

| Threat | Mitigation | Residual risk |
| --- | --- | --- |
| A malicious crate or pub package | Committed lockfiles, `cargo audit` and `cargo deny` in CI, a small dependency list | Transitive dependencies are large (arti and libp2p especially) |
| A CI compromise or leaked secrets | Secrets only in GitHub secrets, release signing only on tag workflows, no secrets for PRs from forks, and actions pinned to commit SHAs | A compromised maintainer account is game over |
| Tampered release downloads | `SHA256SUMS` on every release, Android APK signing, and later signing the checksums (minisign or Sigstore) | The checksum file is only as trustworthy as the release page until it's signed |
| Non-reproducible builds | Pinned toolchains | Reproducible builds are a future goal |

## Security invariants

These should be enforced in code and covered by tests:

1. Nothing goes on the wire unless it's been encrypted by vodozemac.
2. Private keys never leave `core/` and never cross the FFI boundary.
3. A message from a `PeerId` whose pinned vodozemac key doesn't match gets
   dropped and raises a warning.
4. When Tor is on, QUIC, mDNS, and direct dials are off, and every libp2p dial
   goes through `umbra-tor`. Tests check that no other transport gets used.
5. Call SDP is only accepted from inside an established Olm session.
6. Logs never contain plaintext, keys, or full `PeerId`s at info level or
   above.

## Before calling it secure

- An external review of `core/` (crypto use, FFI, storage) and the call
  signaling path.
- Fuzzing the envelope parser and anything that decodes peer input.
- A written and tested key-change UX.
- Updating this document to match the code.
