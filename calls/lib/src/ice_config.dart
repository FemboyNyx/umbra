/// ICE server configuration for Umbra calls.
///
/// Default STUN servers are well-known public ones and can be replaced app-wide
/// via [IceConfig.defaultStun]. TURN is never hardcoded: the user (or their
/// self-hosted config) supplies TURN URLs and credentials.
///
/// In Tor mode ([IceConfig.torMode]) no STUN server is ever contacted, since a
/// STUN request reveals the device's real public IP to the STUN operator.
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
    this.torMode = false,
  })  : stunServers = stunServers ?? defaultStun,
        turnServers = turnServers ?? const [];

  final List<IceServer> stunServers;
  final List<IceServer> turnServers;
  final CallRelayPolicy relayPolicy;

  /// When true (app is in Tor mode), STUN is never used, regardless of
  /// [stunServers]. Only user-configured TURN servers are passed to WebRTC.
  final bool torMode;

  /// STUN servers that will actually be used: none in Tor mode or relay-only.
  List<IceServer> get effectiveStunServers =>
      (torMode || relayPolicy == CallRelayPolicy.relayOnly)
          ? const []
          : stunServers;

  /// Public STUN only — no private infrastructure, no secrets.
  static const List<IceServer> defaultPublicStun = [
    IceServer(urls: ['stun:stun.l.google.com:19302']),
    IceServer(urls: ['stun:stun1.l.google.com:19302']),
  ];

  /// App-wide default STUN list used when [stunServers] is not passed.
  /// Set this at startup (e.g. from user settings) to use other servers,
  /// or to `const []` to disable STUN entirely.
  static List<IceServer> defaultStun = defaultPublicStun;

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
      ...effectiveStunServers.map((s) => s.toWebRtcMap()),
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
        torMode: torMode,
      );

  IceConfig withRelayPolicy(CallRelayPolicy policy) => IceConfig(
        stunServers: stunServers,
        turnServers: turnServers,
        relayPolicy: policy,
        torMode: torMode,
      );

  IceConfig withTorMode(bool enabled) => IceConfig(
        stunServers: stunServers,
        turnServers: turnServers,
        relayPolicy: relayPolicy,
        torMode: enabled,
      );

  IceConfig withStunServers(List<IceServer> stun) => IceConfig(
        stunServers: stun,
        turnServers: turnServers,
        relayPolicy: relayPolicy,
        torMode: torMode,
      );
}
