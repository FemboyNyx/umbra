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
}
