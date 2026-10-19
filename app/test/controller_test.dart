import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:umbra/src/core/mock_umbra_core.dart';
import 'package:umbra/src/core/umbra_core.dart';
import 'package:umbra/src/keystore/db_key_store.dart';
import 'package:umbra/src/models/core_types.dart';
import 'package:umbra/src/settings/settings_store.dart';
import 'package:umbra/src/state/umbra_controller.dart';

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
  Future<void> startNetwork() {
    order.add('startNetwork');
    return inner.startNetwork();
  }

  @override
  Future<void> stopNetwork() => inner.stopNetwork();

  void emitForTest(CoreEvent event) => inner.emitForTest(event);
}
