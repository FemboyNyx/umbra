/// ICE server configuration for Umbra calls.
///
/// Default STUN servers are well-known public ones and can be replaced app-wide
/// via [IceConfig.defaultStun]. TURN is never hardcoded: the user (or their
/// self-hosted config) supplies TURN URLs and credentials.
///
/// In Tor mode ([IceConfig.torMode]) calls are forced to relay-only: no STUN
/// server is contacted and `iceTransportPolicy` is `relay`, so a direct P2P
/// path (which would reveal the real IP) is never used. Without TURN, calls in
/// Tor mode fail closed; there is no fallback to direct.
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

  /// True when only relay (TURN) candidates may be used: relay-only policy,
  /// or Tor mode (which always forces relay).
  bool get forcesRelay => torMode || relayPolicy == CallRelayPolicy.relayOnly;

  /// STUN servers that will actually be used: none when relay is forced.
  List<IceServer> get effectiveStunServers =>
      forcesRelay ? const [] : stunServers;

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
  bool get relayOnlyBlocked => forcesRelay && !hasTurn;

  /// True when Tor mode is on but no TURN server exists, so calls can't run.
  bool get torRelayBlocked => torMode && !hasTurn;

  /// Merge STUN + user TURN for `RTCPeerConnection` configuration.
  ///
  /// In [CallRelayPolicy.relayOnly], STUN is omitted and
  /// `iceTransportPolicy` is set to `relay` so host/srflx candidates
  /// are not used (requires TURN).
  Map<String, dynamic> toPeerConnectionConfig() {
    final relayOnly = forcesRelay;
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
