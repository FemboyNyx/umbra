import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:umbra/src/calls/calls_bridge.dart';
import 'package:umbra/src/core/mock_umbra_core.dart';
import 'package:umbra/src/models/core_types.dart';
import 'package:umbra_calls/umbra_calls.dart' as calls;

void main() {
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
}
