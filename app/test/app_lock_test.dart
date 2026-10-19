import 'dart:typed_data';

import 'package:fake_async/fake_async.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:umbra/src/core/mock_umbra_core.dart';
import 'package:umbra/src/keystore/db_key_store.dart';
import 'package:umbra/src/lock/app_lock.dart';
import 'package:umbra/src/lock/lock_copy.dart';
import 'package:umbra/src/lock/platform_lock.dart';
import 'package:umbra/src/models/core_types.dart';
import 'package:umbra/src/settings/settings_store.dart';
import 'package:umbra/src/state/umbra_controller.dart';
import 'package:umbra/src/ui/umbra_app.dart';

Future<void> _turns() async {
  for (var i = 0; i < 8; i++) {
    await Future<void>.delayed(Duration.zero);
  }
}

Future<MockUmbraCore> _core(String dir) async {
  final core = MockUmbraCore(timing: MockTiming.instant);
  await core.init(dataDir: dir, dbKey: Uint8List(32), cfg: const CoreConfig());
  await core.startNetwork();
  return core;
}

class _YesBiometric implements SoftLockBiometric {
  @override
  Future<bool> available() async => true;

  @override
  Future<bool> authenticate() async => true;
}

Message _secret(String body) {
  return Message(
    id: 'm-$body',
    peerId: 'peer',
    outgoing: false,
    body: body,
    timestampMs: 1,
    status: DeliveryStatus.received,
    read: false,
  );
}

void main() {
  test('copy never calls screen security screenshot-proof', () {
    final words = [
      screenSecurityTitle,
      screenSecurityBody,
      linuxCaptureLimit,
      genericCaptureLimit,
      softLockBody,
      hardLockBody,
      newMessageWhileLocked,
    ].join('\n').toLowerCase();
    expect(words, isNot(contains('screenshot-proof')));
    expect(words, isNot(contains('screenshot proof')));
    expect(screenSecurityTitle, 'Screen security');
    expect(newMessageWhileLocked, 'New message');
  });

  test('soft lock waits one minute in the background', () {
    fakeAsync((async) {
      final settings = SettingsStore()..softLock = true;
      final lock = AppLock(
        settings: settings,
        core: () => MockUmbraCore(),
        softAfter: const Duration(minutes: 1),
      );
      lock.noteLifecycle(AppLifecycleState.paused);
      async.elapse(const Duration(seconds: 59));
      expect(lock.softLocked, isFalse);
      async.elapse(const Duration(seconds: 1));
      expect(lock.softLocked, isTrue);
      expect(lock.hardLocked, isFalse);
      lock.dispose();
    });
  });

  test('returning before the minute does not lock', () {
    fakeAsync((async) {
      final settings = SettingsStore()..softLock = true;
      final lock = AppLock(settings: settings, core: () => MockUmbraCore());
      lock.noteLifecycle(AppLifecycleState.paused);
      async.elapse(const Duration(seconds: 30));
      lock.noteLifecycle(AppLifecycleState.resumed);
      async.elapse(const Duration(minutes: 2));
      expect(lock.softLocked, isFalse);
      lock.dispose();
    });
  });

  test('a frozen background still locks from the clock', () {
    var now = DateTime(2026);
    final settings = SettingsStore()..softLock = true;
    final lock = AppLock(
      settings: settings,
      core: () => MockUmbraCore(),
      clock: () => now,
    );
    lock.noteLifecycle(AppLifecycleState.hidden);
    now = now.add(const Duration(minutes: 1));
    lock.noteLifecycle(AppLifecycleState.resumed);
    expect(lock.softLocked, isTrue);
    lock.dispose();
  });

  test('hard lock waits one idle hour and refuses biometrics', () async {
    final core = await _core('/tmp/umbra-lock-hour');
    fakeAsync((async) {
      final settings = SettingsStore();
      final lock = AppLock(
        settings: settings,
        core: () => core,
        biometric: _YesBiometric(),
      );
      lock.setPassphrase('1234');
      async.flushMicrotasks();
      lock.setHardLock(true);
      async.flushMicrotasks();
      expect(lock.hardLocked, isFalse);
      async.elapse(const Duration(minutes: 59));
      async.flushMicrotasks();
      expect(lock.hardLocked, isFalse);
      expect(core, isNotNull);
      async.elapse(const Duration(minutes: 1));
      async.flushMicrotasks();
      expect(lock.hardLocked, isTrue);
      NetworkInfo? stopped;
      core.networkStatus().then((value) => stopped = value);
      async.flushMicrotasks();
      expect(stopped?.running, isFalse);

      String? error;
      lock.unlock('', biometric: true).then((value) => error = value);
      async.flushMicrotasks();
      expect(error, 'A hard lock needs the passphrase.');
      expect(lock.hardLocked, isTrue);

      lock.unlock('1234').then((value) => error = value);
      async.flushMicrotasks();
      expect(error, isNull);
      expect(lock.hardLocked, isFalse);
      lock.dispose();
    });
    final status = await core.networkStatus();
    expect(status.running, isTrue);
  });

  test(
    'soft lock notice has no sender or text, hard lock drops events',
    () async {
      final core = MockUmbraCore(timing: MockTiming.instant);
      final screen = RecordingScreenSecurity(native: false);
      final controller = UmbraController(
        core: core,
        keyStore: MemoryDbKeyStore(),
        settings: SettingsStore(),
        supportDirectory: () async => '/tmp/umbra-lock-ui',
        biometric: _YesBiometric(),
        screen: screen,
        coverOnBlur: true,
        captureLimit: linuxCaptureLimit,
      );
      await controller.bootstrap();
      final lock = controller.appLock;
      await lock.setPassphrase('1234');
      await lock.setSoftLock(true);
      await lock.lockNow();
      expect(lock.softLocked, isTrue);
      expect(controller.network.running, isTrue);

      core.emitForTest(MessageReceived(_secret('secret phrase')));
      await _turns();
      expect(lock.lockedNotice, 'New message');
      expect(lock.lockedNotice, isNot(contains('secret')));
      expect(await lock.unlock('', biometric: true), isNull);
      expect(lock.lockedNotice, isNull);

      await lock.setHardLock(true);
      await lock.lockNow();
      expect(lock.hardLocked, isTrue);
      expect(controller.network.running, isFalse);
      core.emitForTest(MessageReceived(_secret('later secret')));
      await _turns();
      expect(lock.lockedNotice, isNull);
      expect(controller.messagesFor('peer'), isEmpty);
      expect(
        controller.core.sendText(MockUmbraCore.botPeerId, 'hi'),
        throwsA(isA<UmbraException>()),
      );
      expect(await lock.unlock('1234'), isNull);
      expect(controller.network.running, isTrue);

      await lock.setScreenSecurity(true);
      expect(screen.enabled, isTrue);
      expect(screen.calls, 1);
      expect(lock.captureLimitText, linuxCaptureLimit);
      expect(lock.obscured, isFalse);
      lock.noteLifecycle(AppLifecycleState.inactive);
      expect(lock.obscured, isTrue);
      lock.noteLifecycle(AppLifecycleState.resumed);
      expect(lock.obscured, isFalse);
      lock.dispose();
    },
  );

  testWidgets('lock screen hides chats and screen security covers Linux', (
    tester,
  ) async {
    final core = MockUmbraCore(timing: MockTiming.instant);
    final screen = RecordingScreenSecurity(native: false);
    final controller = UmbraController(
      core: core,
      keyStore: MemoryDbKeyStore(),
      settings: SettingsStore(),
      supportDirectory: () async => '/tmp/umbra-lock-widget',
      screen: screen,
      coverOnBlur: true,
      captureLimit: linuxCaptureLimit,
    );
    await tester.pumpWidget(UmbraApp(controller: controller));
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(TextField), 'Nyx');
    await tester.tap(find.text('Continue'));
    await tester.pumpAndSettle();

    await tester.tap(find.byIcon(Icons.settings_outlined));
    await tester.pumpAndSettle();
    Future<void> reveal(Finder target) async {
      final list = find.byKey(const Key('settings-list'));
      for (var i = 0; i < 8; i++) {
        if (target.hitTestable().evaluate().isNotEmpty) return;
        await tester.drag(list, const Offset(0, -250));
        await tester.pumpAndSettle();
      }
    }

    await reveal(find.byKey(const Key('screen-security')));
    expect(
      find.textContaining('Linux cannot block screenshots'),
      findsOneWidget,
    );
    expect(find.textContaining('screenshot-proof'), findsNothing);
    expect(find.text(screenSecurityTitle), findsWidgets);

    await reveal(find.byKey(const Key('soft-lock')));
    await tester.tap(find.byKey(const Key('soft-lock')));
    await tester.pumpAndSettle();
    final fields = find.descendant(
      of: find.byType(AlertDialog),
      matching: find.byType(TextField),
    );
    await tester.enterText(fields.at(0), '1234');
    await tester.enterText(fields.at(1), '1234');
    await tester.tap(find.widgetWithText(FilledButton, 'Save'));
    await tester.pumpAndSettle();
    expect(controller.appLock.settings.softLock, isTrue);

    await reveal(find.text('Lock now'));
    await tester.tap(find.text('Lock now'));
    await tester.pumpAndSettle();
    expect(find.text('Umbra is locked'), findsOneWidget);
    expect(
      find.descendant(
        of: find.byKey(const Key('app-lock')),
        matching: find.text('Mock Bot'),
      ),
      findsNothing,
    );
    expect(find.text('Use biometrics'), findsNothing);

    final lockField = find.descendant(
      of: find.byKey(const Key('app-lock')),
      matching: find.byType(TextField),
    );
    await tester.enterText(lockField, 'nope');
    await tester.tap(find.text('Unlock'));
    await tester.pumpAndSettle();
    expect(find.text('Wrong passphrase.'), findsOneWidget);
    expect(find.text('Umbra is locked'), findsOneWidget);

    await tester.enterText(lockField, '1234');
    await tester.tap(find.text('Unlock'));
    await tester.pumpAndSettle();
    expect(find.text('Umbra is locked'), findsNothing);
    expect(find.byKey(const Key('settings-list')), findsOneWidget);

    await tester.tap(find.text('Lock now'));
    await tester.pumpAndSettle();
    core.emitForTest(MessageReceived(_secret('secret phrase')));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 50));
    expect(
      find.descendant(
        of: find.byKey(const Key('app-lock')),
        matching: find.text('New message'),
      ),
      findsOneWidget,
    );
    expect(find.text('secret phrase'), findsNothing);

    await tester.enterText(lockField, '1234');
    await tester.tap(find.text('Unlock'));
    await tester.pumpAndSettle();

    final security = find.byKey(const Key('screen-security'));
    await reveal(security);
    await tester.tap(security);
    await tester.pumpAndSettle();
    expect(screen.enabled, isTrue);

    await tester.pageBack();
    await tester.pumpAndSettle();
    controller.appLock.noteLifecycle(AppLifecycleState.inactive);
    await tester.pump();
    expect(find.byKey(const Key('privacy-cover')), findsOneWidget);
    expect(find.text('secret phrase'), findsNothing);
    controller.appLock.noteLifecycle(AppLifecycleState.resumed);
    await tester.pump();
    expect(find.byKey(const Key('privacy-cover')), findsNothing);
    controller.appLock.dispose();
  });
}
