//! Plain data types shared across the FFI boundary.
//!
//! Everything here is a plain struct or enum so `flutter_rust_bridge` v2 can
//! translate it to Dart (freezed classes / Dart enums).

use serde::{Deserialize, Serialize};

/// Message identifier (UUID v4, string form).
pub type MessageId = String;

/// Runtime configuration passed to [`crate::api::umbra::init`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreConfig {
    /// Kademlia bootstrap peers, as multiaddrs ending in `/p2p/<PeerId>`.
    pub bootstrap_addrs: Vec<String>,
    /// Circuit-relay v2 servers, as multiaddrs ending in `/p2p/<PeerId>`.
    /// Core reserves a slot on each and advertises the `/p2p-circuit` address.
    pub relay_addrs: Vec<String>,
    /// Enable LAN discovery via mDNS.
    pub enable_mdns: bool,
    /// SOCKS5 proxy for Tor as `ip:port` (e.g. `127.0.0.1:9050` for system
    /// tor, Orbot on Android). When set, the swarm is **Tor-only**: dials go
    /// through the proxy, no clearnet TCP/QUIC, no local listening, no mDNS,
    /// no DCUtR; inbound only via `relay_addrs` circuits. Calls (WebRTC) are
    /// not routed through Tor.
    pub tor_socks: Option<String>,
    /// ADDITION: multiaddrs to listen on. Empty = defaults
    /// (`/ip4/0.0.0.0/tcp/0`, `/ip4/0.0.0.0/udp/0/quic-v1` and IPv6 equivalents).
    pub listen_addrs: Vec<String>,
    /// ADDITION: run the in-memory mock backend (fake peers, echo replies,
    /// fake call answers). No disk or network access. For UI development.
    pub mock: bool,
}

/// Information about the local device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelfInfo {
    /// libp2p PeerId (base58), derived from the Ed25519 device key.
    pub peer_id: String,
    /// Our half of the safety number: 30 digits in 6 groups of 5.
    pub fingerprint: String,
    /// `umbra://add?...` payload to render as a QR code.
    pub qr_payload: String,
    pub display_name: String,
    /// Olm Curve25519 identity key (unpadded base64).
    pub identity_key: String,
    /// Ed25519 device key (unpadded base64).
    pub ed25519_key: String,
    /// Addresses currently advertised in the QR payload.
    pub addrs: Vec<String>,
}

/// A contact as read from a QR code / link or received in a contact request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContactCard {
    pub peer_id: String,
    /// Olm Curve25519 identity key (unpadded base64).
    pub identity_key: String,
    /// Ed25519 device key (unpadded base64). Must derive `peer_id`.
    pub ed25519_key: String,
    pub addrs: Vec<String>,
    /// Optional display name (`name=` query param).
    pub display_name: Option<String>,
    /// Card v2: `ed25519_sign(ed, "umbra-card-v2" || PeerId bytes || ik)`,
    /// unpadded base64. `None` on legacy (v1) cards. Core verifies it
    /// wherever a card is accepted and rejects the card if it is invalid.
    #[serde(default)]
    pub sig: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contact {
    pub peer_id: String,
    pub display_name: String,
    pub identity_key: String,
    pub ed25519_key: String,
    pub addrs: Vec<String>,
    /// User has compared the safety number / scanned the QR in person.
    pub verified: bool,
    /// 60-digit safety number (12 groups of 5) derived from both parties'
    /// keys. Identical on both devices.
    pub safety_number: String,
    pub added_at_ms: i64,
    /// Currently connected (best effort).
    pub online: bool,
    /// This contact presented a different identity key that is NOT trusted
    /// yet (see `CoreEvent::KeyChanged`, `accept_key_change`, `reject_key_change`).
    pub key_changed: bool,
    /// The untrusted new identity key, if `key_changed`.
    pub pending_identity_key: Option<String>,
    /// We hold a valid card v2 signature binding `identity_key` to the
    /// device key. Legacy contacts become signed on their next `Hello`.
    pub signed_card: bool,
}

/// Where a changed identity key was seen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeyChangeSource {
    /// The peer's `Hello` on a new connection.
    Hello,
    /// The peer's answer to a one-time-key claim.
    KeyClaim,
    /// A scanned QR / contact card passed to `add_contact`.
    ContactCard,
    /// A server roster entry (co-member card) for this peer.
    Roster,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeliveryStatus {
    /// Outgoing: persisted in the outbox and handed to the network layer.
    Sent,
    /// Outgoing: recipient decrypted and stored it (acked).
    Delivered,
    /// Outgoing: gave up (expired in outbox or rejected permanently).
    Failed,
    /// Incoming message.
    Received,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub id: MessageId,
    /// The other party (sender for incoming, recipient for outgoing).
    pub peer_id: String,
    pub outgoing: bool,
    pub body: String,
    /// Sender's timestamp, ms since Unix epoch.
    pub timestamp_ms: i64,
    pub status: DeliveryStatus,
    pub read: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationSummary {
    pub peer_id: String,
    pub display_name: String,
    pub last_message: Option<Message>,
    pub unread_count: u32,
    pub updated_at_ms: i64,
}

/// Call signaling message kinds, carried encrypted by core for `calls/`.
///
/// Wire names are the variant names (`"Offer"`, …, `"Accept"`, `"Busy"`) and
/// must match `calls/lib/src/signaling.dart` (`SignalKindWire.wire`).
/// New variants go at the end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SignalKind {
    Offer,
    Answer,
    IceCandidate,
    Hangup,
    Ringing,
    Reject,
    /// Callee accepted a `Ringing`; the caller sends `Offer` next.
    Accept,
    /// Callee is already in a call.
    Busy,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkInfo {
    pub running: bool,
    pub listen_addrs: Vec<String>,
    pub connected_peers: u32,
    /// Number of `/p2p-circuit` listen addresses (active relay reservations).
    pub relay_reservations: u32,
}

/// The single event stream delivered through `events(sink)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoreEvent {
    MessageReceived {
        message: Message,
    },
    MessageStatus {
        id: MessageId,
        peer_id: String,
        status: DeliveryStatus,
    },
    ContactOnline {
        peer_id: String,
    },
    ContactOffline {
        peer_id: String,
    },
    /// A peer that is not (yet) a contact said hello. Accept by calling
    /// `add_contact(card, verified)`; ignore to decline.
    ContactRequest {
        card: ContactCard,
    },
    Signal {
        peer_id: String,
        call_id: String,
        kind: SignalKind,
        payload: String,
    },
    NetworkStatus {
        info: NetworkInfo,
    },
    /// A known contact presented a different identity key. The new key is
    /// rejected and NOT trusted; messages keep using the old key. The contact
    /// is flagged (`Contact.key_changed`) until the app calls
    /// `accept_key_change` or `reject_key_change`. Emitted once per distinct
    /// new key.
    KeyChanged {
        peer_id: String,
        old_identity_key: String,
        new_identity_key: String,
        old_safety_number: String,
        new_safety_number: String,
        source: KeyChangeSource,
    },
    /// Something happened in a server (see [`ServerEvent`]).
    Server {
        server_id: String,
        event: ServerEvent,
    },
}

// ---- servers (docs/servers.md, docs/core-api.md#servers) ----

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerRole {
    Owner,
    Admin,
    Member,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerStatus {
    /// Normal.
    Active,
    /// `join_server` sent; waiting for the first manifest (or approval).
    Joining,
    /// Two different manifests with the same seq were seen: read-only.
    Frozen,
    /// We were kicked or banned (`removed_reason`), or the join was refused.
    Removed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemberLeftReason {
    /// Left, or dropped from the roster by the owner without a ban.
    Left,
    Kicked,
    Banned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerInfo {
    pub server_id: String,
    pub name: String,
    /// Fingerprint of the owner key the server is anchored on (show it in
    /// the join preview and server settings).
    pub owner_fingerprint: String,
    /// Our role; `None` while joining or after removal.
    pub my_role: Option<ServerRole>,
    pub status: ServerStatus,
    pub removed_reason: Option<MemberLeftReason>,
    /// Current manifest seq.
    pub seq: u64,
    pub member_count: u32,
    pub channel_count: u32,
    /// Roles that may create invites (owner always may).
    pub invite_roles: Vec<ServerRole>,
    /// History for new members: `true` = shared (default), `false` = from join.
    pub history_shared: bool,
    /// Host ciphertext retention, days (at most 30).
    pub retention_days: u32,
    /// We hold the owner key on this device.
    pub is_owner_device: bool,
    /// Our display name in this server; `None` = the global one. Same
    /// (default) identity everywhere; only the name is per server.
    pub my_display_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerChannel {
    /// 16 random bytes, base64url.
    pub id: String,
    pub name: String,
    pub can_read: bool,
    pub can_post: bool,
    pub read_roles: Vec<ServerRole>,
    pub post_roles: Vec<ServerRole>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerMember {
    pub peer_id: String,
    pub display_name: String,
    pub role: ServerRole,
    pub added_by: String,
    pub is_self: bool,
    pub is_contact: bool,
}

/// Returned once by `create_server`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreatedServer {
    pub server: ServerInfo,
    /// Generated recovery secret (base64url), shown to the user once and
    /// never stored by core. `None` if recovery keys were supplied.
    pub recovery_secret: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct InviteOptions {
    /// Default 7 days.
    pub ttl_secs: Option<u64>,
    /// Default 1, at most 1000.
    pub max_uses: Option<u32>,
    /// The inviter must call `approve_join` for each joiner.
    pub needs_approval: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreatedInvite {
    pub invite_id: String,
    /// `umbra://join?...`; contains the bearer secret.
    pub link: String,
    pub expires_at_ms: i64,
    pub max_uses: u32,
}

/// What `parse_invite` shows before joining.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvitePreview {
    pub server_id: String,
    pub server_name: String,
    pub owner_fingerprint: String,
    pub inviter: ContactCard,
    pub invite_id: String,
    pub expires_at_ms: i64,
    pub needs_approval: bool,
    /// Host PeerIds named in the invite.
    pub hosts: Vec<String>,
    pub expired: bool,
    pub already_member: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JoinRequest {
    pub server_id: String,
    pub card: ContactCard,
    pub invite_id: String,
    pub requested_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerEvent {
    /// The first manifest arrived after `join_server`: we're in.
    Joined,
    /// A new manifest or roster op was applied.
    Updated {
        seq: u64,
    },
    MemberJoined {
        peer_id: String,
    },
    MemberLeft {
        peer_id: String,
        reason: MemberLeftReason,
    },
    /// Someone used an approval-required invite of ours.
    JoinRequest {
        request: JoinRequest,
    },
    /// A manifest older than the newest seen was offered and refused.
    ManifestRollback {
        have: u64,
        got: u64,
    },
    /// Two different manifests for one seq: the server is frozen read-only.
    Forked {
        seq: u64,
    },
    /// We are no longer in the server.
    Removed {
        reason: MemberLeftReason,
    },
}
