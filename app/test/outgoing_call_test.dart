import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:umbra/src/calls/controller_call_client.dart';
import 'package:umbra/src/calls/outgoing_call.dart';
import 'package:umbra/src/core/mock_umbra_core.dart';
import 'package:umbra/src/keystore/db_key_store.dart';
import 'package:umbra/src/models/core_types.dart';
import 'package:umbra/src/settings/settings_store.dart';
import 'package:umbra/src/state/umbra_controller.dart';
import 'package:umbra_calls/umbra_calls.dart' as calls;

void main() {
  test('audio device counts require an input and an output', () {
    expect(
      () => requireAudioDevices(
        countAudioDevices(const ['audioinput', 'audiooutput']),
      ),
      returnsNormally,
    );
    expect(
      () => requireAudioDevices(countAudioDevices(const ['audioinput'])),
      throwsA(isA<NoAudioDeviceException>()),
    );
    expect(
      () => requireAudioDevices(countAudioDevices(const ['audiooutput'])),
      throwsA(isA<NoAudioDeviceException>()),
    );
    expect(
      () => requireAudioDevices(countAudioDevices(const [])),
      throwsA(isA<NoAudioDeviceException>()),
    );
  });

  test('user facing call errors never include listen addresses', () {
    const onion = "could not listen on /ip4/10.1.2.3 and example.onion";
    final shown = userFacingCallError(Exception(onion));
    expect(shown, callEndedMessage);
    expect(shown, isNot(contains('/ip4/')));
    expect(shown, isNot(contains('.onion')));
    expect(shown, isNot(contains('could not listen')));
    expect(
      userFacingCallError(Exception('Unable to getUserMedia: no_audio_device')),
      noAudioDeviceMessage,
    );
    expect(
      userFacingCallError(const NoAudioDeviceException()),
      noAudioDeviceMessage,
    );
  });

  test('no audio device does not place a call', () async {
    final core = _RecordingCore();
    final client = _ScriptedClient(core);
    final probe = _ScriptedProbe()..failWith = Exception('no devices');
    final guard = OutgoingCallGuard(client: client, probe: probe);
    await expectLater(
      guard.start(MockUmbraCore.botPeerId),
      throwsA(isA<NoAudioDeviceException>()),
    );
    expect(client.places, 0);
    expect(client.cancels, 0);
    expect(core.kinds, isEmpty);
    expect(client.active, isFalse);
  });

  test('init failure before ringing stays idle', () async {
    final core = _RecordingCore();
    final client = _ScriptedClient(core)
      ..failWith = Exception('Failed to initialize the ADM');
    final guard = OutgoingCallGuard(client: client, probe: _ScriptedProbe());
    await expectLater(
      guard.start(MockUmbraCore.botPeerId),
      throwsA(
        isA<NoAudioDeviceException>().having(
          (err) => err.message,
          'message',
          noAudioDeviceMessage,
        ),
      ),
    );
    expect(client.places, 1);
    expect(client.cancels, 0);
    expect(core.kinds, isEmpty);
    expect(client.active, isFalse);
  });

  test('device loss after ringing hangs up and does not offer', () async {
    final core = _RecordingCore();
    await core.init(
      dataDir: '/tmp/umbra-call-mid',
      dbKey: Uint8List(32),
      cfg: const CoreConfig(mock: true),
    );
    core.listen((_) {});
    await core.startNetwork();
    await core.addContact(MockUmbraCore.mockBot, verified: false);
    final client = _ScriptedClient(core)
      ..failAfterSignal = true
      ..failWith = Exception('Unable to getUserMedia: NotFoundError');
    final guard = OutgoingCallGuard(client: client, probe: _ScriptedProbe());
    await expectLater(
      guard.start(MockUmbraCore.botPeerId),
      throwsA(isA<NoAudioDeviceException>()),
    );
    expect(core.kinds, [SignalKind.ringing, SignalKind.hangup]);
    expect(core.kinds, isNot(contains(SignalKind.offer)));
    expect(client.cancels, 1);
    expect(client.active, isFalse);
  });

  test('tor mode does not place a call', () async {
    final core = _RecordingCore();
    final client = _ScriptedClient(core)..blocked = true;
    final probe = _ScriptedProbe();
    final guard = OutgoingCallGuard(client: client, probe: probe);
    await expectLater(
      guard.start(MockUmbraCore.botPeerId),
      throwsA(
        isA<CallStartException>().having(
          (err) => err.message,
          'message',
          calls.torRelayRequiredMessage,
        ),
      ),
    );
    expect(probe.calls, 0);
    expect(client.places, 0);
    expect(core.kinds, isEmpty);
  });

  test('happy path sends ringing through the mock core', () async {
    final core = _RecordingCore();
    await core.init(
      dataDir: '/tmp/umbra-call-happy',
      dbKey: Uint8List(32),
      cfg: const CoreConfig(mock: true),
    );
    core.listen((_) {});
    await core.startNetwork();
    await core.addContact(MockUmbraCore.mockBot, verified: false);

    final client = _ScriptedClient(core);
    final guard = OutgoingCallGuard(client: client, probe: _ScriptedProbe());
    await guard.start(MockUmbraCore.botPeerId);

    expect(core.kinds, [SignalKind.ringing]);
    expect(client.active, isTrue);
    expect(client.cancels, 0);
  });

  test(
    'controller start honors no-device, init, tor, and mock ringing',
    () async {
      final core = _RecordingCore();
      final controller = await _booted(core);
      final probe = _ScriptedProbe();
      final client = _ScriptedClient(core);
      controller.outgoing = OutgoingCallGuard(client: client, probe: probe);

      expect(probe.calls, 0, reason: 'bootstrap must not open an audio device');

      probe.failWith = StateError('no input');
      await controller.startOutgoingCall(MockUmbraCore.botPeerId);
      expect(controller.callNotice, noAudioDeviceMessage);
      expect(controller.activeCallPeerId, isNull);
      expect(controller.callStarting, isFalse);
      expect(core.kinds, isEmpty);
      expect(controller.canStartCall(MockUmbraCore.botPeerId), isTrue);

      probe.failWith = null;
      client.failWith = Exception('ADM init');
      await controller.startOutgoingCall(MockUmbraCore.botPeerId);
      expect(controller.callNotice, noAudioDeviceMessage);
      expect(controller.activeCallPeerId, isNull);
      expect(core.kinds, isEmpty);

      client.failWith = null;
      controller.setCallsBlockedByTor(true);
      client.blocked = true;
      expect(controller.canStartCall(MockUmbraCore.botPeerId), isFalse);
      expect(
        controller.startCallTooltip(MockUmbraCore.botPeerId),
        calls.torRelayRequiredMessage,
      );
      await controller.startOutgoingCall(MockUmbraCore.botPeerId);
      expect(core.kinds, isEmpty);
      expect(probe.calls, 2);

      controller.setCallsBlockedByTor(false);
      client.blocked = false;
      await controller.startOutgoingCall(MockUmbraCore.botPeerId);
      expect(core.kinds, [SignalKind.ringing]);
      expect(controller.activeCallPeerId, MockUmbraCore.botPeerId);
      expect(controller.callNotice, isNull);
      expect(controller.canStartCall(MockUmbraCore.botPeerId), isFalse);

      await controller.cancelOutgoingCall();
      expect(core.kinds, [SignalKind.ringing, SignalKind.hangup]);
      expect(controller.activeCallPeerId, isNull);
      expect(controller.canStartCall(MockUmbraCore.botPeerId), isTrue);
    },
  );

  test('key change pending disables start', () async {
    final core = _RecordingCore();
    final controller = await _booted(core);
    final probe = _ScriptedProbe();
    controller.outgoing = OutgoingCallGuard(
      client: _ScriptedClient(core),
      probe: probe,
    );
    core.presentKeyChange(
      peerId: MockUmbraCore.botPeerId,
      newIdentityKey: 'EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE',
      newSafetyNumber:
          '11111 22222 33333 44444 55555 66666 77777 88888 99999 00000 12121 34343',
    );
    await Future<void>.delayed(Duration.zero);
    expect(controller.canStartCall(MockUmbraCore.botPeerId), isFalse);
    expect(
      controller.startCallTooltip(MockUmbraCore.botPeerId),
      'Safety number changed',
    );
    await controller.startOutgoingCall(MockUmbraCore.botPeerId);
    expect(probe.calls, 0);
    expect(core.kinds, isEmpty);
  });

  test('controller client uses startCall and hangup', () async {
    final fake = _FakeController();
    final client = ControllerOutgoingCall(fake);
    var idled = 0;
    client.onIdle = () => idled++;
    await client.placeCall('peer-1');
    expect(fake.starts, 1);
    expect(fake.skippedPrivacy, isTrue);
    expect(fake.peer, 'peer-1');
    expect(client.hasActiveCall, isTrue);

    await client.cancelCall(reason: 'no_audio_device');
    expect(fake.session!.hangups, 1);
    expect(fake.session!.reason, 'no_audio_device');
    expect(fake.active, isNull);
    expect(client.hasActiveCall, isFalse);
    expect(idled, 1);
  });
}

Future<UmbraController> _booted(_RecordingCore core) async {
  final controller = UmbraController(
    core: core,
    keyStore: MemoryDbKeyStore(),
    settings: SettingsStore(),
    supportDirectory: () async => '/tmp/umbra-outgoing',
  );
  await controller.bootstrap();
  await controller.setDisplayName('Nyx');
  await controller.acceptRequest(MockUmbraCore.mockBot);
  return controller;
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
  bool failAfterSignal = false;
  int places = 0;
  int cancels = 0;

  @override
  bool get callsBlockedByTor => blocked;

  @override
  bool get hasActiveCall => active;

  @override
  String get torBlockedMessage => calls.torRelayRequiredMessage;

  @override
  Future<void> placeCall(String peerId) async {
    places++;
    if (failWith != null && !failAfterSignal) throw failWith!;
    active = true;
    await core.sendSignal(
      peerId: peerId,
      callId: 'c-out',
      kind: SignalKind.ringing,
      payload: '',
    );
    if (failWith != null && failAfterSignal) throw failWith!;
  }

  @override
  Future<void> cancelCall({String? reason}) async {
    cancels++;
    active = false;
    await core.sendSignal(
      peerId: MockUmbraCore.botPeerId,
      callId: 'c-out',
      kind: SignalKind.hangup,
      payload: '',
    );
  }
}

class _NoopTransport implements calls.CallSignalTransport {
  @override
  Future<void> send(calls.CallSignalMessage message) async {}
}

class _QuietSession extends calls.CallSession {
  _QuietSession()
    : super(
        callId: 'c-quiet',
        localPeerId: 'local',
        remotePeerId: 'peer-1',
        mediaMode: calls.CallMediaMode.audioOnly,
        iceConfig: calls.IceConfig(),
        transport: _NoopTransport(),
        isCaller: true,
      );

  int hangups = 0;
  String? reason;

  @override
  Future<void> hangup({String? reason}) async {
    hangups++;
    this.reason = reason;
    state = calls.CallSessionState.ended;
  }

  @override
  Future<void> destroy() async {}
}

class _FakeController extends calls.CallController {
  _FakeController() : super(localPeerId: 'local', transport: _NoopTransport());

  int starts = 0;
  bool? skippedPrivacy;
  String? peer;
  _QuietSession? session;

  @override
  Future<calls.CallSession> startCall({
    required String remotePeerId,
    calls.CallMediaMode mediaMode = calls.CallMediaMode.audioOnly,
    bool skipPrivacyCheck = false,
  }) async {
    starts++;
    peer = remotePeerId;
    skippedPrivacy = skipPrivacyCheck;
    final created = _QuietSession()..state = calls.CallSessionState.ringingOut;
    session = created;
    active = created;
    return created;
  }
}
