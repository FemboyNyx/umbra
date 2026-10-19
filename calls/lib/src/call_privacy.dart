/// Privacy UX for Umbra calls.
///
/// WebRTC media paths use ICE (STUN/TURN). Direct or STUN-assisted paths
/// expose peer IP addresses to each other and cannot be routed over Tor.
/// See Umbra threat model: calls are a deliberate metadata tradeoff.
library;

/// Shown before placing or accepting a call when Tor mode is on, or always
/// as a first-time disclosure (app decides when to surface).
const String preCallIpWarning = '''
Voice and video calls use a direct peer connection when possible. That can '
'reveal your IP address to the other person, and call media cannot go over Tor.

For stronger network privacy, enable “Relay only” so media goes through a TURN '
'relay you configure — slower, but it hides your IP from the other peer.
''';

/// Short one-liner for banners / confirm dialogs.
const String preCallIpWarningShort =
    'Calls can reveal your IP and do not use Tor. Use Relay only to hide it.';

/// User-facing call network policy.
enum CallRelayPolicy {
  /// Allow host/srflx (STUN) candidates; fastest, may expose IPs.
  allowDirect,

  /// Force relay candidates only (requires configured TURN).
  relayOnly,
}

class CallPrivacySettings {
  CallPrivacySettings({
    this.relayPolicy = CallRelayPolicy.allowDirect,
    this.acknowledgeIpRisk = false,
  });

  CallRelayPolicy relayPolicy;
  bool acknowledgeIpRisk;

  bool get requiresTurn => relayPolicy == CallRelayPolicy.relayOnly;

  /// Whether the UI should block starting a call until the user confirms.
  bool get shouldShowPreCallWarning => !acknowledgeIpRisk;
}
