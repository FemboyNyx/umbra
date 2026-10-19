import 'package:umbra_calls/umbra_calls.dart' as calls;

import '../core/umbra_core.dart';
import '../models/core_types.dart';

/// Connects the core event stream to Gem's call stack.
///
/// Incoming `CoreEvent::Signal` values are forwarded only through
/// [handleCoreSignal]. This file does not branch on Offer, Answer, or ICE.
class UmbraCallsBridge {
  UmbraCallsBridge({required this.core, required this.localPeerId});

  final UmbraCore core;
  final String localPeerId;

  late final calls.CallController controller = calls.CallController(
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
  );

  /// Gem's ingress (`CallController.handleCoreSignal` on `gem` at `af9871c`).
  ///
  /// Invoked by name. Offer, Answer, and ICE are not sent through
  /// `handleSignal` or any other call entry point.
  Future<void> handleCoreSignal({
    required String peerId,
    required String callId,
    required String kind,
    required String payload,
  }) {
    final gem = controller as dynamic;
    return gem.handleCoreSignal(
      peerId: peerId,
      callId: callId,
      kind: kind,
      payload: payload,
    );
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
    calls.SignalKind.accept || calls.SignalKind.busy => throw UmbraException(
      'core has no signal kind ${kind.wire}',
    ),
  };
}
