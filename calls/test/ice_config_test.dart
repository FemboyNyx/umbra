import 'package:flutter_test/flutter_test.dart';
import 'package:umbra_calls/umbra_calls.dart';

void main() {
  const turn = IceServer(
    urls: ['turn:example.invalid:3478'],
    username: 'u',
    credential: 'p',
  );

  List<String> urlsOf(IceConfig c) => [
        for (final s in c.toPeerConnectionConfig()['iceServers'] as List)
          ...(s as Map)['urls'] as List<String>,
      ];

  test('tor mode never uses STUN', () {
    final c = IceConfig(turnServers: const [turn]).withTorMode(true);
    expect(c.effectiveStunServers, isEmpty);
    expect(urlsOf(c).any((u) => u.startsWith('stun:')), isFalse);
    expect(urlsOf(c), contains('turn:example.invalid:3478'));
  });

  test('tor mode survives withTurn / withRelayPolicy', () {
    final c = IceConfig()
        .withTorMode(true)
        .withTurn(const [turn]).withRelayPolicy(CallRelayPolicy.allowDirect);
    expect(c.torMode, isTrue);
    expect(c.effectiveStunServers, isEmpty);
  });

  test('default STUN list is configurable', () {
    final saved = IceConfig.defaultStun;
    addTearDown(() => IceConfig.defaultStun = saved);
    IceConfig.defaultStun = const [
      IceServer(urls: ['stun:stun.example.invalid:3478']),
    ];
    expect(urlsOf(IceConfig()), ['stun:stun.example.invalid:3478']);
    expect(
      urlsOf(IceConfig().withStunServers(const [])),
      isEmpty,
    );
  });

  test('pre-call warning has no stray quotes', () {
    expect(preCallIpWarning.contains("'"), isFalse);
    expect(preCallIpWarning, contains('reveal your IP address'));
  });

  test('tor mode forces relay and blocks without TURN', () {
    final noTurn = IceConfig().withTorMode(true);
    expect(noTurn.forcesRelay, isTrue);
    expect(noTurn.torRelayBlocked, isTrue);
    expect(noTurn.toPeerConnectionConfig()['iceTransportPolicy'], 'relay');
    expect(urlsOf(noTurn), isEmpty);

    final withTurn = noTurn.withTurn(const [turn]);
    expect(withTurn.torRelayBlocked, isFalse);
    expect(withTurn.toPeerConnectionConfig()['iceTransportPolicy'], 'relay');

    final direct = withTurn.withRelayPolicy(CallRelayPolicy.allowDirect);
    expect(direct.forcesRelay, isTrue, reason: 'Tor never allows direct');
  });
}
