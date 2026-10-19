import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:umbra/src/core/mock_umbra_core.dart';
import 'package:umbra/src/keystore/db_key_store.dart';
import 'package:umbra/src/settings/settings_store.dart';
import 'package:umbra/src/state/umbra_controller.dart';
import 'package:umbra/src/ui/umbra_app.dart';

void main() {
  testWidgets('onboarding, accept, and echo', (tester) async {
    final controller = UmbraController(
      core: MockUmbraCore(timing: MockTiming.instant),
      keyStore: MemoryDbKeyStore(),
      settings: SettingsStore(),
      supportDirectory: () async => '/tmp/umbra-widget',
    );
    await tester.pumpWidget(UmbraApp(controller: controller));
    await tester.pumpAndSettle();

    expect(find.text('Talk in the shadows.'), findsOneWidget);
    await tester.enterText(find.byType(TextField), 'Nyx');
    await tester.tap(find.text('Continue'));
    await tester.pumpAndSettle();

    expect(find.text('Mock Bot'), findsWidgets);
    await tester.tap(find.text('Accept'));
    await tester.pumpAndSettle();

    await tester.tap(find.text('Mock Bot').first);
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(TextField), 'hi');
    await tester.tap(find.byIcon(Icons.send));
    await tester.pumpAndSettle();

    expect(find.text('echo: hi'), findsOneWidget);
    expect(find.textContaining('Delivered'), findsOneWidget);

    await tester.enterText(find.byType(TextField), 'again');
    await tester.testTextInput.receiveAction(TextInputAction.send);
    await tester.pumpAndSettle();
    expect(
      tester.widget<TextField>(find.byType(TextField)).focusNode!.hasFocus,
      isTrue,
    );
    await tester.pageBack();
    await tester.pumpAndSettle();
    expect(find.text('1'), findsNothing);
  });

  testWidgets('safety number change can be accepted', (tester) async {
    final core = MockUmbraCore(timing: MockTiming.instant);
    final controller = UmbraController(
      core: core,
      keyStore: MemoryDbKeyStore(),
      settings: SettingsStore(),
      supportDirectory: () async => '/tmp/umbra-widget-key',
    );
    await tester.pumpWidget(UmbraApp(controller: controller));
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(TextField), 'Nyx');
    await tester.tap(find.text('Continue'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Accept'));
    await tester.pumpAndSettle();

    core.presentKeyChange(
      peerId: MockUmbraCore.botPeerId,
      newIdentityKey: 'EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE',
      newSafetyNumber:
          '11111 22222 33333 44444 55555 66666 77777 88888 99999 00000 12121 34343',
    );
    await tester.pump();
    expect(find.text('Safety number changed'), findsOneWidget);
    await tester.tap(find.widgetWithText(FilledButton, 'Accept'));
    await tester.pumpAndSettle();
    expect(find.text('Safety number changed'), findsNothing);
    expect(controller.contacts.single.verified, isFalse);
  });

  testWidgets('tor mode shows the relay warning', (tester) async {
    final controller = UmbraController(
      core: MockUmbraCore(timing: MockTiming.instant),
      keyStore: MemoryDbKeyStore(),
      settings: SettingsStore()..torSocks = '127.0.0.1:9050',
      supportDirectory: () async => '/tmp/umbra-widget-tor',
    );
    controller.attachCalls = (self) {
      controller.setCallsBlockedByTor(true);
    };
    await tester.pumpWidget(UmbraApp(controller: controller));
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(TextField), 'Nyx');
    await tester.tap(find.text('Continue'));
    await tester.pumpAndSettle();
    expect(find.text('Calls over Tor need a relay server.'), findsOneWidget);
    controller.noteMissedCall(
      peerId: 'peer-x',
      callId: 'call-x',
      reason: 'tor_relay_required',
    );
    await tester.pump();
    expect(find.text('Someone tried to call'), findsOneWidget);
    expect(find.text('peer-x'), findsOneWidget);
  });
}
