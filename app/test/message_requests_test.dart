import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:umbra/src/core/frb_umbra_core.dart';
import 'package:umbra/src/core/mock_umbra_core.dart';
import 'package:umbra/src/keystore/db_key_store.dart';
import 'package:umbra/src/models/core_types.dart';
import 'package:umbra/src/settings/settings_store.dart';
import 'package:umbra/src/state/umbra_controller.dart';
import 'package:umbra/src/ui/message_requests_screen.dart';
import 'package:umbra/src/ui/umbra_app.dart';

const _body = 'hello from the server';

const _card = ContactCard(
  peerId: '12D3KooWUmbraCoMemberPeer0000000000000000001',
  identityKey: 'EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE',
  ed25519Key: 'FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF',
  addrs: [],
  displayName: 'Ash',
);

Future<void> _turns() async {
  for (var i = 0; i < 8; i++) {
    await Future<void>.delayed(Duration.zero);
  }
}

Future<UmbraController> _boot(MockUmbraCore core, String dir) async {
  final controller = UmbraController(
    core: core,
    keyStore: MemoryDbKeyStore(),
    settings: SettingsStore(),
    supportDirectory: () async => dir,
  );
  await controller.bootstrap();
  return controller;
}

void main() {
  test('count label is words, not a bare unread number', () {
    expect(messageRequestCountLabel(1), '1 request');
    expect(messageRequestCountLabel(2), '2 requests');
    expect(messageRequestCountLabel(1), isNot('1'));
  });

  test('real core stubs message requests until the FFI has them', () async {
    final core = FrbUmbraCore();
    expect(await core.listMessageRequests(), isEmpty);
    expect(
      core.acceptMessageRequest('req'),
      throwsA(
        isA<UmbraException>().having(
          (err) => err.message,
          'message',
          'message requests are not in the core FFI yet',
        ),
      ),
    );
    expect(core.dismissMessageRequest('req'), throwsA(isA<UmbraException>()));
  });

  test('a request stays out of the inbox until accept', () async {
    final core = MockUmbraCore(timing: MockTiming.instant);
    final controller = await _boot(core, '/tmp/umbra-requests');
    core.presentMessageRequest(
      card: _card,
      serverId: 'server-1',
      serverName: 'Night Market',
      body: _body,
      id: 'req-1',
    );
    core.presentMessageRequest(
      card: const ContactCard(
        peerId: '12D3KooWUmbraCoMemberPeer0000000000000000002',
        identityKey: 'GGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGG',
        ed25519Key: 'HHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHHH',
        addrs: [],
        displayName: 'Briar',
      ),
      serverId: 'server-1',
      serverName: 'Night Market',
      body: 'second',
      id: 'req-2',
    );
    await _turns();

    expect(controller.messageRequests, hasLength(2));
    expect(
      messageRequestCountLabel(controller.messageRequests.length),
      '2 requests',
    );
    expect(controller.contacts, isEmpty);
    expect(controller.conversations, isEmpty);

    await controller.dismissMessageRequest('req-2');
    expect(controller.messageRequests, hasLength(1));
    expect(controller.contacts, isEmpty);

    await controller.acceptMessageRequest('req-1');
    expect(controller.messageRequests, isEmpty);
    expect(controller.contacts.single.peerId, _card.peerId);
    expect(controller.contacts.single.verified, isFalse);
    expect(controller.conversations.single.unreadCount, 1);
    final stored = controller.messagesFor(_card.peerId).single;
    expect(stored.body, _body);
    expect(stored.read, isFalse);
    expect(
      controller.messagesFor('12D3KooWUmbraCoMemberPeer0000000000000000002'),
      isEmpty,
    );
  });

  testWidgets('collapsed row hides the text until the list is opened', (
    tester,
  ) async {
    final core = MockUmbraCore(timing: MockTiming.instant);
    final controller = UmbraController(
      core: core,
      keyStore: MemoryDbKeyStore(),
      settings: SettingsStore(),
      supportDirectory: () async => '/tmp/umbra-requests-widget',
    );
    await tester.pumpWidget(UmbraApp(controller: controller));
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(TextField), 'Nyx');
    await tester.tap(find.text('Continue'));
    await tester.pumpAndSettle();

    expect(find.text('Message requests'), findsNothing);

    core.presentMessageRequest(
      card: _card,
      serverId: 'server-1',
      serverName: 'Night Market',
      body: _body,
      id: 'req-ui',
    );
    await tester.pump();
    await tester.pumpAndSettle();

    expect(find.text('Message requests'), findsOneWidget);
    expect(find.text('1 request'), findsOneWidget);
    expect(find.text(_body), findsNothing);
    expect(find.text('From Night Market'), findsNothing);
    expect(find.text('Ash'), findsNothing);
    expect(find.text('1'), findsNothing);

    await tester.tap(find.text('Message requests'));
    await tester.pumpAndSettle();
    expect(find.text('Ash'), findsOneWidget);
    expect(find.text('From Night Market'), findsOneWidget);
    expect(find.text(_body), findsOneWidget);

    await tester.tap(
      find.descendant(
        of: find.byType(MessageRequestsScreen),
        matching: find.widgetWithText(FilledButton, 'Accept'),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.text('No message requests.'), findsOneWidget);
    expect(controller.contacts.single.verified, isFalse);
    expect(controller.messagesFor(_card.peerId).single.read, isFalse);

    await tester.pageBack();
    await tester.pumpAndSettle();
    expect(find.text('1 request'), findsNothing);
    expect(find.text('1'), findsOneWidget);
    expect(find.text(_body), findsOneWidget);
  });
}
