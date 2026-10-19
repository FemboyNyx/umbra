import 'package:umbra_calls/umbra_calls.dart'
    show CallPrivacyException, torRelayRequiredMessage;

import '../core/core_errors.dart';

/// Shown when a call cannot open a microphone or speaker.
const noAudioDeviceMessage = 'No microphone or audio device found';

/// Shown when setup fails for a reason that is not safe to quote.
const callEndedMessage = 'The call ended';

/// Probe or WebRTC init could not open an audio device.
///
/// The message is fixed so a raw core or platform error, which may contain
/// listen addresses, never reaches the UI.
class NoAudioDeviceException implements Exception {
  const NoAudioDeviceException([this.message = noAudioDeviceMessage]);

  final String message;

  @override
  String toString() => message;
}

/// A start attempt that stopped before a call was left active.
class CallStartException implements Exception {
  const CallStartException(this.message);

  final String message;

  @override
  String toString() => message;
}

/// Counts reported by `enumerateDevices` / `getSources`.
class AudioDeviceCounts {
  const AudioDeviceCounts({required this.inputs, required this.outputs});

  final int inputs;
  final int outputs;
}

AudioDeviceCounts countAudioDevices(Iterable<String> kinds) {
  var inputs = 0;
  var outputs = 0;
  for (final kind in kinds) {
    if (kind == 'audioinput') inputs++;
    if (kind == 'audiooutput') outputs++;
  }
  return AudioDeviceCounts(inputs: inputs, outputs: outputs);
}

/// Both a capture device and a playout device are required.
void requireAudioDevices(AudioDeviceCounts counts) {
  if (counts.inputs < 1 || counts.outputs < 1) {
    throw const NoAudioDeviceException();
  }
}

/// True for ADM / getUserMedia failures. Matching is on stable substrings,
/// not on the full platform string, so addresses are not echoed.
bool isAudioSetupFailure(Object error) {
  if (error is NoAudioDeviceException) return true;
  final text = error.toString().toLowerCase();
  return text.contains('no_audio_device') ||
      text.contains('no audio device') ||
      text.contains('no microphone') ||
      text.contains('getusermedia') ||
      text.contains('notfounderror') ||
      text.contains('notreadableerror') ||
      text.contains('microphone') ||
      text.contains('failed to initialize the adm');
}

/// User-facing text. Unknown failures collapse to [callEndedMessage] so a
/// core error that embeds a listen address is never shown.
String userFacingCallError(Object error) {
  if (error is NoAudioDeviceException) return error.message;
  if (error is CallStartException) return error.message;
  if (error is CallPrivacyException && error.code == 'tor_relay_required') {
    return torRelayRequiredMessage;
  }
  if (isAudioSetupFailure(error)) return noAudioDeviceMessage;
  // Never quote the raw error. It can embed listen addresses.
  return callEndedMessage;
}

/// Checks that this device can capture and play audio.
///
/// Implementations must not be called at process launch. The first call is
/// the first time WebRTC / ADM is allowed to initialize.
abstract class AudioDeviceProbe {
  Future<void> ensureAvailable();
}

/// Places and cancels an outgoing call. Production wraps [CallController].
abstract class OutgoingCallClient {
  bool get callsBlockedByTor;

  /// Text to show instead of starting a call while [callsBlockedByTor].
  String get torBlockedMessage;

  /// A session exists that has not been fully released.
  bool get hasActiveCall;

  Future<void> placeCall(String peerId);

  /// Hang up or reject if the peer was already signaled, then release.
  Future<void> cancelCall({String? reason});
}

/// Orders the pre-call checks so a missing device never rings the peer.
class OutgoingCallGuard {
  OutgoingCallGuard({required this.client, required this.probe});

  final OutgoingCallClient client;
  final AudioDeviceProbe probe;

  Future<void> start(String peerId) async {
    if (client.callsBlockedByTor) {
      throw CallStartException(client.torBlockedMessage);
    }
    if (client.hasActiveCall) {
      throw const CallStartException('A call is already active');
    }
    try {
      await probe.ensureAvailable();
    } catch (err, stack) {
      debugLogCoreError(err, stack);
      if (client.hasActiveCall) {
        await _cancel(audio: true);
      }
      throw const NoAudioDeviceException();
    }
    try {
      await client.placeCall(peerId);
    } catch (err, stack) {
      debugLogCoreError(err, stack);
      if (err is CallPrivacyException) {
        if (client.hasActiveCall) await _cancel(audio: false);
        throw CallStartException(
          err.code == 'tor_relay_required'
              ? torRelayRequiredMessage
              : callEndedMessage,
        );
      }
      final audio = !client.hasActiveCall || isAudioSetupFailure(err);
      if (client.hasActiveCall) await _cancel(audio: audio);
      if (audio) throw const NoAudioDeviceException();
      throw const CallStartException(callEndedMessage);
    }
  }

  Future<void> _cancel({required bool audio}) async {
    try {
      await client.cancelCall(
        reason: audio ? 'no_audio_device' : 'setup_failed',
      );
    } catch (err, stack) {
      debugLogCoreError(err, stack);
    }
  }
}
