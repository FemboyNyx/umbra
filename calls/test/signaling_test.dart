import 'package:flutter_test/flutter_test.dart';
import 'package:umbra_calls/umbra_calls.dart';

void main() {
  test('ringing round-trips', () {
    final msg = CallSignalMessage(
      kind: SignalKind.ringing,
      callId: 'abc',
      fromPeerId: 'a',
      toPeerId: 'b',
      mediaMode: CallMediaMode.audioVideo,
    );
    final parsed = CallSignalMessage.parse(msg.toJson());
    expect(parsed!.kind, SignalKind.ringing);
    expect(parsed.mediaMode, CallMediaMode.audioVideo);
  });

  test('SignalKind wires match core', () {
    expect(SignalKind.accept.wire, 'Accept');
    expect(SignalKind.busy.wire, 'Busy');
    expect(SignalKind.iceCandidate.wire, 'IceCandidate');
  });

  test('relay-only policy', () {
    final blocked = IceConfig(relayPolicy: CallRelayPolicy.relayOnly);
    expect(blocked.relayOnlyBlocked, isTrue);
    final ok = IceConfig(
      turnServers: const [
        IceServer(urls: ['turn:example.invalid:3478'], username: 'u', credential: 'p'),
      ],
      relayPolicy: CallRelayPolicy.relayOnly,
    );
    expect(ok.toPeerConnectionConfig()['iceTransportPolicy'], 'relay');
  });
}
