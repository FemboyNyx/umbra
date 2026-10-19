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

  test('fromCore builds trusted signal', () {
    final msg = CallSignalMessage.fromCore(
      peerId: 'remote',
      localPeerId: 'local',
      callId: 'c1',
      kindWire: 'Offer',
      payload: {'sdp': 'v=0', 'sdpType': 'offer', 'media': 'audio'},
    );
    expect(msg!.kind, SignalKind.offer);
    expect(msg.fromPeerId, 'remote');
    expect(msg.toPeerId, 'local');
    expect(msg.sdp, 'v=0');
  });

  test('relay-only policy', () {
    final blocked = IceConfig(relayPolicy: CallRelayPolicy.relayOnly);
    expect(blocked.relayOnlyBlocked, isTrue);
    final ok = IceConfig(
      turnServers: const [
        IceServer(
          urls: ['turn:example.invalid:3478'],
          username: 'u',
          credential: 'p',
        ),
      ],
      relayPolicy: CallRelayPolicy.relayOnly,
    );
    expect(ok.toPeerConnectionConfig()['iceTransportPolicy'], 'relay');
  });

  test('fromCore ignores routing fields injected in the payload', () {
    final msg = CallSignalMessage.fromCore(
      peerId: 'mallory',
      localPeerId: 'local',
      callId: 'c1',
      kindWire: 'Ringing',
      payload: {
        'from': 'bob',
        'fromPeerId': 'bob',
        'peer_id': 'bob',
        'callId': 'bobs-call',
        'to': 'someone-else',
        'kind': 'Answer',
        'type': 'Answer',
        'media': 'av',
      },
    );
    expect(msg, isNotNull);
    expect(msg!.fromPeerId, 'mallory');
    expect(msg.callId, 'c1');
    expect(msg.toPeerId, 'local');
    expect(msg.kind, SignalKind.ringing);
    expect(msg.mediaMode, CallMediaMode.audioVideo);
  });
}
