import 'dart:async';
import 'dart:convert';
import 'dart:math';

import 'call_privacy.dart';
import 'call_session.dart';
import 'ice_config.dart';
import 'signaling.dart';

/// Shown when Tor mode is on and no TURN relay is configured.
const String torRelayRequiredMessage = 'Calls over Tor need a relay server.';

/// Thrown when a call can't start for privacy reasons. [code] is one of:
/// - `ack_required`: user must acknowledge [preCallIpWarning] first.
/// - `turn_required`: Relay only is on but no TURN server is configured.
/// - `tor_relay_required`: Tor mode is on but no TURN server is configured.
///   This one cannot be bypassed with `skipPrivacyCheck`.
class CallPrivacyException implements Exception {
  CallPrivacyException(this.code, this.message);
  final String code;
  final String message;
  @override
  String toString() => 'CallPrivacyException($code): $message';
}

/// App-facing API for 1:1 calls.
///
/// **E2E guarantee:** SDP offers/answers and ICE candidates must be applied
/// only after they arrive on core's encrypted `CoreEvent::Signal` path.
/// Use [handleCoreSignal] — never feed raw SDP from the UI, deep links,
/// push payloads, or any unencrypted channel into a [CallSession].
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

  /// Tor mode never falls back to direct; enforced even with skipPrivacyCheck.
  void _assertTorRelay() {
    if (iceConfig.torRelayBlocked) {
      throw CallPrivacyException('tor_relay_required', torRelayRequiredMessage);
    }
  }

  /// True when the app should show [torRelayRequiredMessage] instead of a
  /// call button.
  bool get callsBlockedByTor => iceConfig.torRelayBlocked;

  void setTorMode(bool enabled) {
    iceConfig = iceConfig.withTorMode(enabled);
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
    _assertTorRelay();
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
    await session.startAsCaller();
    return session;
  }

  /// Canonical ingress for core `CoreEvent::Signal`.
  ///
  /// This is the **only** supported way to inject remote Offer / Answer / ICE.
  Future<void> handleCoreSignal({
    required String peerId,
    required String callId,
    required String kindWire,
    required String payload,
  }) async {
    Map<String, Object?> body = {};
    if (payload.isNotEmpty) {
      final decoded = jsonDecode(payload);
      if (decoded is Map) {
        body = decoded.map((k, v) => MapEntry(k.toString(), v as Object?));
      }
    }
    final msg = CallSignalMessage.fromCore(
      peerId: peerId,
      localPeerId: localPeerId,
      callId: callId,
      kindWire: kindWire,
      payload: body,
    );
    if (msg == null) return;
    await _dispatchTrusted(msg);
  }

  Future<void> _dispatchTrusted(CallSignalMessage msg) async {
    if (msg.toPeerId.isNotEmpty && msg.toPeerId != localPeerId) return;

    // Media renegotiation kinds are only valid for the active peer/call.
    if (_isMediaKind(msg.kind)) {
      final session = active;
      if (session == null ||
          session.callId != msg.callId ||
          session.remotePeerId != msg.fromPeerId) {
        return; // drop injected / mismatched SDP & ICE
      }
      await session.applyRemoteSignal(msg);
      return;
    }

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
    if (session.remotePeerId != msg.fromPeerId) return;
    await session.applyRemoteSignal(msg);
  }

  static bool _isMediaKind(SignalKind kind) =>
      kind == SignalKind.offer ||
      kind == SignalKind.answer ||
      kind == SignalKind.iceCandidate;

  Future<void> acceptIncoming({bool skipPrivacyCheck = false}) async {
    _assertTorRelay();
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
