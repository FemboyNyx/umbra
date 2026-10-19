import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:umbra/src/calls/outgoing_call.dart';
import 'package:umbra/src/core/mock_umbra_core.dart';
import 'package:umbra/src/keystore/db_key_store.dart';
import 'package:umbra/src/models/core_types.dart';
import 'package:umbra/src/settings/settings_store.dart';
import 'package:umbra/src/state/umbra_controller.dart';
import 'package:umbra/src/ui/umbra_app.dart';
import 'package:umbra_calls/umbra_calls.dart' show torRelayRequiredMessage;

void main() {
  testWidgets('no audio device shows a message and stays idle', (tester) async {
    final harness = await _openChat(tester);
    harness.probe.failWith = Exception('enumerate failed');

    await tester.tap(find.byTooltip('Start call'));
    await tester.pumpAndSettle();

    expect(find.text('No microphone or audio device found'), findsWidgets);
    expect(find.textContaining('Calling'), findsNothing);
    expect(harness.core.kinds, isEmpty);
    expect(harness.controller.activeCallPeerId, isNull);
    expect(harness.controller.callStarting, isFalse);
    expect(harness.client.places, 0);
  });

  testWidgets('init failure does not ring or stick on calling', (tester) async {
    final harness = await _openChat(tester);
    harness.client.failWith = Exception('Failed to initialize the ADM');

    await tester.tap(find.byTooltip('Start call'));
    await tester.pumpAndSettle();

    expect(find.text('No microphone or audio device found'), findsWidgets);
    expect(find.textContaining('Calling'), findsNothing);
    expect(harness.core.kinds, isEmpty);
    expect(harness.controller.activeCallPeerId, isNull);
    expect(harness.client.cancels, 0);
  });

  testWidgets('tor mode disables start and does not call', (tester) async {
    final harness = await _openChat(
      tester,
      torSocks: '127.0.0.1:9050',
      blockedByTor: true,
    );

    expect(find.text(torRelayRequiredMessage), findsWidgets);
    expect(_callButton(tester).onPressed, isNull);
    expect(find.byTooltip(torRelayRequiredMessage), findsOneWidget);

    await tester.tap(find.byTooltip(torRelayRequiredMessage));
    await tester.pumpAndSettle();

    expect(harness.probe.calls, 0);
    expect(harness.client.places, 0);
    expect(harness.core.kinds, isEmpty);
    expect(find.textContaining('Calling'), findsNothing);
  });

  testWidgets('happy path rings the mock peer and can be cancelled', (
    tester,
  ) async {
    final harness = await _openChat(tester);

    await tester.tap(find.byTooltip('Start call'));
    await tester.pumpAndSettle();

    expect(harness.core.kinds, [SignalKind.ringing]);
    expect(harness.core.kinds, isNot(contains(SignalKind.offer)));
    expect(find.text('Calling…'), findsOneWidget);
    expect(harness.controller.activeCallPeerId, MockUmbraCore.botPeerId);
    expect(_callButton(tester).onPressed, isNull);
    expect(find.byTooltip('A call is already active'), findsOneWidget);

    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();

    expect(harness.core.kinds, [SignalKind.ringing, SignalKind.hangup]);
    expect(find.textContaining('Calling'), findsNothing);
    expect(harness.controller.activeCallPeerId, isNull);
    expect(find.byTooltip('Start call'), findsOneWidget);
  });

  testWidgets('key change pending disables start', (tester) async {
    final harness = await _openChat(tester);
    harness.core.presentKeyChange(
      peerId: MockUmbraCore.botPeerId,
      newIdentityKey: 'EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE',
      newSafetyNumber:
          '11111 22222 33333 44444 55555 66666 77777 88888 99999 00000 12121 34343',
    );
    await tester.pumpAndSettle();

    expect(_callButton(tester).onPressed, isNull);
    expect(find.byTooltip('Safety number changed'), findsOneWidget);
    await tester.tap(find.byTooltip('Safety number changed'));
    await tester.pumpAndSettle();
    expect(harness.probe.calls, 0);
    expect(harness.core.kinds, isEmpty);
  });
}

IconButton _callButton(WidgetTester tester) {
  return tester.widget<IconButton>(find.widgetWithIcon(IconButton, Icons.call));
}

class _Harness {
  _Harness({
    required this.controller,
    required this.core,
    required this.probe,
    required this.client,
  });

  final UmbraController controller;
  final _RecordingCore core;
  final _ScriptedProbe probe;
  final _ScriptedClient client;
}

Future<_Harness> _openChat(
  WidgetTester tester, {
  String? torSocks,
  bool blockedByTor = false,
}) async {
  final core = _RecordingCore();
  final controller = UmbraController(
    core: core,
    keyStore: MemoryDbKeyStore(),
    settings: SettingsStore()..torSocks = torSocks ?? '',
    supportDirectory: () async => '/tmp/umbra-start-call',
  );
  final probe = _ScriptedProbe();
  final client = _ScriptedClient(core)..blocked = blockedByTor;
  controller.outgoing = OutgoingCallGuard(client: client, probe: probe);
  if (blockedByTor) {
    controller.attachCalls = (_) {
      controller.setCallsBlockedByTor(true);
    };
  }
  await tester.pumpWidget(UmbraApp(controller: controller));
  await tester.pumpAndSettle();
  await tester.enterText(find.byType(TextField), 'Nyx');
  await tester.tap(find.text('Continue'));
  await tester.pumpAndSettle();
  await tester.tap(find.text('Accept'));
  await tester.pumpAndSettle();
  await tester.tap(find.text('Mock Bot').first);
  await tester.pumpAndSettle();
  return _Harness(
    controller: controller,
    core: core,
    probe: probe,
    client: client,
  );
}

class _RecordingCore extends MockUmbraCore {
  _RecordingCore() : super(timing: MockTiming.instant);

  final kinds = <SignalKind>[];

  @override
  Future<void> sendSignal({
    required String peerId,
    required String callId,
    required SignalKind kind,
    required String payload,
  }) async {
    kinds.add(kind);
    await super.sendSignal(
      peerId: peerId,
      callId: callId,
      kind: kind,
      payload: payload,
    );
  }
}

class _ScriptedProbe implements AudioDeviceProbe {
  Object? failWith;
  int calls = 0;

  @override
  Future<void> ensureAvailable() async {
    calls++;
    final fail = failWith;
    if (fail != null) throw fail;
  }
}

class _ScriptedClient implements OutgoingCallClient {
  _ScriptedClient(this.core);

  final _RecordingCore core;
  bool active = false;
  bool blocked = false;
  Object? failWith;
  int places = 0;
  int cancels = 0;

  @override
  bool get callsBlockedByTor => blocked;

  @override
  bool get hasActiveCall => active;

  @override
  String get torBlockedMessage => torRelayRequiredMessage;

  @override
  Future<void> placeCall(String peerId) async {
    places++;
    final fail = failWith;
    if (fail != null) throw fail;
    active = true;
    await core.sendSignal(
      peerId: peerId,
      callId: 'c-ui',
      kind: SignalKind.ringing,
      payload: '',
    );
  }

  @override
  Future<void> cancelCall({String? reason}) async {
    cancels++;
    active = false;
    await core.sendSignal(
      peerId: MockUmbraCore.botPeerId,
      callId: 'c-ui',
      kind: SignalKind.hangup,
      payload: '',
    );
  }
}
