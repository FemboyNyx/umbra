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
  });
}
