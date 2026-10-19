import 'package:flutter_test/flutter_test.dart';
import 'package:umbra_calls/umbra_calls.dart';

class _RecordingTransport implements CallSignalTransport {
  final sent = <CallSignalMessage>[];
  @override
  Future<void> send(CallSignalMessage message) async => sent.add(message);
}

void main() {
  test('Tor mode without TURN auto-rejects Ringing and emits a missed call',
      () async {
    final transport = _RecordingTransport();
    final controller = CallController(
      localPeerId: 'local',
      transport: transport,
      iceConfig: IceConfig().withTorMode(true),
    );
    final missed = <MissedCall>[];
    final sub = controller.missedCalls.listen(missed.add);
    var rang = false;
    final sub2 = controller.incomingCalls.listen((_) => rang = true);

    await controller.handleCoreSignal(
      peerId: 'remote',
      callId: 'c1',
      kindWire: 'Ringing',
      payload: '{"media":"audio"}',
    );
    await Future<void>.delayed(Duration.zero);

    expect(transport.sent, hasLength(1));
    final reply = transport.sent.single;
    expect(reply.kind, SignalKind.reject);
    expect(reply.reason, 'tor_relay_required');
    expect(reply.toPeerId, 'remote');
    expect(reply.callId, 'c1');

    expect(missed, hasLength(1));
    expect(missed.single.peerId, 'remote');
    expect(missed.single.callId, 'c1');
    expect(missed.single.reason, 'tor_relay_required');

    expect(rang, isFalse);
    expect(controller.active, isNull);

    await sub.cancel();
    await sub2.cancel();
  });
}
