import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:umbra/src/core/mock_umbra_core.dart';
import 'package:umbra/src/models/core_types.dart';

Future<void> _flush() async {
  for (var i = 0; i < 8; i++) {
    await Future<void>.delayed(Duration.zero);
  }
}

void main() {
  late MockUmbraCore core;
  final events = <CoreEvent>[];

  setUp(() async {
    events.clear();
    core = MockUmbraCore(timing: MockTiming.instant);
  });

  Future<void> boot() async {
    await core.init(
      dataDir: '/tmp/umbra',
      dbKey: Uint8List(32),
      cfg: const CoreConfig(mock: true),
    );
    core.listen(events.add);
    await core.startNetwork();
    await _flush();
  }

  test('subscribing before start_network receives the mock request', () async {
    await boot();
    expect(events.whereType<NetworkStatus>().single.info.running, isTrue);
    final request = events.whereType<ContactRequest>().single;
    expect(request.card.displayName, 'Mock Bot');
    expect(request.card.peerId, MockUmbraCore.botPeerId);
  });

  test('echoes text and marks it delivered', () async {
    await boot();
    final contact = await core.addContact(
      MockUmbraCore.mockBot,
      verified: false,
    );
    expect(contact.verified, isFalse);
    final groups = contact.safetyNumber.split(' ');
    expect(groups, hasLength(12));
    final self = await core.selfInfo();
    expect(groups.take(6).join(' '), isNot(self.fingerprint));
    expect((await core.networkStatus()).connectedPeers, 1);

    final id = await core.sendText(contact.peerId, 'hi');
    await _flush();

    final statuses = events.whereType<MessageStatusChanged>().toList();
    expect(statuses.map((e) => e.status), contains(DeliveryStatus.sent));
    expect(statuses.map((e) => e.status), contains(DeliveryStatus.delivered));
    expect(events.whereType<MessageReceived>().single.message.body, 'echo: hi');

    final messages = await core.listMessages(contact.peerId);
    expect(messages, hasLength(2));
    expect(messages.first.body, 'echo: hi');
    expect((await core.listConversations()).single.unreadCount, 1);
    await core.markRead(contact.peerId, id);
    // The echo is newer than the outgoing id, so mark the newest.
    await core.markRead(contact.peerId, messages.first.id);
    expect((await core.listConversations()).single.unreadCount, 0);
  });

  test('offer answers with sdp and ringing answers with accept', () async {
    await boot();
    await core.addContact(MockUmbraCore.mockBot, verified: false);
    await core.sendSignal(
      peerId: MockUmbraCore.botPeerId,
      callId: 'call-1',
      kind: SignalKind.offer,
      payload: 'sdp',
    );
    await core.sendSignal(
      peerId: MockUmbraCore.botPeerId,
      callId: 'call-1',
      kind: SignalKind.ringing,
      payload: '',
    );
    await _flush();
    final signals = events.whereType<SignalEvent>().toList();
    expect(signals.map((e) => e.kind), [SignalKind.answer, SignalKind.accept]);
    expect(signals.first.payload, 'mock-answer-sdp');
    expect(signals.last.payload, isEmpty);
  });

  test(
    'accept key change clears verified and reject drops the warning',
    () async {
      await boot();
      await core.addContact(MockUmbraCore.mockBot, verified: true);
      expect(
        core.acceptKeyChange(MockUmbraCore.botPeerId),
        throwsA(
          isA<UmbraException>().having(
            (e) => e.message,
            'message',
            contains('no pending key change'),
          ),
        ),
      );
      core.presentKeyChange(
        peerId: MockUmbraCore.botPeerId,
        newIdentityKey: 'EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE',
        newSafetyNumber:
            '11111 22222 33333 44444 55555 66666 77777 88888 99999 00000 12121 34343',
      );
      final changed = (await core.listContacts()).single;
      expect(changed.keyChanged, isTrue);
      final accepted = await core.acceptKeyChange(MockUmbraCore.botPeerId);
      expect(accepted.verified, isFalse);
      expect(accepted.keyChanged, isFalse);
      expect(
        accepted.identityKey,
        'EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE',
      );
    },
  );

  test('refuses a db_key that is not 32 bytes', () async {
    expect(
      core.init(
        dataDir: '/tmp/umbra',
        dbKey: Uint8List(16),
        cfg: const CoreConfig(mock: true),
      ),
      throwsA(isA<UmbraException>()),
    );
  });
}
