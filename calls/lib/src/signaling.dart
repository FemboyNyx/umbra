/// Call signaling messages carried by core's encrypted channel.
///
/// Wire format: a JSON object with `type` and payload fields below.
/// Core should treat these as opaque application messages (e.g. `app/call/v1`).
library;

/// Message type discriminator for call signaling.
enum CallSignalType {
  invite,
  accept,
  reject,
  hangup,
  sdpOffer,
  sdpAnswer,
  iceCandidate,
}

extension CallSignalTypeWire on CallSignalType {
  String get wire {
    switch (this) {
      case CallSignalType.invite:
        return 'call.invite';
      case CallSignalType.accept:
        return 'call.accept';
      case CallSignalType.reject:
        return 'call.reject';
      case CallSignalType.hangup:
        return 'call.hangup';
      case CallSignalType.sdpOffer:
        return 'call.sdp.offer';
      case CallSignalType.sdpAnswer:
        return 'call.sdp.answer';
      case CallSignalType.iceCandidate:
        return 'call.ice';
    }
  }

  static CallSignalType? tryParse(String wire) {
    for (final v in CallSignalType.values) {
      if (v.wire == wire) return v;
    }
    return null;
  }
}

/// Media mode requested for a call.
enum CallMediaMode {
  audioOnly,
  audioVideo,
}

/// A signaling envelope. Serialize with [toJson] / parse with [CallSignalMessage.parse].
class CallSignalMessage {
  CallSignalMessage({
    required this.type,
    required this.callId,
    required this.fromPeerId,
    required this.toPeerId,
    this.mediaMode = CallMediaMode.audioOnly,
    this.sdp,
    this.sdpType,
    this.iceCandidate,
    this.iceSdpMid,
    this.iceSdpMLineIndex,
    this.reason,
  });

  final CallSignalType type;
  final String callId;
  final String fromPeerId;
  final String toPeerId;
  final CallMediaMode mediaMode;

  /// SDP body for offer/answer.
  final String? sdp;
  final String? sdpType; // "offer" | "answer"

  final String? iceCandidate;
  final String? iceSdpMid;
  final int? iceSdpMLineIndex;

  /// Optional human-readable reject/hangup reason.
  final String? reason;

  Map<String, Object?> toJson() => {
        'type': type.wire,
        'callId': callId,
        'from': fromPeerId,
        'to': toPeerId,
        'media': mediaMode == CallMediaMode.audioVideo ? 'av' : 'audio',
        if (sdp != null) 'sdp': sdp,
        if (sdpType != null) 'sdpType': sdpType,
        if (iceCandidate != null) 'candidate': iceCandidate,
        if (iceSdpMid != null) 'sdpMid': iceSdpMid,
        if (iceSdpMLineIndex != null) 'sdpMLineIndex': iceSdpMLineIndex,
        if (reason != null) 'reason': reason,
      };

  static CallSignalMessage? parse(Map<String, Object?> json) {
    final typeRaw = json['type'];
    if (typeRaw is! String) return null;
    final type = CallSignalTypeWire.tryParse(typeRaw);
    if (type == null) return null;

    final callId = json['callId'];
    final from = json['from'];
    final to = json['to'];
    if (callId is! String || from is! String || to is! String) return null;

    final mediaRaw = json['media'];
    final media = mediaRaw == 'av'
        ? CallMediaMode.audioVideo
        : CallMediaMode.audioOnly;

    return CallSignalMessage(
      type: type,
      callId: callId,
      fromPeerId: from,
      toPeerId: to,
      mediaMode: media,
      sdp: json['sdp'] as String?,
      sdpType: json['sdpType'] as String?,
      iceCandidate: json['candidate'] as String?,
      iceSdpMid: json['sdpMid'] as String?,
      iceSdpMLineIndex: (json['sdpMLineIndex'] as num?)?.toInt(),
      reason: json['reason'] as String?,
    );
  }
}

/// Sink the app/core bridge implements so [CallController] can send signals.
abstract class CallSignalTransport {
  /// Deliver [message] to [message.toPeerId] over core's E2E channel.
  Future<void> send(CallSignalMessage message);
}
