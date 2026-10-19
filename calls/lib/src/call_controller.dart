import 'dart:async';
import 'dart:math';

import 'call_privacy.dart';
import 'call_session.dart';
import 'ice_config.dart';
import 'signaling.dart';

class CallPrivacyException implements Exception {
  CallPrivacyException(this.code, this.message);
  final String code;
  final String message;
  @override
  String toString() => 'CallPrivacyException($code): $message';
}

/// App-facing API. Wire [transport] to core `send_signal`; peer id from `self_info()`.
class CallController {
  CallController({
    required this.localPeerId,
    required this.transport,
    IceConfig? iceConfig,
    CallPrivacySettings? privacy,
  })  : iceConfig = iceConfig ?? IceConfig(),
        privacy = privacy ?? CallPrivacySettings();

  final String localPeerId;
  final CallSignalTransport transport;
  IceConfig iceConfig;
  CallPrivacySettings privacy;

  CallSession? active;
  final _incoming = StreamController<CallSession>.broadcast();
  Stream<CallSession> get incomingCalls => _incoming.stream;
  final _rng = Random.secure();

  String _newCallId() {
    final bytes = List<int>.generate(16, (_) => _rng.nextInt(256));
    return bytes.map((b) => b.toRadixString(16).padLeft(2, '0')).join();
  }

  void _assertMayStartMedia() {
    if (privacy.shouldShowPreCallWarning) {
      throw CallPrivacyException('ack_required', preCallIpWarningShort);
    }
    if (iceConfig.relayOnlyBlocked) {
      throw CallPrivacyException(
        'turn_required',
        'Relay only is on, but no TURN server is configured.',
      );
    }
  }

  void acknowledgeIpRisk() => privacy.acknowledgeIpRisk = true;

  void setRelayOnly(bool enabled) {
    privacy.relayPolicy =
        enabled ? CallRelayPolicy.relayOnly : CallRelayPolicy.allowDirect;
    iceConfig = iceConfig.withRelayPolicy(privacy.relayPolicy);
  }

  Future<CallSession> startCall({
    required String remotePeerId,
    CallMediaMode mediaMode = CallMediaMode.audioOnly,
    bool skipPrivacyCheck = false,
  }) async {
    if (!skipPrivacyCheck) _assertMayStartMedia();
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
    try {
      await session.startAsCaller();
    } catch (_) {
      // Session already marked failed (unreachable peer / payload).
      rethrow;
    }
    return session;
  }

  Future<void> handleSignal(Map<String, Object?> json) async {
    final msg = CallSignalMessage.parse(json);
    if (msg == null) return;
    if (msg.toPeerId.isNotEmpty && msg.toPeerId != localPeerId) return;

    if (msg.kind == SignalKind.ringing) {
      if (active != null &&
          active!.state != CallSessionState.ended &&
          active!.state != CallSessionState.failed) {
        await transport.send(CallSignalMessage(
          kind: SignalKind.busy,
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

  Future<void> acceptIncoming({bool skipPrivacyCheck = false}) async {
    if (!skipPrivacyCheck) _assertMayStartMedia();
    await active?.accept();
  }

  void setTurnServers(List<IceServer> turn) {
    iceConfig = iceConfig.withTurn(turn);
  }

  Future<void> dispose() async {
    await active?.destroy();
    active = null;
    await _incoming.close();
  }
}
