import 'dart:async';
import 'dart:math';

import 'call_session.dart';
import 'ice_config.dart';
import 'signaling.dart';

/// App-facing API for placing and receiving 1:1 calls.
///
/// Wire [transport] to core so signals travel on the E2E channel.
/// Provide [localPeerId] from core identity.
class CallController {
  CallController({
    required this.localPeerId,
    required this.transport,
    IceConfig? iceConfig,
  }) : iceConfig = iceConfig ?? IceConfig();

  final String localPeerId;
  final CallSignalTransport transport;
  IceConfig iceConfig;

  CallSession? active;
  final _incoming = StreamController<CallSession>.broadcast();

  /// Fires when a remote invite arrives (show accept/decline UI).
  Stream<CallSession> get incomingCalls => _incoming.stream;

  final _rng = Random.secure();

  String _newCallId() {
    final bytes = List<int>.generate(16, (_) => _rng.nextInt(256));
    return bytes.map((b) => b.toRadixString(16).padLeft(2, '0')).join();
  }

  /// Place an outbound call.
  Future<CallSession> startCall({
    required String remotePeerId,
    CallMediaMode mediaMode = CallMediaMode.audioOnly,
  }) async {
    if (active != null &&
        active!.state != CallSessionState.ended &&
        active!.state != CallSessionState.failed) {
      throw StateError('A call is already active');
    }
    final session = CallSession(
      callId: _newCallId(),
      localPeerId: localPeerId,
      remotePeerId: remotePeerId,
      mediaMode: mediaMode,
      iceConfig: iceConfig,
      transport: transport,
      isCaller: true,
    );
    await session.initRenderers();
    active = session;
    await session.startAsCaller();
    return session;
  }

  /// Feed every core-delivered call signal into the controller.
  Future<void> handleSignal(Map<String, Object?> json) async {
    final msg = CallSignalMessage.parse(json);
    if (msg == null) return;
    if (msg.toPeerId != localPeerId) return;

    if (msg.type == CallSignalType.invite) {
      if (active != null &&
          active!.state != CallSessionState.ended &&
          active!.state != CallSessionState.failed) {
        // Busy: auto-reject.
        await transport.send(CallSignalMessage(
          type: CallSignalType.reject,
          callId: msg.callId,
          fromPeerId: localPeerId,
          toPeerId: msg.fromPeerId,
          mediaMode: msg.mediaMode,
          reason: 'busy',
        ));
        return;
      }
      final session = CallSession(
        callId: msg.callId,
        localPeerId: localPeerId,
        remotePeerId: msg.fromPeerId,
        mediaMode: msg.mediaMode,
        iceConfig: iceConfig,
        transport: transport,
        isCaller: false,
      );
      await session.initRenderers();
      active = session;
      await session.notifyIncoming();
      _incoming.add(session);
      return;
    }

    final session = active;
    if (session == null || session.callId != msg.callId) return;
    await session.onRemoteSignal(msg);
  }

  /// Replace TURN servers from user settings (never hardcode secrets).
  void setTurnServers(List<IceServer> turn) {
    iceConfig = iceConfig.withTurn(turn);
  }

  Future<void> dispose() async {
    await active?.destroy();
    active = null;
    await _incoming.close();
  }
}
