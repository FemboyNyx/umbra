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
  });
}
