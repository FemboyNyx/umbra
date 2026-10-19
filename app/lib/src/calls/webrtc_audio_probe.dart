import 'package:flutter_webrtc/flutter_webrtc.dart';

import 'outgoing_call.dart';

/// Opens the microphone only when the user starts a call.
///
/// Constructing this does not call into WebRTC. [ensureAvailable] is the
/// first native audio entry, so a machine with no audio device can still
/// launch. The capture stream is stopped before the call itself starts.
class WebrtcAudioProbe implements AudioDeviceProbe {
  const WebrtcAudioProbe();

  @override
  Future<void> ensureAvailable() async {
    final listed = await navigator.mediaDevices.enumerateDevices();
    requireAudioDevices(
      countAudioDevices(listed.map((device) => device.kind ?? '')),
    );
    final stream = await navigator.mediaDevices.getUserMedia(const {
      'audio': true,
      'video': false,
    });
    try {
      if (stream.getAudioTracks().isEmpty) {
        throw const NoAudioDeviceException();
      }
    } finally {
      for (final track in stream.getTracks()) {
        await track.stop();
      }
      await stream.dispose();
    }
  }
}
