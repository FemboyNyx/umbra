import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:umbra/src/calls/calls_bridge.dart';
import 'package:umbra/src/core/mock_umbra_core.dart';
import 'package:umbra/src/models/core_types.dart';
import 'package:umbra_calls/umbra_calls.dart' as calls;

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test('tor mode blocks calls and records a missed ringing', () async {
    final core = MockUmbraCore(timing: MockTiming.instant);
    await core.init(
      dataDir: '/tmp/umbra-calls',
      dbKey: Uint8List(32),
      cfg: const CoreConfig(mock: true),
    );
    core.listen((_) {});
    await core.startNetwork();
    await core.addContact(MockUmbraCore.mockBot, verified: false);

    final bridge = UmbraCallsBridge(
      core: core,
      localPeerId: MockUmbraCore.selfPeerId,
      torMode: true,
    );
    expect(bridge.controller.callsBlockedByTor, isTrue);
    expect(bridge.controller.iceConfig.torMode, isTrue);

    final missed = <calls.MissedCall>[];
    final sub = bridge.controller.missedCalls.listen(missed.add);
    await bridge.handleCoreSignal(
      peerId: MockUmbraCore.botPeerId,
      callId: 'call-1',
      kindWire: 'Ringing',
      payload: '',
    );
    await Future<void>.delayed(Duration.zero);
    expect(missed, hasLength(1));
    expect(missed.single.peerId, MockUmbraCore.botPeerId);
    expect(missed.single.callId, 'call-1');
    expect(missed.single.reason, 'tor_relay_required');

    bridge.setTorMode(false);
    expect(bridge.controller.callsBlockedByTor, isFalse);
    await sub.cancel();
    await bridge.dispose();
  });

  test('ringing with no audio device rejects and stays idle', () async {
    final core = _RecordingCore();
    await _ready(core);
    final bridge = UmbraCallsBridge(
      core: core,
      localPeerId: MockUmbraCore.selfPeerId,
      torMode: false,
    );

    await expectLater(
      bridge.handleCoreSignal(
        peerId: MockUmbraCore.botPeerId,
        callId: 'call-ring',
        kindWire: 'Ringing',
        payload: '',
      ),
      throwsA(anything),
    );

    expect(bridge.controller.active, isNull);
    expect(core.kinds, [SignalKind.reject]);
    expect(core.kinds, isNot(contains(SignalKind.offer)));
    expect(core.kinds, isNot(contains(SignalKind.ringing)));
    await bridge.dispose();
  });

  test('setup failure after the peer is ringing sends hangup', () async {
    final core = _RecordingCore();
    await _ready(core);
    final session = _QuietSession();
    final bridge = UmbraCallsBridge(
      core: core,
      localPeerId: MockUmbraCore.selfPeerId,
      torMode: false,
      controller: _BoomController(session),
    );

    await expectLater(
      bridge.handleCoreSignal(
        peerId: MockUmbraCore.botPeerId,
        callId: 'call-mid',
        kindWire: 'Accept',
        payload: '',
      ),
      throwsA(isA<Exception>()),
    );

    expect(session.hangups, 1);
    expect(session.reason, 'no_audio_device');
    expect(bridge.controller.active, isNull);
    expect(core.kinds, isNot(contains(SignalKind.offer)));
    await bridge.dispose();
  });
}

Future<void> _ready(MockUmbraCore core) async {
  await core.init(
    dataDir: '/tmp/umbra-calls',
    dbKey: Uint8List(32),
    cfg: const CoreConfig(mock: true),
  );
  core.listen((_) {});
  await core.startNetwork();
  await core.addContact(MockUmbraCore.mockBot, verified: false);
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

class _NoopTransport implements calls.CallSignalTransport {
  @override
  Future<void> send(calls.CallSignalMessage message) async {}
}

class _QuietSession extends calls.CallSession {
  _QuietSession()
    : super(
        callId: 'call-mid',
        localPeerId: MockUmbraCore.selfPeerId,
        remotePeerId: MockUmbraCore.botPeerId,
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

class _BoomController extends calls.CallController {
  _BoomController(this.session)
    : super(localPeerId: MockUmbraCore.selfPeerId, transport: _NoopTransport());

  final _QuietSession session;

  @override
  Future<void> handleCoreSignal({
    required String peerId,
    required String callId,
    required String kindWire,
    required String payload,
  }) async {
    session.state = calls.CallSessionState.connecting;
    active = session;
    throw Exception('Unable to getUserMedia: NotFoundError');
  }
}
