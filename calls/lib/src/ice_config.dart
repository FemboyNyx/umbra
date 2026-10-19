/// ICE server configuration for Umbra calls.
///
/// Default STUN servers are well-known public ones. TURN is never hardcoded:
/// the user (or their self-hosted config) supplies TURN URLs and credentials.
library;

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
}

class IceConfig {
  IceConfig({List<IceServer>? stunServers, List<IceServer>? turnServers})
      : stunServers = stunServers ?? defaultPublicStun,
        turnServers = turnServers ?? const [];

  final List<IceServer> stunServers;
  final List<IceServer> turnServers;

  /// Public STUN only — no private infrastructure, no secrets.
  static const List<IceServer> defaultPublicStun = [
    IceServer(urls: ['stun:stun.l.google.com:19302']),
    IceServer(urls: ['stun:stun1.l.google.com:19302']),
  ];

  /// Merge STUN + user TURN for `RTCPeerConnection` configuration.
  Map<String, dynamic> toPeerConnectionConfig() => {
        'iceServers': [
          ...stunServers.map((s) => s.toWebRtcMap()),
          ...turnServers.map((s) => s.toWebRtcMap()),
        ],
        'sdpSemantics': 'unified-plan',
      };

  IceConfig withTurn(List<IceServer> turn) => IceConfig(
        stunServers: stunServers,
        turnServers: turn,
      );
}
