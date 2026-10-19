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
    expect(contact.safetyNumber.split(' '), hasLength(12));

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

  test('signal offer is not interpreted by the mock store', () async {
    await boot();
    await core.addContact(MockUmbraCore.mockBot, verified: false);
    await core.sendSignal(
      peerId: MockUmbraCore.botPeerId,
      callId: 'call-1',
      kind: SignalKind.offer,
      payload: 'sdp',
    );
    await _flush();
    final signals = events.whereType<SignalEvent>().toList();
    expect(signals.map((e) => e.kind), [SignalKind.ringing, SignalKind.answer]);
    expect(signals.last.payload, 'mock-answer-sdp');
  });

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
