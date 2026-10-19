import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:umbra/src/core/core_errors.dart';
import 'package:umbra/src/core/mock_umbra_core.dart';
import 'package:umbra/src/core/umbra_core.dart';
import 'package:umbra/src/keystore/db_key_store.dart';
import 'package:umbra/src/models/core_types.dart';
import 'package:umbra/src/settings/settings_store.dart';
import 'package:umbra/src/state/umbra_controller.dart';

Future<void> _turns() async {
  for (var i = 0; i < 8; i++) {
    await Future<void>.delayed(Duration.zero);
  }
}

void main() {
  test('listens before start_network and dedupes contact requests', () async {
    final core = _OrderCore(MockUmbraCore(timing: MockTiming.instant));
    final controller = UmbraController(
      core: core,
      keyStore: MemoryDbKeyStore(),
      settings: SettingsStore(),
      supportDirectory: () async => '/tmp/umbra-app',
    );
    await controller.bootstrap();

    expect(
      core.order.indexOf('listen'),
      lessThan(core.order.indexOf('startNetwork')),
    );
    expect(core.seenKeyLength, 32);

    final card = MockUmbraCore.mockBot;
    core.emitForTest(ContactRequest(card));
    core.emitForTest(ContactRequest(card));
    expect(controller.pending, hasLength(1));

    await controller.acceptRequest(card);
    expect(controller.pending, isEmpty);
    expect(controller.contacts.single.verified, isFalse);
    expect(controller.contacts.single.peerId, card.peerId);
  });

  test(
    'forwards every CoreEvent signal only through handleCoreSignal',
    () async {
      final core = MockUmbraCore(timing: MockTiming.instant);
      final forwarded = <Map<String, String>>[];
      final controller = UmbraController(
        core: core,
        keyStore: MemoryDbKeyStore(),
        settings: SettingsStore(),
        supportDirectory: () async => '/tmp/umbra-app',
      );
      controller.handleCoreSignal =
          ({
            required String peerId,
            required String callId,
            required String kindWire,
            required String payload,
          }) async {
            forwarded.add({
              'peerId': peerId,
              'callId': callId,
              'kindWire': kindWire,
              'payload': payload,
            });
          };

      await controller.bootstrap();
      core.emitForTest(
        const SignalEvent(
          peerId: 'peer-a',
          callId: 'call-1',
          kind: SignalKind.offer,
          payload: '{"sdp":"v=0"}',
        ),
      );
      core.emitForTest(
        const SignalEvent(
          peerId: 'peer-a',
          callId: 'call-1',
          kind: SignalKind.iceCandidate,
          payload: '{"candidate":"c"}',
        ),
      );
      await Future<void>.delayed(Duration.zero);

      expect(forwarded, [
        {
          'peerId': 'peer-a',
          'callId': 'call-1',
          'kindWire': 'Offer',
          'payload': '{"sdp":"v=0"}',
        },
        {
          'peerId': 'peer-a',
          'callId': 'call-1',
          'kindWire': 'IceCandidate',
          'payload': '{"candidate":"c"}',
        },
      ]);
      expect(controller.threads, isEmpty);
    },
  );

  test('unmodifiable db key still finishes bootstrap', () async {
    final frozen = (Uint8List(32)..fillRange(0, 32, 7)).asUnmodifiableView();
    final core = MockUmbraCore(timing: MockTiming.instant);
    final controller = UmbraController(
      core: core,
      keyStore: _FixedKey(frozen),
      settings: SettingsStore(),
      supportDirectory: () async => '/tmp/umbra-key',
    );
    await controller.bootstrap();
    expect(controller.booting, isFalse);
    expect(controller.started, isTrue);
    expect(controller.error, isNull);
    expect(frozen[0], 7);
  });

  test(
    'start_network failure hides listen addresses and clears booting',
    () async {
      final core = _NetFail(MockUmbraCore(timing: MockTiming.instant));
      final controller = UmbraController(
        core: core,
        keyStore: MemoryDbKeyStore(),
        settings: SettingsStore()..mock = false,
        supportDirectory: () async => '/tmp/umbra-net',
      );
      await controller.bootstrap();
      expect(controller.booting, isFalse);
      expect(controller.started, isFalse);
      expect(controller.error, couldNotStartNetwork);
      expect(controller.error, isNot(contains('/ip4/')));
      expect(controller.error, isNot(contains('.onion')));

      await controller.fallBackToMock();
      expect(controller.booting, isFalse);
      expect(controller.started, isTrue);
    },
  );

  test('messages that arrive in an open chat are not unread', () async {
    final core = MockUmbraCore(timing: MockTiming.instant);
    final controller = UmbraController(
      core: core,
      keyStore: MemoryDbKeyStore(),
      settings: SettingsStore(),
      supportDirectory: () async => '/tmp/umbra-read',
    );
    await controller.bootstrap();
    await controller.acceptRequest(MockUmbraCore.mockBot);
    controller.setThreadOpen(MockUmbraCore.botPeerId, open: true);
    await _turns();
    await controller.sendText(MockUmbraCore.botPeerId, 'hi');
    await _turns();
    expect(controller.conversations.single.unreadCount, 0);

    controller.setThreadOpen(MockUmbraCore.botPeerId, open: false);
    await controller.sendText(MockUmbraCore.botPeerId, 'later');
    await _turns();
    expect(controller.conversations.single.unreadCount, 1);
  });

  test('accepting a key change clears verified', () async {
    final core = MockUmbraCore(timing: MockTiming.instant);
    final controller = UmbraController(
      core: core,
      keyStore: MemoryDbKeyStore(),
      settings: SettingsStore(),
      supportDirectory: () async => '/tmp/umbra-keychange',
    );
    await controller.bootstrap();
    await controller.acceptRequest(MockUmbraCore.mockBot);
    await controller.setVerified(MockUmbraCore.botPeerId, verified: true);
    core.presentKeyChange(
      peerId: MockUmbraCore.botPeerId,
      newIdentityKey: 'EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE',
      newSafetyNumber:
          '11111 22222 33333 44444 55555 66666 77777 88888 99999 00000 12121 34343',
    );
    await Future<void>.delayed(Duration.zero);
    expect(controller.keyChangeNotices, hasLength(1));
    expect(
      controller.keyChangeNotices.single.newSafetyNumber,
      contains('11111'),
    );

    await controller.acceptKeyChange(MockUmbraCore.botPeerId);
    expect(controller.keyChangeNotices, isEmpty);
    expect(controller.contacts.single.verified, isFalse);
    expect(controller.contacts.single.keyChanged, isFalse);
    expect(
      controller.contacts.single.identityKey,
      'EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE',
    );
  });
}

class _OrderCore implements UmbraCore {
  _OrderCore(this.inner);

  final MockUmbraCore inner;
  final order = <String>[];
  int? seenKeyLength;

  @override
  Future<Contact> addContact(ContactCard card, {required bool verified}) =>
      inner.addContact(card, verified: verified);

  @override
  Future<SelfInfo> init({
    required String dataDir,
    required Uint8List dbKey,
    required CoreConfig cfg,
  }) {
    order.add('init');
    seenKeyLength = dbKey.length;
    return inner.init(dataDir: dataDir, dbKey: dbKey, cfg: cfg);
  }

  @override
  void listen(void Function(CoreEvent event) onEvent) {
    order.add('listen');
    inner.listen(onEvent);
  }

  @override
  Future<List<Contact>> listContacts() => inner.listContacts();

  @override
  Future<List<ConversationSummary>> listConversations() =>
      inner.listConversations();

  @override
  Future<List<Message>> listMessages(
    String peerId, {
    int? before,
    int limit = 50,
  }) => inner.listMessages(peerId, before: before, limit: limit);

  @override
  Future<void> markRead(String peerId, String upTo) =>
      inner.markRead(peerId, upTo);

  @override
  Future<List<MessageRequest>> listMessageRequests() =>
      inner.listMessageRequests();

  @override
  Future<void> acceptMessageRequest(String id) =>
      inner.acceptMessageRequest(id);

  @override
  Future<void> dismissMessageRequest(String id) =>
      inner.dismissMessageRequest(id);

  @override
  Future<NetworkInfo> networkStatus() => inner.networkStatus();

  @override
  Future<ContactCard> parseQr(String payload) => inner.parseQr(payload);

  @override
  Future<void> removeContact(String peerId) => inner.removeContact(peerId);

  @override
  Future<SelfInfo> selfInfo() => inner.selfInfo();

  @override
  Future<void> sendSignal({
    required String peerId,
    required String callId,
    required SignalKind kind,
    required String payload,
  }) => inner.sendSignal(
    peerId: peerId,
    callId: callId,
    kind: kind,
    payload: payload,
  );

  @override
  Future<String> sendText(String peerId, String body) =>
      inner.sendText(peerId, body);

  @override
  Future<void> setDisplayName(String name) => inner.setDisplayName(name);

  @override
  Future<void> setVerified(String peerId, {required bool verified}) =>
      inner.setVerified(peerId, verified: verified);

  @override
  Future<Contact> acceptKeyChange(String peerId) =>
      inner.acceptKeyChange(peerId);

  @override
  Future<void> rejectKeyChange(String peerId) => inner.rejectKeyChange(peerId);

  @override
  Future<void> startNetwork() {
    order.add('startNetwork');
    return inner.startNetwork();
  }

  @override
  Future<void> stopNetwork() => inner.stopNetwork();

  void emitForTest(CoreEvent event) => inner.emitForTest(event);
}

class _NetFail extends _OrderCore {
  _NetFail(super.inner);

  @override
  Future<void> startNetwork() {
    order.add('startNetwork');
    throw const UmbraException(
      'could not listen on any address: /ip4/0.0.0.0/tcp/0: denied; '
      'abcdef.onion:4001: denied',
    );
  }
}

class _FixedKey implements DbKeyStore {
  _FixedKey(this.bytes);

  final Uint8List bytes;

  @override
  Future<DbKeyMaterial> getOrCreate({required String storageDir}) async {
    return DbKeyMaterial(bytes: bytes, backend: 'test');
  }
}
