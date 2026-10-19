import 'dart:async';
import 'dart:convert';
import 'dart:math';
import 'dart:typed_data';

import '../models/core_types.dart';
import 'qr.dart';
import 'umbra_core.dart';

/// Delays copied from `core/src/mock.rs`.
class MockTiming {
  const MockTiming({
    this.contactRequest = const Duration(seconds: 2),
    this.onlineAfterAdd = const Duration(milliseconds: 300),
    this.delivered = const Duration(milliseconds: 400),
    this.echoAfterDelivered = const Duration(milliseconds: 600),
    this.ringing = const Duration(milliseconds: 300),
    this.answer = const Duration(milliseconds: 1200),
    this.hangupEcho = const Duration(milliseconds: 100),
  });

  final Duration contactRequest;
  final Duration onlineAfterAdd;
  final Duration delivered;
  final Duration echoAfterDelivered;
  final Duration ringing;
  final Duration answer;
  final Duration hangupEcho;

  static const instant = MockTiming(
    contactRequest: Duration.zero,
    onlineAfterAdd: Duration.zero,
    delivered: Duration.zero,
    echoAfterDelivered: Duration.zero,
    ringing: Duration.zero,
    answer: Duration.zero,
    hangupEcho: Duration.zero,
  );
}

/// In-memory stand-in for `CoreConfig { mock: true }`.
///
/// Same visible behavior as `core/src/mock.rs`: a Mock Bot request, echo
/// replies, and a fake call answer. No disk and no network. Cryptographic
/// QR checks stay in Rust; [parseQr] here only checks the link shape.
class MockUmbraCore implements UmbraCore {
  MockUmbraCore({this.timing = const MockTiming()});

  final MockTiming timing;

  static const selfPeerId = '12D3KooWUmbraLocalDeviceMockPeer00000000000001';
  static const selfIdentityKey = 'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA';
  static const selfEd25519Key = 'BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB';
  static const botPeerId = '12D3KooWUmbraMockBotPeer0000000000000000000001';
  static const botIdentityKey = 'CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC';
  static const botEd25519Key = 'DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD';

  static final ContactCard mockBot = ContactCard(
    peerId: botPeerId,
    identityKey: botIdentityKey,
    ed25519Key: botEd25519Key,
    addrs: ['/ip4/192.0.2.1/tcp/4001/p2p/$botPeerId'],
    displayName: 'Mock Bot',
  );

  final _rng = Random();
  var _lastTs = 0;
  String? _dataDir;
  SelfInfo? _self;
  String _fpDigits = '';
  final List<Contact> _contacts = [];
  final List<Message> _messages = [];
  NetworkInfo _network = const NetworkInfo();
  void Function(CoreEvent event)? _listener;
  final List<CoreEvent> _buffered = [];
  var _hasListener = false;

  /// Test hook. Events before [listen] are buffered, matching a subscriber
  /// that attaches after `init` and before `start_network`.
  void emitForTest(CoreEvent event) => _emit(event);

  @override
  Future<SelfInfo> init({
    required String dataDir,
    required Uint8List dbKey,
    required CoreConfig cfg,
  }) async {
    if (dbKey.length != 32) {
      throw const UmbraException('db_key must be 32 bytes');
    }
    if (_dataDir != null) {
      if (_dataDir != dataDir) {
        throw const UmbraException(
          'core already initialized with a different data_dir',
        );
      }
      return _self!;
    }
    _dataDir = dataDir;
    _fpDigits = _digits(selfPeerId, 30);
    final card = ContactCard(
      peerId: selfPeerId,
      identityKey: selfIdentityKey,
      ed25519Key: selfEd25519Key,
      addrs: ['/ip4/192.0.2.10/tcp/4001/p2p/$selfPeerId'],
      displayName: null,
    );
    _self = SelfInfo(
      peerId: selfPeerId,
      fingerprint: _group(_fpDigits, 6),
      qrPayload: buildUmbraQr(card),
      displayName: '',
      identityKey: selfIdentityKey,
      ed25519Key: selfEd25519Key,
      addrs: card.addrs,
    );
    _later(timing.contactRequest, ContactRequest(mockBot));
    return _self!;
  }

  @override
  void listen(void Function(CoreEvent event) onEvent) {
    _listener = onEvent;
    _hasListener = true;
    if (_buffered.isEmpty) return;
    final pending = List<CoreEvent>.of(_buffered);
    _buffered.clear();
    for (final event in pending) {
      onEvent(event);
    }
  }

  @override
  Future<void> startNetwork() async {
    _requireInit();
    _network = NetworkInfo(
      running: true,
      listenAddrs: const [
        '/ip4/192.0.2.10/tcp/4001',
        '/ip4/192.0.2.10/udp/4001/quic-v1',
      ],
      connectedPeers: _contacts.length,
      relayReservations: 0,
    );
    _emit(NetworkStatus(_network));
  }

  @override
  Future<void> stopNetwork() async {
    _requireInit();
    _network = const NetworkInfo();
    _emit(NetworkStatus(_network));
  }

  @override
  Future<NetworkInfo> networkStatus() async {
    _requireInit();
    return _network;
  }

  @override
  Future<SelfInfo> selfInfo() async {
    _requireInit();
    return _self!;
  }

  @override
  Future<void> setDisplayName(String name) async {
    _requireInit();
    final card = ContactCard(
      peerId: _self!.peerId,
      identityKey: _self!.identityKey,
      ed25519Key: _self!.ed25519Key,
      addrs: _self!.addrs,
      displayName: name,
    );
    _self = _self!.copyWith(displayName: name, qrPayload: buildUmbraQr(card));
  }

  @override
  Future<ContactCard> parseQr(String payload) async => parseUmbraQr(payload);

  @override
  Future<Contact> addContact(ContactCard card, {required bool verified}) async {
    _requireInit();
    parseUmbraQr(buildUmbraQr(card));
    final theirs = _digits(card.peerId, 30);
    final contact = Contact(
      peerId: card.peerId,
      displayName: card.displayName?.trim() ?? '',
      identityKey: card.identityKey,
      ed25519Key: card.ed25519Key,
      addrs: card.addrs,
      verified: verified,
      safetyNumber: _safety(_fpDigits, theirs),
      addedAtMs: _now(),
      online: true,
    );
    _contacts.removeWhere((c) => c.peerId == contact.peerId);
    _contacts.add(contact);
    _later(timing.onlineAfterAdd, ContactOnline(contact.peerId));
    return contact;
  }

  @override
  Future<List<Contact>> listContacts() async {
    _requireInit();
    return List<Contact>.unmodifiable(_contacts);
  }

  @override
  Future<void> setVerified(String peerId, {required bool verified}) async {
    _requireInit();
    final index = _contacts.indexWhere((c) => c.peerId == peerId);
    if (index < 0) throw const UmbraException('unknown contact');
    _contacts[index] = _contacts[index].copyWith(verified: verified);
  }

  @override
  Future<void> removeContact(String peerId) async {
    _requireInit();
    _contacts.removeWhere((c) => c.peerId == peerId);
    _messages.removeWhere((m) => m.peerId == peerId);
  }

  @override
  Future<String> sendText(String peerId, String body) async {
    _requireInit();
    _requireContact(peerId);
    final bytes = utf8.encode(body);
    if (body.trim().isEmpty) {
      throw const UmbraException('message is empty');
    }
    if (bytes.length > 64 * 1024) {
      throw const UmbraException('message body exceeds 64 KiB');
    }
    final id = _uuid();
    final message = Message(
      id: id,
      peerId: peerId,
      outgoing: true,
      body: body,
      timestampMs: _now(),
      status: DeliveryStatus.sent,
      read: true,
    );
    _messages.add(message);
    _emit(
      MessageStatusChanged(id: id, peerId: peerId, status: DeliveryStatus.sent),
    );
    unawaited(_simulateReply(peerId, id, body));
    return id;
  }

  Future<void> _simulateReply(String peerId, String id, String body) async {
    await Future<void>.delayed(timing.delivered);
    final index = _messages.indexWhere((m) => m.id == id);
    if (index >= 0) {
      _messages[index] = _messages[index].copyWith(
        status: DeliveryStatus.delivered,
      );
    }
    _emit(
      MessageStatusChanged(
        id: id,
        peerId: peerId,
        status: DeliveryStatus.delivered,
      ),
    );
    await Future<void>.delayed(timing.echoAfterDelivered);
    final reply = Message(
      id: _uuid(),
      peerId: peerId,
      outgoing: false,
      body: 'echo: $body',
      timestampMs: _now(),
      status: DeliveryStatus.received,
      read: false,
    );
    _messages.add(reply);
    _emit(MessageReceived(reply));
  }

  @override
  Future<List<ConversationSummary>> listConversations() async {
    _requireInit();
    final summaries = _contacts.map((contact) {
      final msgs = _messages.where((m) => m.peerId == contact.peerId).toList();
      Message? last;
      for (final message in msgs) {
        if (last == null || message.timestampMs >= last.timestampMs) {
          last = message;
        }
      }
      final unread = msgs.where((m) => !m.outgoing && !m.read).length;
      return ConversationSummary(
        peerId: contact.peerId,
        displayName: contact.displayName,
        lastMessage: last,
        unreadCount: unread,
        updatedAtMs: last?.timestampMs ?? contact.addedAtMs,
      );
    }).toList();
    summaries.sort((a, b) => b.updatedAtMs.compareTo(a.updatedAtMs));
    return summaries;
  }

  @override
  Future<List<Message>> listMessages(
    String peerId, {
    int? before,
    int limit = 50,
  }) async {
    _requireInit();
    final matched =
        _messages
            .where(
              (m) =>
                  m.peerId == peerId &&
                  (before == null || m.timestampMs < before),
            )
            .toList()
          ..sort((a, b) => b.timestampMs.compareTo(a.timestampMs));
    if (matched.length > limit) {
      return matched.sublist(0, limit);
    }
    return matched;
  }

  @override
  Future<void> markRead(String peerId, String upTo) async {
    _requireInit();
    final target = _messages.where((m) => m.id == upTo).firstOrNull;
    if (target == null) throw const UmbraException('unknown message');
    for (var i = 0; i < _messages.length; i++) {
      final message = _messages[i];
      if (message.peerId == peerId &&
          message.timestampMs <= target.timestampMs) {
        _messages[i] = message.copyWith(read: true);
      }
    }
  }

  @override
  Future<void> sendSignal({
    required String peerId,
    required String callId,
    required SignalKind kind,
    required String payload,
  }) async {
    _requireInit();
    _requireContact(peerId);
    switch (kind) {
      case SignalKind.offer:
        _later(
          timing.ringing,
          SignalEvent(
            peerId: peerId,
            callId: callId,
            kind: SignalKind.ringing,
            payload: '',
          ),
        );
        _later(
          timing.answer,
          SignalEvent(
            peerId: peerId,
            callId: callId,
            kind: SignalKind.answer,
            payload: 'mock-answer-sdp',
          ),
        );
      case SignalKind.hangup:
        _later(
          timing.hangupEcho,
          SignalEvent(
            peerId: peerId,
            callId: callId,
            kind: SignalKind.hangup,
            payload: '',
          ),
        );
      case SignalKind.answer:
      case SignalKind.iceCandidate:
      case SignalKind.ringing:
      case SignalKind.reject:
        break;
    }
  }

  void _requireInit() {
    if (_self == null) {
      throw const UmbraException('core not initialized; call init() first');
    }
  }

  void _requireContact(String peerId) {
    if (_contacts.every((c) => c.peerId != peerId)) {
      throw UmbraException('unknown contact $peerId');
    }
  }

  void _emit(CoreEvent event) {
    if (!_hasListener) {
      _buffered.add(event);
      return;
    }
    _listener?.call(event);
  }

  void _later(Duration delay, CoreEvent event) {
    unawaited(
      Future<void>.delayed(delay, () {
        _emit(event);
      }),
    );
  }

  int _now() {
    var now = DateTime.now().millisecondsSinceEpoch;
    if (now <= _lastTs) now = _lastTs + 1;
    _lastTs = now;
    return now;
  }

  String _uuid() {
    final bytes = List<int>.generate(16, (_) => _rng.nextInt(256));
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    final hex = bytes.map((b) => b.toRadixString(16).padLeft(2, '0')).join();
    return '${hex.substring(0, 8)}-${hex.substring(8, 12)}-'
        '${hex.substring(12, 16)}-${hex.substring(16, 20)}-${hex.substring(20)}';
  }

  static String _digits(String seed, int count) {
    var state = _fnv(seed);
    final raw = StringBuffer();
    while (raw.length < count) {
      state = (state * 1103515245 + 12345) & 0x7fffffff;
      raw.write((state % 100000).toString().padLeft(5, '0'));
    }
    return raw.toString().substring(0, count);
  }

  static String _group(String digits, int groups) {
    return [
      for (var i = 0; i < groups; i++) digits.substring(i * 5, (i + 1) * 5),
    ].join(' ');
  }

  static String _safety(String ours, String theirs) {
    final parts = [ours, theirs]..sort();
    return _group(parts.join(), 12);
  }

  static int _fnv(String seed) {
    var hash = 0x811c9dc5;
    for (final unit in seed.codeUnits) {
      hash ^= unit;
      hash = (hash * 0x01000193) & 0x7fffffff;
    }
    return hash;
  }
}
