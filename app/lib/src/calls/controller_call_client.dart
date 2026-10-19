import 'dart:async';

import 'package:umbra_calls/umbra_calls.dart' as calls;

import '../core/core_errors.dart';
import 'outgoing_call.dart';

/// Sends Hangup or Reject when the peer may already be in the call, then
/// the caller releases renderers. A session that never signaled is only
/// released locally.
Future<void> tearDownCallSession(
  calls.CallSession session, {
  String? reason,
}) async {
  switch (session.state) {
    case calls.CallSessionState.ringingOut:
    case calls.CallSessionState.connecting:
    case calls.CallSessionState.connected:
      await session.hangup(reason: reason);
    case calls.CallSessionState.creating:
      if (!session.isCaller) {
        await session.reject(reason: reason);
      }
    case calls.CallSessionState.ringingIn:
      await session.reject(reason: reason);
    case calls.CallSessionState.idle:
    case calls.CallSessionState.ended:
    case calls.CallSessionState.failed:
      break;
  }
}

/// [CallController] as an [OutgoingCallClient].
///
/// `skipPrivacyCheck` skips the direct-IP acknowledgement. Tor mode is still
/// enforced inside `startCall`. The settings screen already states that
/// calls reveal a network path.
class ControllerOutgoingCall implements OutgoingCallClient {
  ControllerOutgoingCall(this.controller);

  final calls.CallController controller;
  void Function()? onIdle;
  StreamSubscription<calls.CallSessionState>? _states;

  @override
  bool get callsBlockedByTor => controller.callsBlockedByTor;

  @override
  String get torBlockedMessage => calls.torRelayRequiredMessage;

  @override
  bool get hasActiveCall {
    final session = controller.active;
    if (session == null) return false;
    return session.state != calls.CallSessionState.ended;
  }

  @override
  Future<void> placeCall(String peerId) async {
    final session = await controller.startCall(
      remotePeerId: peerId,
      skipPrivacyCheck: true,
    );
    await _states?.cancel();
    _states = session.stateChanges.listen((state) {
      if (state == calls.CallSessionState.ended ||
          state == calls.CallSessionState.failed) {
        onIdle?.call();
      }
    });
  }

  @override
  Future<void> cancelCall({String? reason}) async {
    final session = controller.active;
    if (session == null) return;
    await _states?.cancel();
    _states = null;
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
      onIdle?.call();
    }
  }
}
