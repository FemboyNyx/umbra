# Message protection (design)

Status: **design only, nothing here is implemented.** Written against
`jal-servers-2` on top of main `b79c7af`. `app/` hasn't landed yet, so every
**app** item is a requirement for Copilot, not something that's been checked.
Numbers marked **(proposed)** are defaults we picked and can change.

This covers what protects messages once they're on a device or on the wire,
beyond the Olm and Megolm encryption in [architecture.md](architecture.md)
and [servers.md](servers.md): screen security, app lock and a duress PIN,
wiping after failed unlocks, sealed sender, padding and timing, view-once, a
forward and copy hint, and anti-replay and anti-rollback built on the
sequence numbers, hash chains and checkpoints from
[servers.md](servers.md#host-sanity-checks).

The rule for this doc: each item says who it actually stops. Several
features messengers ship stop nobody determined, and those are called out as
**theatre**. Theatre isn't always useless (some of it stops accidents), but
it must never be described to users as protection against a recipient.

## Ranked: value versus cost

Ranked by value for the cost. **Layer** says where the work is: **core**
(Rust), **app** (Flutter), or **both**. Phases match
[servers.md](servers.md#phasing): 0 is core prerequisites, 1 is servers v1,
2 is the next round, 3 is voice and later.

| Rank | Item | Layer | Stops | Value | Cost | Phase |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | [Anti-replay](#anti-replay) | core | Replayed Olm or Megolm messages, duplicate delivery | High | Low | 0 (DMs), 1 (servers) |
| 2 | [Padding](#padding) | core | Length analysis by the network, relays and hosts | High | Low | 0 (DMs), 1 (servers, already designed) |
| 3 | [Hash chains and checkpoints](#hash-chains-and-checkpoints) | core | Silent drops, withholding, split views, sender equivocation | High | Medium | 1 |
| 4 | [Anti-rollback](#anti-rollback) | core | A restored old database resurrecting deleted messages or reusing ratchet state | High | Medium | 1 |
| 5 | [App lock](#app-lock) (with a hard lock that closes the database) | both | Someone who picks up an unlocked phone; a seized device after the hard lock | High | Medium | 1 |
| 6 | [Screen security](#screen-security-flag_secure) (`FLAG_SECURE`, app-switcher hiding) | app | Recents thumbnails, casual screenshots, screen-recording malware without root, accidental screen sharing | Medium | Low | 1 |
| 7 | [Timing](#timing) toggles (read receipts, typing) | both | Peers learning exactly when you read | Medium | Low | 1 |
| 8 | [View-once](#view-once) | both | Content lingering on the recipient's device by accident | Low to medium | Low | 2 |
| 9 | [Wipe after failed unlocks](#wipe-after-failed-unlocks) | both | Someone guessing PINs in the UI | Low to medium | Low | 2 |
| 10 | [Forward and copy hint](#forward-and-copy-hint) | app | Accidental forwarding by an honest recipient | Low (**theatre** against anyone else) | Very low | 2 |
| 11 | [Duress PIN](#duress-pin) | both | Someone who forces you to unlock and only uses the UI | Low (**mostly theatre**) | Medium, plus legal risk | 3 (pending Syd) |
| 12 | [Sealed sender for hosts](#sealed-sender) | core | A host learning which member posted or relayed what | Medium on Tor, **theatre on clearnet** | High | 3 |
| — | Cover traffic | core | Global timing analysis | Low at any cost we can afford | Very high (battery, bandwidth) | Not planned |

## Anti-replay

**Layer: core. Phase 0 for DMs, 1 for servers.**

- **Olm.** A replayed Olm message fails: vodozemac's ratchet has already used
  and dropped that message key. Nothing to add beyond the `MessageId` check.
- **Megolm doesn't stop replays.** `InboundGroupSession::decrypt` in
  vodozemac 0.11.1 (`src/megolm/inbound_group_session.rs`) re-derives the
  ratchet for any index at or after `first_known_index` and decrypts it
  again. It keeps no record of indices it already returned. So anyone who
  holds an envelope (a host, a removed member) can re-push it and honest
  clients would show it twice.
- **What core does.** Keep `(session_id, message_index)` for every decrypted
  Megolm message, and `(sender, seq)` from the
  [hash chains](#hash-chains-and-checkpoints), and drop anything already seen.
  Same `(sender, seq)` with different content is a **sender fork**, not a
  replay, and is flagged as one. Keep the record until the session is deleted
  (it's small: 12 bytes per message).
- **DMs.** `Payload` v2 carries `seq` and `prev`. A `seq` at or below the
  highest seen from that contact is dropped. `MessageId` dedupe stays.
- **Signals.** Call signals already only come from Olm, and Olm already
  rejects replays. `call_id` dedupe in `calls/` covers the rest.

## Padding

**Layer: core. Phase 0 for DMs, 1 for servers.**

- Olm plaintext (`Payload`) is padded to buckets of **256 B, 1 KiB, 4 KiB,
  16 KiB, 64 KiB (proposed)** before encryption. Today message length leaks
  exactly (threat model follow-up 6).
- Megolm plaintext is padded to power-of-two buckets up to 64 KiB, as in
  [servers.md](servers.md#wire-protocol-additions).
- **Files** leak their size through the chunk count. The last chunk is padded
  to a full 256 KiB, and the chunk count is rounded up to the next Padmé
  value (at most about 12% overhead), with dummy chunks that decrypt to
  nothing. Thumbnails are padded to one bucket.
- Padding hides lengths, not counts or timing.

## Timing

**Layer: both. Phase 1 for the toggles.**

What leaks: when you're online (`Hello`, `Delivered`), when you read (read
receipts), when you type, and to a host, when each member posts and fetches.

| Measure | Value | Notes |
| --- | --- | --- |
| Read receipts and typing indicators off (a setting, **app**) | Medium | Off means peers learn delivery, not reading. Delivery acks stay, because the outbox needs them |
| Hosts answer fetches in batches, not as each envelope arrives (**core**, host) | Low | Blurs exact post times between members. The host still knows |
| Random send jitter | Very low | Seconds of jitter don't stop anyone correlating traffic |
| Tor-only mode | Medium | Hides IPs, not timing |
| Cover traffic (constant-rate dummy messages) | Low for us | Only works at a constant rate all the time, which kills mobile batteries. **Not planned** |

## Hash chains and checkpoints

**Layer: core. Phase 1.** Designed in full in
[servers.md](servers.md#host-sanity-checks). In summary: every channel
message and every DM carries a per-sender `seq` and the hash of the sender's
previous message (`prev`), senders sign periodic heads, hosts sign receipts,
and members cross-check heads. That gives:

- detection of drops, withholding and split views by hosts, with alerts that
  don't fire for expiry (`sent + timer`), retention, deletions or join
  windows;
- **transcript consistency** for DMs, which Olm alone doesn't give: after
  each `Hello`, both sides exchange their last sent and received `seq` and
  hash, so a missing or altered message shows up;
- detection of a sender showing different members different content;
- the witnesses that [anti-rollback](#anti-rollback) relies on.

## Anti-rollback

**Layer: core. Phase 1.** (Manifest rollback is already covered by `seq` and
`prev`, invariant S5 in servers.md.)

The risk here is **your own device's state** going backwards: someone
restores an old copy of the database (a backup, a disk image, a copied app
folder). That would bring back deleted and expired messages, and roll Olm and
Megolm state back. Sending from rolled-back Olm state reuses message keys,
and Olm v1 derives the AES-CBC IV from the message key, so two different
messages under the same key leak where their plaintexts match.

Defences:

1. **Peers are witnesses.** Our `seq` per chat and per channel is in what we
   sent. After a `Hello`, a contact's `Heads` say the highest `seq` they've
   received from us. If that's above what our database thinks we sent, our
   state has been rolled back. Core then **refuses to send on any existing Olm
   session with that peer**, starts a fresh one (a new pre-key message),
   rotates every Megolm outbound session, and jumps its `seq` past what the
   peer saw. The UI says "This device's data was restored from an older copy."
2. **Peers see it too.** A contact who receives a `seq` they've already seen
   with different content treats it as a fork from our device and shows a
   warning, the same as in servers.
3. **A local epoch** in the `meta` table, bumped on every expiry run and every
   session change, also stored in the OS keystore item next to `db_key` where
   the platform allows. A database whose epoch is lower than the keystore's
   was rolled back. This only works if the attacker can't roll the keystore
   back too.
4. **Backups off.** Android `allowBackup="false"` and data extraction rules
   (already a threat model requirement), so the OS doesn't make the old copy
   for them.

It can't stop someone reading a stolen old copy. It stops that copy from
being used to send, and from quietly bringing deleted messages back on a
device that's still in use.

## App lock

**Layer: both. Phase 1.**

Two levels, both on by choice:

| Level | What it does | Stops | Doesn't stop |
| --- | --- | --- | --- |
| **Soft lock** (after the app is backgrounded, **1 minute, proposed**) | A PIN, passphrase or OS biometric before the UI shows anything. Core keeps running, so messages still arrive | Someone who picks up an unlocked phone | Anyone who can read the app's memory or the database with `db_key` from the keystore |
| **Hard lock** (after **1 hour idle, proposed**, or on demand) | The app calls core `lock()`, which stops the network, closes SQLCipher and zeroizes `db_key` and the pickle keys. With a passphrase set, `db_key` is wrapped with `Argon2id(passphrase)` as well as the keystore, so it can't be unwrapped without the passphrase | A seized device after the hard lock, as long as the passphrase is strong | Nothing arrives while hard-locked. Messages wait in senders' outboxes and on hosts, up to `sent + timer` for disappearing ones |

Notes:

- Biometrics only unlock the soft lock, never a hard lock that has a
  passphrase.
- Notifications while locked say "New message", with no sender or text.
- A soft lock without a hard lock is mostly a UI gate. It's still worth it,
  because "someone picked up my phone" is the common case.
- The Argon2id passphrase is the Linux fallback the threat model already asks
  for when Secret Service isn't there.

## Screen security (FLAG_SECURE)

**Layer: app. Phase 1.** On by default in chats with a disappearing timer
and in the view-once viewer, and as a global setting (**off by default
elsewhere, proposed**, because it also blocks screenshots the user wants).

| Platform | Mechanism | Effect |
| --- | --- | --- |
| Android | `WindowManager.LayoutParams.FLAG_SECURE` on the activity window | Blocks screenshots and screen recording, shows a blank screen to casting and to the recents thumbnail. On Android 14 and later, `Activity.ScreenCaptureCallback` can also tell us a screenshot was taken where it isn't blocked |
| Windows | `SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE)` (Windows 10 2004 and later) | The window is left out of screenshots, recordings and screen sharing |
| Linux | Nothing reliable. Any X11 client can read the screen, and Wayland has no standard opt-out | The app says so. We cover over the window when it loses focus, but that's all |

**Theatre call-out.** None of this stops a recipient who photographs the
screen with another phone, uses a rooted or modified device, or runs a
modified Umbra. It stops recents thumbnails, casual screenshots, unprivileged
recording apps and accidental screen sharing. The UI calls it "screen
security", never "screenshot-proof", and a sender's request for it is shown
as a request, not a guarantee.

## View-once

**Layer: both. Phase 2.**

- The sender sets `view_once: true` in the payload (DMs and channels). The
  recipient sees "Tap to view". Opening it shows it once in a viewer with
  screen security on, and closing the viewer deletes the plaintext, any
  attachment and its file key, and marks it viewed.
- Never shown in notifications, previews, replies or search. Can't be
  forwarded, saved, copied or quoted.
- **Hosts and timers.** A view-once message always has a disappearing timer:
  the chat or channel timer if one is set, otherwise **14 days (proposed)**.
  It follows the [servers.md rules](servers.md#when-the-timer-starts-decided-chi):
  hosts delete at `sent + timer` (or 30-day retention), and unopened copies
  on recipient devices go at `sent + timer + 7 days` at the latest. None of
  this raises integrity alerts.
- **In servers**, each member gets to view it once. It's never part of shared
  history, the same as expired messages.
- Viewed receipts follow the read receipt setting.

**Theatre call-out.** View-once relies on the recipient's app being honest.
A photo of the screen, a modified client or a rooted device keeps it. It's
good for "don't leave this sitting in your gallery", not for "the recipient
can't keep it". The sending UI says that.

## Forward and copy hint

**Layer: app. Phase 2.** `no_forward: true` in the payload. Honest clients
hide forward, copy, share, save and quote, and show "The sender asked you not
to forward this".

**Theatre call-out.** This is a request, nothing more. Anyone can retype,
screenshot or use a modified client. The UI must call it a request.

## Wipe after failed unlocks

**Layer: both. Phase 2. Off by default.**

After **10 failed unlocks (proposed)**, with a warning from 5 left and an
increasing delay after each failure, the app **crypto-erases**: it deletes
the keystore item holding `db_key` (and the wrapped copy), then deletes the
database. Deleting the key is what counts, since flash wear-levelling means
deleting files doesn't really erase them.

Limits:

- The counter lives on the device. Someone with a disk image skips the UI and
  attacks the Argon2id-wrapped key offline, so the real protection is a
  strong passphrase, not the wipe.
- It's easy to trigger by accident (a child, a pocket). Hence off by default
  and a clear warning.

## Duress PIN

**Layer: both. Phase 3, pending Syd's legal review.**

A second PIN that, at the lock screen, does the same crypto-erase as the
failed-unlock wipe and then shows the app as freshly installed.

**Mostly theatre**:

- A forensic examiner images the device first and never touches our lock
  screen.
- An app that's obviously been used for months but opens empty is
  suspicious, and it's not plausible deniability.
- Destroying data while someone is demanding access can be a crime in its
  own right (destroying evidence, obstruction) in many places. **Syd** should
  review this before we build it.

The one honest case: someone who grabs your phone and forces you to open the
app, then looks at it on the spot. If Nyx wants it, it ships off by default
with that limit spelled out.

## Sealed sender

**Layer: core. Phase 3.**

Signal's sealed sender hides the sender from Signal's server. In Umbra:

- **Direct DMs** gain nothing. The recipient has to know the sender, and the
  Noise connection is authenticated as the sender's PeerId anyway. A circuit
  relay sees both PeerIds, and fixing that needs onion routing, not sealed
  sender.
- **Hosts** are where it could help. Today a host knows which member pushed
  each envelope and who relays what to whom, from the authenticated
  connection. The design to hide that: members push over a connection with an
  **ephemeral PeerId**, and prove membership with an anonymous, rate-limited
  token (blind-signed tokens in the style of Privacy Pass, issued per member
  per day by an admin) instead of the host ACL. The sender's identity stays
  inside Megolm or Olm, where only members see it.

Costs: a token issuer, rate limits per token instead of per member (so abuse
control gets weaker), and new connections for every push.

**Theatre call-out.** On clearnet, the host sees your IP on every push, so it
can link pushes to you no matter which PeerId you use. Sealed sender only
means something in Tor-only mode with separate circuits per push.

## Threat model touch-ups

[threat-model.md](threat-model.md) now links here for replay, size and timing
analysis, stolen unlocked devices and backups. New assets: none. New
invariants:

| # | Invariant |
| --- | --- |
| P1 | A Megolm `(session_id, message_index)` or a `(sender, seq)` is never accepted twice |
| P2 | Core never sends on an Olm or Megolm session after it detects its own state was rolled back |
| P3 | Every Olm and Megolm plaintext is padded to a bucket before encryption |
| P4 | A hard lock leaves no `db_key`, pickle key or open database in core's memory |
| P5 | View-once content is never stored in notifications, search, history shares or the gallery |
| P6 | The UI never describes screen security, view-once or the forward hint as protection against the recipient |
