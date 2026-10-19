import 'dart:async';

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

/// Low-level WebRTC session for one 1:1 call.
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

  Future<void> startAsCaller() async {
    _setState(CallSessionState.creating);
    await _ensurePeerConnection();
    await _attachLocalMedia();
    await transport.send(CallSignalMessage(
      type: CallSignalType.invite,
      callId: callId,
      fromPeerId: localPeerId,
      toPeerId: remotePeerId,
      mediaMode: mediaMode,
    ));
    _setState(CallSessionState.ringingOut);
  }

  /// Callee: ring UI only; media starts on [accept].
  Future<void> notifyIncoming() async {
    _setState(CallSessionState.ringingIn);
  }

  Future<void> accept() async {
    if (!isCaller && state != CallSessionState.ringingIn) return;
    _setState(CallSessionState.creating);
    await _ensurePeerConnection();
    await _attachLocalMedia();
    await transport.send(CallSignalMessage(
      type: CallSignalType.accept,
      callId: callId,
      fromPeerId: localPeerId,
      toPeerId: remotePeerId,
      mediaMode: mediaMode,
    ));
    _setState(CallSessionState.connecting);
  }

  Future<void> reject({String? reason}) async {
    await transport.send(CallSignalMessage(
      type: CallSignalType.reject,
      callId: callId,
      fromPeerId: localPeerId,
      toPeerId: remotePeerId,
      mediaMode: mediaMode,
      reason: reason,
    ));
    await dispose(failed: false);
  }

  Future<void> hangup({String? reason}) async {
    await transport.send(CallSignalMessage(
      type: CallSignalType.hangup,
      callId: callId,
      fromPeerId: localPeerId,
      toPeerId: remotePeerId,
      mediaMode: mediaMode,
      reason: reason,
    ));
    await dispose(failed: false);
  }

  Future<void> onRemoteSignal(CallSignalMessage msg) async {
    if (msg.callId != callId) return;
    switch (msg.type) {
      case CallSignalType.accept:
        if (isCaller) {
          _setState(CallSessionState.connecting);
          await _createAndSendOffer();
        }
      case CallSignalType.reject:
      case CallSignalType.hangup:
        await dispose(failed: false);
      case CallSignalType.sdpOffer:
        await _handleRemoteOffer(msg);
      case CallSignalType.sdpAnswer:
        await _handleRemoteAnswer(msg);
      case CallSignalType.iceCandidate:
        await _handleRemoteIce(msg);
      case CallSignalType.invite:
        // Handled by controller before session create.
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
      await transport.send(CallSignalMessage(
        type: CallSignalType.iceCandidate,
        callId: callId,
        fromPeerId: localPeerId,
        toPeerId: remotePeerId,
        mediaMode: mediaMode,
        iceCandidate: c.candidate,
        iceSdpMid: c.sdpMid,
        iceSdpMLineIndex: c.sdpMLineIndex,
      ));
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
          ? {
              'facingMode': 'user',
            }
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
    await transport.send(CallSignalMessage(
      type: CallSignalType.sdpOffer,
      callId: callId,
      fromPeerId: localPeerId,
      toPeerId: remotePeerId,
      mediaMode: mediaMode,
      sdp: offer.sdp,
      sdpType: offer.type,
    ));
  }

  Future<void> _handleRemoteOffer(CallSignalMessage msg) async {
    await _ensurePeerConnection();
    if (_localStream == null) await _attachLocalMedia();
    await _pc!.setRemoteDescription(
      RTCSessionDescription(msg.sdp, msg.sdpType ?? 'offer'),
    );
    final answer = await _pc!.createAnswer();
    await _pc!.setLocalDescription(answer);
    await transport.send(CallSignalMessage(
      type: CallSignalType.sdpAnswer,
      callId: callId,
      fromPeerId: localPeerId,
      toPeerId: remotePeerId,
      mediaMode: mediaMode,
      sdp: answer.sdp,
      sdpType: answer.type,
    ));
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
