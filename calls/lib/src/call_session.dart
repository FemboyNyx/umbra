import 'dart:async';
import 'dart:convert';

import 'package:flutter_webrtc/flutter_webrtc.dart';

import 'ice_config.dart';
import 'signaling.dart';

enum CallSessionState {
  idle,
  creating,
  ringingOut,
  ringingIn,
  connecting,
  connected,
  ended,
  failed,
}

/// 1:1 WebRTC session. Flow: Ringing → Accept|Busy|Reject → Offer → Answer → ICE → Hangup.
class CallSession {
  CallSession({
    required this.callId,
    required this.localPeerId,
    required this.remotePeerId,
    required this.mediaMode,
    required this.iceConfig,
    required this.transport,
    required this.isCaller,
  });

  final String callId;
  final String localPeerId;
  final String remotePeerId;
  final CallMediaMode mediaMode;
  final IceConfig iceConfig;
  final CallSignalTransport transport;
  final bool isCaller;

  final _stateController = StreamController<CallSessionState>.broadcast();
  Stream<CallSessionState> get stateChanges => _stateController.stream;
  CallSessionState state = CallSessionState.idle;

  RTCPeerConnection? _pc;
  MediaStream? _localStream;
  MediaStream? _remoteStream;
  final localRenderer = RTCVideoRenderer();
  final remoteRenderer = RTCVideoRenderer();
  MediaStream? get localStream => _localStream;
  MediaStream? get remoteStream => _remoteStream;

  Future<void> initRenderers() async {
    await localRenderer.initialize();
    await remoteRenderer.initialize();
  }

  void _setState(CallSessionState next) {
    state = next;
    _stateController.add(next);
  }

  CallSignalMessage _msg(
    SignalKind kind, {
    String? reason,
    String? sdp,
    String? sdpType,
    String? ice,
    String? mid,
    int? mLine,
  }) =>
      CallSignalMessage(
        kind: kind,
        callId: callId,
        fromPeerId: localPeerId,
        toPeerId: remotePeerId,
        mediaMode: mediaMode,
        reason: reason,
        sdp: sdp,
        sdpType: sdpType,
        iceCandidate: ice,
        iceSdpMid: mid,
        iceSdpMLineIndex: mLine,
      );

  /// Core does not queue signals; unreachable peer → [CallSessionState.failed].
  Future<void> _sendOrFail(CallSignalMessage message) async {
    try {
      final bytes = utf8.encode(message.toPayload());
      if (bytes.length > 64 * 1024) {
        _setState(CallSessionState.failed);
        throw StateError('Call signal payload exceeds core 64 KiB limit');
      }
      await transport.send(message);
    } catch (_) {
      if (state != CallSessionState.ended && state != CallSessionState.failed) {
        _setState(CallSessionState.failed);
      }
      rethrow;
    }
  }

  Future<void> startAsCaller() async {
    _setState(CallSessionState.creating);
    await _sendOrFail(_msg(SignalKind.ringing));
    _setState(CallSessionState.ringingOut);
  }

  Future<void> notifyIncoming() async {
    _setState(CallSessionState.ringingIn);
  }

  Future<void> accept() async {
    if (!isCaller && state != CallSessionState.ringingIn) return;
    _setState(CallSessionState.creating);
    await _ensurePeerConnection();
    await _attachLocalMedia();
    await _sendOrFail(_msg(SignalKind.accept));
    _setState(CallSessionState.connecting);
  }

  Future<void> reject({String? reason}) async {
    await _sendOrFail(_msg(SignalKind.reject, reason: reason));
    await dispose(failed: false);
  }

  Future<void> busy({String? reason}) async {
    await _sendOrFail(_msg(SignalKind.busy, reason: reason ?? 'busy'));
    await dispose(failed: false);
  }

  Future<void> hangup({String? reason}) async {
    await _sendOrFail(_msg(SignalKind.hangup, reason: reason));
    await dispose(failed: false);
  }

  /// Apply a signal CallController authenticated as CoreEvent::Signal
  /// for this session remote peer. Do not call from UI or other channels.
  Future<void> applyRemoteSignal(CallSignalMessage msg) async {
    if (msg.callId != callId) return;
    switch (msg.kind) {
      case SignalKind.accept:
        if (isCaller && state == CallSessionState.ringingOut) {
          _setState(CallSessionState.connecting);
          await _ensurePeerConnection();
          await _attachLocalMedia();
          await _createAndSendOffer();
        }
      case SignalKind.busy:
      case SignalKind.reject:
      case SignalKind.hangup:
        await dispose(failed: false);
      case SignalKind.offer:
        await _handleRemoteOffer(msg);
      case SignalKind.answer:
        await _handleRemoteAnswer(msg);
      case SignalKind.iceCandidate:
        await _handleRemoteIce(msg);
      case SignalKind.ringing:
        break;
    }
  }

  Future<void> setMicMuted(bool muted) async {
    for (final track in _localStream?.getAudioTracks() ?? const []) {
      track.enabled = !muted;
    }
  }

  Future<void> setCameraEnabled(bool enabled) async {
    for (final track in _localStream?.getVideoTracks() ?? const []) {
      track.enabled = enabled;
    }
  }

  Future<void> _ensurePeerConnection() async {
    if (_pc != null) return;
    _pc = await createPeerConnection(iceConfig.toPeerConnectionConfig());
    _pc!.onIceCandidate = (RTCIceCandidate c) async {
      if (c.candidate == null || c.candidate!.isEmpty) return;
      try {
        await _sendOrFail(_msg(
          SignalKind.iceCandidate,
          ice: c.candidate,
          mid: c.sdpMid,
          mLine: c.sdpMLineIndex,
        ));
      } catch (_) {
        // Already marked failed in _sendOrFail.
      }
    };
    _pc!.onTrack = (RTCTrackEvent event) {
      if (event.streams.isNotEmpty) {
        _remoteStream = event.streams.first;
        remoteRenderer.srcObject = _remoteStream;
      }
    };
    _pc!.onConnectionState = (RTCPeerConnectionState s) {
      if (s == RTCPeerConnectionState.RTCPeerConnectionStateConnected) {
        _setState(CallSessionState.connected);
      } else if (s == RTCPeerConnectionState.RTCPeerConnectionStateFailed) {
        _setState(CallSessionState.failed);
      }
    };
  }

  Future<void> _attachLocalMedia() async {
    final constraints = <String, dynamic>{
      'audio': true,
      'video': mediaMode == CallMediaMode.audioVideo
          ? {'facingMode': 'user'}
          : false,
    };
    _localStream = await navigator.mediaDevices.getUserMedia(constraints);
    localRenderer.srcObject = _localStream;
    for (final track in _localStream!.getTracks()) {
      await _pc!.addTrack(track, _localStream!);
    }
  }

  Future<void> _createAndSendOffer() async {
    final offer = await _pc!.createOffer();
    await _pc!.setLocalDescription(offer);
    await _sendOrFail(_msg(SignalKind.offer, sdp: offer.sdp, sdpType: offer.type));
  }

  Future<void> _handleRemoteOffer(CallSignalMessage msg) async {
    await _ensurePeerConnection();
    if (_localStream == null) await _attachLocalMedia();
    await _pc!.setRemoteDescription(
      RTCSessionDescription(msg.sdp, msg.sdpType ?? 'offer'),
    );
    final answer = await _pc!.createAnswer();
    await _pc!.setLocalDescription(answer);
    await _sendOrFail(_msg(SignalKind.answer, sdp: answer.sdp, sdpType: answer.type));
    _setState(CallSessionState.connecting);
  }

  Future<void> _handleRemoteAnswer(CallSignalMessage msg) async {
    await _pc!.setRemoteDescription(
      RTCSessionDescription(msg.sdp, msg.sdpType ?? 'answer'),
    );
  }

  Future<void> _handleRemoteIce(CallSignalMessage msg) async {
    if (_pc == null || msg.iceCandidate == null) return;
    await _pc!.addCandidate(RTCIceCandidate(
      msg.iceCandidate,
      msg.iceSdpMid,
      msg.iceSdpMLineIndex,
    ));
  }

  Future<void> dispose({bool failed = false}) async {
    _setState(failed ? CallSessionState.failed : CallSessionState.ended);
    await _localStream?.dispose();
    _localStream = null;
    _remoteStream = null;
    localRenderer.srcObject = null;
    remoteRenderer.srcObject = null;
    await _pc?.close();
    _pc = null;
  }

  Future<void> destroy() async {
    await dispose();
    await localRenderer.dispose();
    await remoteRenderer.dispose();
    await _stateController.close();
  }
}
