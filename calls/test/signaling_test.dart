import 'package:flutter_test/flutter_test.dart';
import 'package:umbra_calls/umbra_calls.dart';

void main() {
  test('invite round-trips through JSON', () {
    final msg = CallSignalMessage(
      type: CallSignalType.invite,
      callId: 'abc',
      fromPeerId: 'peer-a',
      toPeerId: 'peer-b',
      mediaMode: CallMediaMode.audioVideo,
    );
    final parsed = CallSignalMessage.parse(msg.toJson());
    expect(parsed, isNotNull);
    expect(parsed!.type, CallSignalType.invite);
    expect(parsed.callId, 'abc');
    expect(parsed.mediaMode, CallMediaMode.audioVideo);
  });

  test('ice config has public stun and empty turn by default', () {
    final cfg = IceConfig();
    expect(cfg.stunServers, isNotEmpty);
    expect(cfg.turnServers, isEmpty);
    final map = cfg.toPeerConnectionConfig();
    expect(map['iceServers'], isA<List>());
    expect(map.containsKey('iceTransportPolicy'), isFalse);
  });

  test('relay-only sets iceTransportPolicy and omits stun', () {
    final cfg = IceConfig(
      turnServers: const [
        IceServer(
          urls: ['turn:example.invalid:3478'],
          username: 'u',
          credential: 'p',
        ),
      ],
      relayPolicy: CallRelayPolicy.relayOnly,
    );
    expect(cfg.relayOnlyBlocked, isFalse);
    final map = cfg.toPeerConnectionConfig();
    expect(map['iceTransportPolicy'], 'relay');
    final servers = map['iceServers'] as List;
    expect(servers.length, 1);
  });

  test('relay-only without TURN is blocked', () {
    final cfg = IceConfig(relayPolicy: CallRelayPolicy.relayOnly);
    expect(cfg.relayOnlyBlocked, isTrue);
  });
}
