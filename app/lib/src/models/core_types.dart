/// Dart mirror of `core/src/api/types.rs`.
///
/// Private keys never appear here. These are the decrypted messages, contact
/// cards, and opaque ids that the FFI contract allows across the boundary.
library;

enum DeliveryStatus { sent, delivered, failed, received }

/// New variants stay at the end so they match the Rust enum order.
enum SignalKind {
  offer,
  answer,
  iceCandidate,
  hangup,
  ringing,
  reject,
  accept,
  busy,
}

/// Where a changed identity key was seen.
enum KeyChangeSource { hello, keyClaim, contactCard, roster }

/// Wire names from `core` `SignalKind` (`Offer`, `Answer`, …).
///
/// This is a name, not call handling. `CallController.handleCoreSignal`
/// (`peerId`, `callId`, `kindWire`, `payload`) is the only ingress.
String coreSignalKindWire(SignalKind kind) {
  return switch (kind) {
    SignalKind.offer => 'Offer',
    SignalKind.answer => 'Answer',
    SignalKind.iceCandidate => 'IceCandidate',
    SignalKind.hangup => 'Hangup',
    SignalKind.ringing => 'Ringing',
    SignalKind.reject => 'Reject',
    SignalKind.accept => 'Accept',
    SignalKind.busy => 'Busy',
  };
}

class CoreConfig {
  const CoreConfig({
    this.bootstrapAddrs = const [],
    this.relayAddrs = const [],
    this.enableMdns = false,
    this.torSocks,
    this.listenAddrs = const [],
    this.mock = true,
  });

  final List<String> bootstrapAddrs;
  final List<String> relayAddrs;
  final bool enableMdns;
  final String? torSocks;
  final List<String> listenAddrs;

  /// In-memory mock backend. Default for UI development.
  final bool mock;
}

class SelfInfo {
  const SelfInfo({
    required this.peerId,
    required this.fingerprint,
    required this.qrPayload,
    required this.displayName,
    required this.identityKey,
    required this.ed25519Key,
    required this.addrs,
  });

  final String peerId;
  final String fingerprint;
  final String qrPayload;
  final String displayName;
  final String identityKey;
  final String ed25519Key;
  final List<String> addrs;

  SelfInfo copyWith({
    String? displayName,
    String? qrPayload,
    List<String>? addrs,
  }) {
    return SelfInfo(
      peerId: peerId,
      fingerprint: fingerprint,
      qrPayload: qrPayload ?? this.qrPayload,
      displayName: displayName ?? this.displayName,
      identityKey: identityKey,
      ed25519Key: ed25519Key,
      addrs: addrs ?? this.addrs,
    );
  }
}

class ContactCard {
  const ContactCard({
    required this.peerId,
    required this.identityKey,
    required this.ed25519Key,
    required this.addrs,
    this.displayName,
    this.sig,
  });

  final String peerId;
  final String identityKey;
  final String ed25519Key;
  final List<String> addrs;
  final String? displayName;

  /// Card v2 signature (QR `sig=`). Opaque to the app: pass it back to core
  /// unchanged so core can verify the card on import.
  final String? sig;

  String get label {
    final name = displayName?.trim();
    if (name != null && name.isNotEmpty) return name;
    return peerId;
  }
}

class Contact {
  const Contact({
    required this.peerId,
    required this.displayName,
    required this.identityKey,
    required this.ed25519Key,
    required this.addrs,
    required this.verified,
    required this.safetyNumber,
    required this.addedAtMs,
    required this.online,
    this.keyChanged = false,
    this.pendingIdentityKey,
  });

  final String peerId;
  final String displayName;
  final String identityKey;
  final String ed25519Key;
  final List<String> addrs;
  final bool verified;
  final String safetyNumber;
  final int addedAtMs;
  final bool online;

  /// A new identity key was presented and is not trusted yet.
  final bool keyChanged;

  /// The untrusted key, when [keyChanged] is set.
  final String? pendingIdentityKey;

  String get label => displayName.trim().isEmpty ? peerId : displayName;

  Contact copyWith({
    bool? verified,
    bool? online,
    String? displayName,
    String? identityKey,
    String? safetyNumber,
    bool? keyChanged,
    Object? pendingIdentityKey = _keepPending,
  }) {
    return Contact(
      peerId: peerId,
      displayName: displayName ?? this.displayName,
      identityKey: identityKey ?? this.identityKey,
      ed25519Key: ed25519Key,
      addrs: addrs,
      verified: verified ?? this.verified,
      safetyNumber: safetyNumber ?? this.safetyNumber,
      addedAtMs: addedAtMs,
      online: online ?? this.online,
      keyChanged: keyChanged ?? this.keyChanged,
      pendingIdentityKey: identical(pendingIdentityKey, _keepPending)
          ? this.pendingIdentityKey
          : pendingIdentityKey as String?,
    );
  }
}

const _keepPending = Object();

class Message {
  const Message({
    required this.id,
    required this.peerId,
    required this.outgoing,
    required this.body,
    required this.timestampMs,
    required this.status,
    required this.read,
  });

  final String id;
  final String peerId;
  final bool outgoing;
  final String body;
  final int timestampMs;
  final DeliveryStatus status;
  final bool read;

  Message copyWith({DeliveryStatus? status, bool? read}) {
    return Message(
      id: id,
      peerId: peerId,
      outgoing: outgoing,
      body: body,
      timestampMs: timestampMs,
      status: status ?? this.status,
      read: read ?? this.read,
    );
  }
}

class ConversationSummary {
  const ConversationSummary({
    required this.peerId,
    required this.displayName,
    required this.lastMessage,
    required this.unreadCount,
    required this.updatedAtMs,
  });

  final String peerId;
  final String displayName;
  final Message? lastMessage;
  final int unreadCount;
  final int updatedAtMs;

  String get label => displayName.trim().isEmpty ? peerId : displayName;
}

class NetworkInfo {
  const NetworkInfo({
    this.running = false,
    this.listenAddrs = const [],
    this.connectedPeers = 0,
    this.relayReservations = 0,
  });

  final bool running;
  final List<String> listenAddrs;
  final int connectedPeers;
  final int relayReservations;
}

sealed class CoreEvent {
  const CoreEvent();
}

class MessageReceived extends CoreEvent {
  const MessageReceived(this.message);
  final Message message;
}

class MessageStatusChanged extends CoreEvent {
  const MessageStatusChanged({
    required this.id,
    required this.peerId,
    required this.status,
  });

  final String id;
  final String peerId;
  final DeliveryStatus status;
}

class ContactOnline extends CoreEvent {
  const ContactOnline(this.peerId);
  final String peerId;
}

class ContactOffline extends CoreEvent {
  const ContactOffline(this.peerId);
  final String peerId;
}

class ContactRequest extends CoreEvent {
  const ContactRequest(this.card);
  final ContactCard card;
}

class SignalEvent extends CoreEvent {
  const SignalEvent({
    required this.peerId,
    required this.callId,
    required this.kind,
    required this.payload,
  });

  final String peerId;
  final String callId;
  final SignalKind kind;
  final String payload;
}

class NetworkStatus extends CoreEvent {
  const NetworkStatus(this.info);
  final NetworkInfo info;
}

class KeyChanged extends CoreEvent {
  const KeyChanged({
    required this.peerId,
    required this.oldIdentityKey,
    required this.newIdentityKey,
    required this.oldSafetyNumber,
    required this.newSafetyNumber,
    required this.source,
  });

  final String peerId;
  final String oldIdentityKey;
  final String newIdentityKey;
  final String oldSafetyNumber;
  final String newSafetyNumber;
  final KeyChangeSource source;
}

/// Something changed in a server. The app has no server UI yet, so only
/// the server id is kept; the details are in core's `ServerEvent`.
class ServerActivity extends CoreEvent {
  const ServerActivity(this.serverId);
  final String serverId;
}

/// Safety numbers from a [KeyChanged] event, kept so the warning can show
/// both values after the contact list reloads.
class MissedCallNotice {
  const MissedCallNotice({
    required this.peerId,
    required this.callId,
    required this.reason,
  });

  final String peerId;
  final String callId;
  final String reason;
}

class KeyChangeNotice {
  const KeyChangeNotice({
    required this.peerId,
    required this.oldIdentityKey,
    required this.newIdentityKey,
    required this.oldSafetyNumber,
    required this.newSafetyNumber,
    required this.source,
  });

  final String peerId;
  final String oldIdentityKey;
  final String newIdentityKey;
  final String oldSafetyNumber;
  final String newSafetyNumber;
  final KeyChangeSource source;
}

class UmbraException implements Exception {
  const UmbraException(this.message);
  final String message;

  @override
  String toString() => message;
}
