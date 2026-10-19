/// Call signaling via core `send_signal` / `CoreEvent::Signal`.
///
/// SignalKind: Ringing | Accept | Busy | Reject | Offer | Answer | IceCandidate | Hangup
library;

enum SignalKind {
  ringing,
  accept,
  busy,
  reject,
  offer,
  answer,
  iceCandidate,
  hangup,
}

extension SignalKindWire on SignalKind {
  String get wire {
    switch (this) {
      case SignalKind.ringing:
        return 'Ringing';
      case SignalKind.accept:
        return 'Accept';
      case SignalKind.busy:
        return 'Busy';
      case SignalKind.reject:
        return 'Reject';
      case SignalKind.offer:
        return 'Offer';
      case SignalKind.answer:
        return 'Answer';
      case SignalKind.iceCandidate:
        return 'IceCandidate';
      case SignalKind.hangup:
        return 'Hangup';
    }
  }

  static SignalKind? tryParse(String raw) {
    for (final v in SignalKind.values) {
      if (v.wire == raw || v.name.toLowerCase() == raw.toLowerCase()) {
        return v;
      }
    }
    switch (raw) {
      case 'call.invite':
        return SignalKind.ringing;
      case 'call.accept':
        return SignalKind.accept;
      case 'call.reject':
        return SignalKind.reject;
      case 'call.hangup':
        return SignalKind.hangup;
      case 'call.sdp.offer':
        return SignalKind.offer;
      case 'call.sdp.answer':
        return SignalKind.answer;
      case 'call.ice':
        return SignalKind.iceCandidate;
      default:
        return null;
    }
  }
}

enum CallMediaMode { audioOnly, audioVideo }

class CallSignalMessage {
  CallSignalMessage({
    required this.kind,
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

  final SignalKind kind;
  final String callId;
  final String fromPeerId;
  final String toPeerId;
  final CallMediaMode mediaMode;
  final String? sdp;
  final String? sdpType;
  final String? iceCandidate;
  final String? iceSdpMid;
  final int? iceSdpMLineIndex;
  final String? reason;

  Map<String, Object?> payloadMap() => {
        'media': mediaMode == CallMediaMode.audioVideo ? 'av' : 'audio',
        if (sdp != null) 'sdp': sdp,
        if (sdpType != null) 'sdpType': sdpType,
        if (iceCandidate != null) 'candidate': iceCandidate,
        if (iceSdpMid != null) 'sdpMid': iceSdpMid,
        if (iceSdpMLineIndex != null) 'sdpMLineIndex': iceSdpMLineIndex,
        if (reason != null) 'reason': reason,
      };

  String toPayload() {
    final map = payloadMap();
    final parts = <String>[];
    for (final e in map.entries) {
      final v = e.value;
      if (v is String) {
        parts.add('"${e.key}":"${_escape(v)}"');
      } else if (v is num || v is bool) {
        parts.add('"${e.key}":$v');
      }
    }
    return '{${parts.join(',')}}';
  }

  static String _escape(String s) => s
      .replaceAll(r'\', r'\\')
      .replaceAll('"', r'\"')
      .replaceAll('\n', r'\n')
      .replaceAll('\r', r'\r');

  Map<String, Object?> toJson() => {
        'kind': kind.wire,
        'type': kind.wire,
        'callId': callId,
        'from': fromPeerId,
        'to': toPeerId,
        ...payloadMap(),
      };

  static CallSignalMessage? parse(Map<String, Object?> json) {
    final typeRaw = (json['kind'] ?? json['type']) as String?;
    if (typeRaw == null) return null;
    final kind = SignalKindWire.tryParse(typeRaw);
    if (kind == null) return null;
    final callId = json['callId'];
    final from = json['from'] ?? json['peer_id'] ?? json['fromPeerId'];
    final to = json['to'] ?? json['toPeerId'];
    if (callId is! String || from is! String) return null;
    final media = json['media'] == 'av'
        ? CallMediaMode.audioVideo
        : CallMediaMode.audioOnly;
    return CallSignalMessage(
      kind: kind,
      callId: callId,
      fromPeerId: from,
      toPeerId: to is String ? to : '',
      mediaMode: media,
      sdp: json['sdp'] as String?,
      sdpType: json['sdpType'] as String?,
      iceCandidate: json['candidate'] as String?,
      iceSdpMid: json['sdpMid'] as String?,
      iceSdpMLineIndex: (json['sdpMLineIndex'] as num?)?.toInt(),
      reason: json['reason'] as String?,
    );
  }

  static CallSignalMessage? fromCore({
    required String peerId,
    required String localPeerId,
    required String callId,
    required String kindWire,
    Map<String, Object?> payload = const {},
  }) {
    // Routing fields come only from core's authenticated Signal event. The
    // payload is written by the remote peer, so it is spread first and can
    // never override who sent the signal, which call it is for, or its kind.
    return parse({
      ...payload,
      'kind': kindWire,
      'type': kindWire,
      'callId': callId,
      'from': peerId,
      'to': localPeerId,
    });
  }
}

abstract class CallSignalTransport {
  Future<void> send(CallSignalMessage message);
}

/// Adapts core `send_signal(peer_id, call_id, kind, payload)`.
class CoreSignalTransport implements CallSignalTransport {
  CoreSignalTransport(this._send);
  final Future<void> Function({
    required String peerId,
    required String callId,
    required SignalKind kind,
    required String payload,
  }) _send;

  @override
  Future<void> send(CallSignalMessage message) => _send(
        peerId: message.toPeerId,
        callId: message.callId,
        kind: message.kind,
        payload: message.toPayload(),
      );
}
