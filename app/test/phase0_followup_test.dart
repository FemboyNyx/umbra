import 'dart:io';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:umbra/src/core/mock_umbra_core.dart';
import 'package:umbra/src/keystore/db_key_store.dart';
import 'package:umbra/src/models/core_types.dart';
import 'package:umbra/src/settings/settings_store.dart';
import 'package:umbra/src/state/umbra_controller.dart';
import 'package:umbra/src/ui/key_change_banner.dart';
import 'package:umbra/src/ui/umbra_app.dart';

void main() {
  test('android backup rules keep secrets out of backup and transfer', () {
    final manifest = File(
      'android/app/src/main/AndroidManifest.xml',
    ).readAsStringSync();
    final rules = File(
      'android/app/src/main/res/xml/data_extraction_rules.xml',
    ).readAsStringSync();
    final legacy = File(
      'android/app/src/main/res/xml/backup_rules.xml',
    ).readAsStringSync();

    expect(manifest, contains('android:allowBackup="false"'));
    expect(manifest, contains('android:fullBackupOnly="true"'));
    expect(rules, contains('<cloud-backup'));
    expect(rules, contains('<device-transfer'));
    for (final body in [rules, legacy]) {
      for (final name in [
        'umbra.db',
        'epoch',
        'epoch.tmp',
        'db_key.bin',
        'umbra_keystore.xml',
      ]) {
        expect(body, contains(name), reason: name);
      }
    }
  });

  test('roster key changes say they were seen in a server roster', () {
    expect(
      keyChangeSourceCopy(KeyChangeSource.roster),
      'Seen in a server roster.',
    );
  });

  test('a card without sig stays an unsigned contact', () async {
    final core = MockUmbraCore(timing: MockTiming.instant);
    await core.init(
      dataDir: '/tmp/umbra-signed',
      dbKey: Uint8List(32),
      cfg: const CoreConfig(mock: true),
    );
    core.listen((_) {});
    await core.startNetwork();
    final unsigned = await core.addContact(
      MockUmbraCore.mockBot,
      verified: false,
    );
    expect(unsigned.signedCard, isFalse);
    final signed = await core.addContact(
      const ContactCard(
        peerId: '12D3KooWSignedPeer',
        identityKey: 'EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE',
        ed25519Key: 'FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF',
        addrs: [],
        sig: 'abc+def/ghi',
      ),
      verified: false,
    );
    expect(signed.signedCard, isTrue);
  });

  testWidgets('contact detail hints at an unsigned card and a roster change', (
    tester,
  ) async {
    final core = MockUmbraCore(timing: MockTiming.instant);
    final controller = UmbraController(
      core: core,
      keyStore: MemoryDbKeyStore(),
      settings: SettingsStore(),
      supportDirectory: () async => '/tmp/umbra-phase0-ui',
    );
    await tester.pumpWidget(UmbraApp(controller: controller));
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(TextField), 'Nyx');
    await tester.tap(find.text('Continue'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Accept'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Mock Bot').first);
    await tester.pumpAndSettle();
    await tester.tap(find.byIcon(Icons.info_outline));
    await tester.pumpAndSettle();

    expect(find.text('Unsigned (legacy) card'), findsOneWidget);
    expect(find.text('Remove contact'), findsOneWidget);

    core.presentKeyChange(
      peerId: MockUmbraCore.botPeerId,
      newIdentityKey: 'EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE',
      newSafetyNumber:
          '11111 22222 33333 44444 55555 66666 77777 88888 99999 00000 12121 34343',
      source: KeyChangeSource.roster,
    );
    await tester.pump();
    expect(find.text('Safety number changed'), findsWidgets);
    expect(find.text('Seen in a server roster.'), findsWidgets);
  });
}
