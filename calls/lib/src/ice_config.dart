/// ICE server configuration for Umbra calls.
///
/// Default STUN servers are well-known public ones. TURN is never hardcoded:
/// the user (or their self-hosted config) supplies TURN URLs and credentials.
library;

import 'call_privacy.dart';

class IceServer {
  const IceServer({
    required this.urls,
    this.username,
    this.credential,
  });

  /// One or more `stun:` / `turn:` / `turns:` URLs.
  final List<String> urls;
  final String? username;
  final String? credential;

  Map<String, dynamic> toWebRtcMap() {
    final map = <String, dynamic>{'urls': urls};
    if (username != null) map['username'] = username;
    if (credential != null) map['credential'] = credential;
    return map;
  }

  bool get isTurn =>
      urls.any((u) => u.startsWith('turn:') || u.startsWith('turns:'));
}

class IceConfig {
  IceConfig({
    List<IceServer>? stunServers,
    List<IceServer>? turnServers,
    this.relayPolicy = CallRelayPolicy.allowDirect,
  })  : stunServers = stunServers ?? defaultPublicStun,
        turnServers = turnServers ?? const [];

  final List<IceServer> stunServers;
  final List<IceServer> turnServers;
  final CallRelayPolicy relayPolicy;

  /// Public STUN only — no private infrastructure, no secrets.
  static const List<IceServer> defaultPublicStun = [
    IceServer(urls: ['stun:stun.l.google.com:19302']),
    IceServer(urls: ['stun:stun1.l.google.com:19302']),
  ];

  bool get hasTurn => turnServers.any((s) => s.isTurn);

  /// True when relay-only is requested but no TURN is configured.
  bool get relayOnlyBlocked =>
      relayPolicy == CallRelayPolicy.relayOnly && !hasTurn;

  /// Merge STUN + user TURN for `RTCPeerConnection` configuration.
  ///
  /// In [CallRelayPolicy.relayOnly], STUN is omitted and
  /// `iceTransportPolicy` is set to `relay` so host/srflx candidates
  /// are not used (requires TURN).
  Map<String, dynamic> toPeerConnectionConfig() {
    final relayOnly = relayPolicy == CallRelayPolicy.relayOnly;
    final servers = <Map<String, dynamic>>[
      if (!relayOnly) ...stunServers.map((s) => s.toWebRtcMap()),
      ...turnServers.map((s) => s.toWebRtcMap()),
    ];
    return {
      'iceServers': servers,
      'sdpSemantics': 'unified-plan',
      if (relayOnly) 'iceTransportPolicy': 'relay',
    };
  }

  IceConfig withTurn(List<IceServer> turn) => IceConfig(
        stunServers: stunServers,
        turnServers: turn,
        relayPolicy: relayPolicy,
      );

  IceConfig withRelayPolicy(CallRelayPolicy policy) => IceConfig(
        stunServers: stunServers,
        turnServers: turnServers,
        relayPolicy: policy,
      );
}
