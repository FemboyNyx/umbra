import 'package:umbra_calls/umbra_calls.dart' as calls;

import '../core/core_errors.dart';
import '../core/umbra_core.dart';
import '../models/core_types.dart';
import 'controller_call_client.dart';
import 'outgoing_call.dart';

/// Connects the core event stream to Gem's call stack.
///
/// Incoming `CoreEvent::Signal` values are forwarded only through
/// [handleCoreSignal]. This file does not branch on Offer, Answer, or ICE.
class UmbraCallsBridge {
  UmbraCallsBridge({
    required this.core,
    required this.localPeerId,
    required bool torMode,
    calls.CallController? controller,
  }) : controller =
           controller ??
           calls.CallController(
             localPeerId: localPeerId,
             transport: calls.CoreSignalTransport(({
               required String peerId,
               required String callId,
               required calls.SignalKind kind,
               required String payload,
             }) {
               return core.sendSignal(
                 peerId: peerId,
                 callId: callId,
                 kind: _toCoreKind(kind),
                 payload: payload,
               );
             }),
             iceConfig: calls.IceConfig().withTorMode(torMode),
           ) {
    this.controller.setTorMode(torMode);
  }

  final UmbraCore core;
  final String localPeerId;
  final calls.CallController controller;

  void setTorMode(bool enabled) => controller.setTorMode(enabled);

  /// `CallController.handleCoreSignal` is the only calls entry point.
  ///
  /// Offer, Answer, and ICE are not interpreted here. If media setup throws
  /// after the peer was invited, the session is ended with Hangup or Reject
  /// so they are not left ringing.
  Future<void> handleCoreSignal({
    required String peerId,
    required String callId,
    required String kindWire,
    required String payload,
  }) async {
    try {
      await controller.handleCoreSignal(
        peerId: peerId,
        callId: callId,
        kindWire: kindWire,
        payload: payload,
      );
    } catch (err, stack) {
      debugLogCoreError(err, stack);
      final reason = isAudioSetupFailure(err)
          ? 'no_audio_device'
          : 'setup_failed';
      if (controller.active != null) {
        await endLiveSetup(reason: reason);
      } else if (kindWire == 'Ringing') {
        await _rejectUnstarted(peerId: peerId, callId: callId, reason: reason);
      }
      rethrow;
    }
  }

  /// Drops a session that failed during setup and tells the peer, when the
  /// protocol already reached them.
  Future<void> endLiveSetup({String? reason}) async {
    final session = controller.active;
    if (session == null) return;
    try {
      await tearDownCallSession(session, reason: reason);
    } catch (err, stack) {
      debugLogCoreError(err, stack);
    } finally {
      try {
        await session.destroy();
      } catch (err, stack) {
        debugLogCoreError(err, stack);
      }
      if (identical(controller.active, session)) {
        controller.active = null;
      }
    }
  }

  Future<void> _rejectUnstarted({
    required String peerId,
    required String callId,
    required String reason,
  }) async {
    try {
      final message = calls.CallSignalMessage(
        kind: calls.SignalKind.reject,
        callId: callId,
        fromPeerId: localPeerId,
        toPeerId: peerId,
        reason: reason,
      );
      await core.sendSignal(
        peerId: peerId,
        callId: callId,
        kind: SignalKind.reject,
        payload: message.toPayload(),
      );
    } catch (err, stack) {
      debugLogCoreError(err, stack);
    }
  }

  Future<void> dispose() => controller.dispose();
}

SignalKind _toCoreKind(calls.SignalKind kind) {
  return switch (kind) {
    calls.SignalKind.offer => SignalKind.offer,
    calls.SignalKind.answer => SignalKind.answer,
    calls.SignalKind.iceCandidate => SignalKind.iceCandidate,
    calls.SignalKind.hangup => SignalKind.hangup,
    calls.SignalKind.ringing => SignalKind.ringing,
    calls.SignalKind.reject => SignalKind.reject,
    calls.SignalKind.accept => SignalKind.accept,
    calls.SignalKind.busy => SignalKind.busy,
  };
}
