import 'dart:async';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:umbra_calls/umbra_calls.dart' show torRelayRequiredMessage;

import '../calls/outgoing_call.dart';
import '../core/core_errors.dart';
import '../core/mock_umbra_core.dart';
import '../core/umbra_core.dart';
import '../keystore/db_key_store.dart';
import '../lock/app_lock.dart';
import '../lock/platform_lock.dart';
import '../models/core_types.dart';
import '../settings/settings_store.dart';

/// Owns startup order: keystore, `init`, `events`, then `start_network`.
class UmbraController extends ChangeNotifier {
  UmbraController({
    required UmbraCore core,
    required this.keyStore,
    required this.settings,
    required this.supportDirectory,
    SoftLockBiometric? biometric,
    ScreenSecurity? screen,
    bool coverOnBlur = false,
    String? captureLimit,
    Duration softLockAfter = const Duration(minutes: 1),
    Duration hardLockAfter = const Duration(hours: 1),
  }) : _core = core {
    appLock = AppLock(
      settings: settings,
      core: () => _core,
      biometric: biometric,
      screen: screen,
      coverOnBlur: coverOnBlur,
      captureLimit: captureLimit,
      softAfter: softLockAfter,
      hardAfter: hardLockAfter,
    );
  }

  UmbraCore _core;
  late final AppLock appLock;
  final DbKeyStore keyStore;
  final SettingsStore settings;
  final Future<String> Function() supportDirectory;

  bool booting = false;
  bool started = false;
  String? error;
  String keystoreBackend = '';
  SelfInfo? self;
  NetworkInfo network = const NetworkInfo();
  List<Contact> contacts = const [];
  List<ConversationSummary> conversations = const [];
  List<ContactCard> pending = const [];
  final Map<String, List<Message>> threads = {};

  final Set<String> _seenRequests = {};
  final Set<String> _dismissed = {};
  final Set<String> openThreads = {};
  var _listening = false;

  bool callsBlockedByTor = false;
  String? callNotice;
  List<MissedCallNotice> missedCallNotices = const [];
  final Map<String, KeyChangeNotice> keyChanges = {};

  /// Set once the call stack is attached. Null until then, so Start call
  /// stays disabled during launch and never touches WebRTC.
  OutgoingCallGuard? outgoing;

  /// True only while the audio probe or `startCall` is in flight.
  /// The calling banner stays hidden until [activeCallPeerId] is set.
  bool callStarting = false;

  /// Peer we are ringing. Null when the UI is idle.
  String? activeCallPeerId;

  /// Applies Tor mode on the live [CallController] when the SOCKS setting changes.
  void Function(bool enabled)? applyTorMode;

  /// Gem's ingress for `CoreEvent::Signal` (`CallController.handleCoreSignal`).
  ///
  /// Offer, Answer, and ICE are not interpreted here. Every signal is forwarded
  /// through this function and nowhere else.
  Future<void> Function({
    required String peerId,
    required String callId,
    required String kindWire,
    required String payload,
  })?
  handleCoreSignal;

  /// Called after `init` and before `listen` / `start_network`, so the calls
  /// ingress is attached before any `CoreEvent::Signal` can arrive.
  void Function(SelfInfo self)? attachCalls;

  UmbraCore get core => _core;

  bool get needsOnboarding => (self?.displayName.trim().isEmpty ?? true);

  List<KeyChangeNotice> get keyChangeNotices {
    final seen = <String>{};
    final out = <KeyChangeNotice>[];
    for (final notice in keyChanges.values) {
      if (seen.add(notice.peerId)) out.add(notice);
    }
    for (final contact in contacts) {
      if (!contact.keyChanged || !seen.add(contact.peerId)) continue;
      out.add(
        KeyChangeNotice(
          peerId: contact.peerId,
          oldIdentityKey: contact.identityKey,
          newIdentityKey: contact.pendingIdentityKey ?? '',
          oldSafetyNumber: contact.safetyNumber,
          newSafetyNumber: '',
          source: KeyChangeSource.hello,
        ),
      );
    }
    return out;
  }

  void setCallsBlockedByTor(bool blocked) {
    if (callsBlockedByTor == blocked) return;
    callsBlockedByTor = blocked;
    notifyListeners();
  }

  void noteMissedCall({
    required String peerId,
    required String callId,
    required String reason,
  }) {
    missedCallNotices = [
      ...missedCallNotices,
      MissedCallNotice(peerId: peerId, callId: callId, reason: reason),
    ];
    notifyListeners();
  }

  void dismissMissedCallAt(int index) {
    if (index < 0 || index >= missedCallNotices.length) return;
    missedCallNotices = [...missedCallNotices]..removeAt(index);
    notifyListeners();
  }

  void dismissCallNotice() {
    if (callNotice == null) return;
    callNotice = null;
    notifyListeners();
  }

  bool keyChangePending(String peerId) {
    if (keyChanges.containsKey(peerId)) return true;
    return contactFor(peerId)?.keyChanged ?? false;
  }

  /// Start call is available for this contact.
  bool canStartCall(String peerId) {
    if (outgoing == null || callsBlockedByTor) return false;
    if (callStarting || activeCallPeerId != null) return false;
    if (contactFor(peerId) == null) return false;
    if (keyChangePending(peerId)) return false;
    return true;
  }

  String startCallTooltip(String peerId) {
    if (callsBlockedByTor) return torRelayRequiredMessage;
    if (keyChangePending(peerId)) return 'Safety number changed';
    if (activeCallPeerId != null) return 'A call is already active';
    return 'Start call';
  }

  /// Probe, then start an outgoing call. Failures stay idle: no calling
  /// banner, and no Ringing unless the peer was already invited.
  Future<void> startOutgoingCall(String peerId) async {
    final guard = outgoing;
    if (guard == null || !canStartCall(peerId)) return;
    callStarting = true;
    callNotice = null;
    notifyListeners();
    try {
      await guard.start(peerId);
      if (guard.client.hasActiveCall) {
        activeCallPeerId = peerId;
      }
    } on NoAudioDeviceException catch (err) {
      activeCallPeerId = null;
      callNotice = err.message;
    } on CallStartException catch (err) {
      activeCallPeerId = null;
      callNotice = err.message;
    } catch (err, stack) {
      debugLogCoreError(err, stack);
      activeCallPeerId = null;
      callNotice = userFacingCallError(err);
    } finally {
      callStarting = false;
      notifyListeners();
    }
  }

  Future<void> cancelOutgoingCall() async {
    final client = outgoing?.client;
    try {
      await client?.cancelCall();
    } catch (err, stack) {
      debugLogCoreError(err, stack);
      callNotice = userFacingCallError(err);
    } finally {
      activeCallPeerId = null;
      callStarting = false;
      notifyListeners();
    }
  }

  /// Remote hangup, reject, or our own teardown finished.
  void noteCallEnded() {
    if (activeCallPeerId == null) return;
    activeCallPeerId = null;
    notifyListeners();
  }

  void noteCallFailed(Object err) {
    final stillUp = outgoing?.client.hasActiveCall ?? false;
    callNotice = userFacingCallError(err);
    if (!stillUp) {
      activeCallPeerId = null;
      callStarting = false;
    }
    notifyListeners();
  }

  Future<void> bootstrap() async {
    if (booting) return;
    booting = true;
    error = null;
    notifyListeners();
    Uint8List? key;
    try {
      final dir = await supportDirectory();
      final material = await keyStore.getOrCreate(storageDir: dir);
      key = Uint8List.fromList(material.bytes);
      _wipe(material.bytes);
      keystoreBackend = material.backend;
      if (key.length != 32) {
        throw const UmbraException('db_key must be 32 bytes');
      }
      final tor = settings.torSocks.trim();
      self = await _core.init(
        dataDir: dir,
        dbKey: key,
        cfg: CoreConfig(
          mock: settings.mock,
          enableMdns: tor.isEmpty && settings.enableMdns,
          torSocks: tor.isEmpty ? null : tor,
        ),
      );
      final saved = settings.displayName.trim();
      if (saved.isNotEmpty && self!.displayName.trim().isEmpty) {
        await _core.setDisplayName(saved);
        self = await _core.selfInfo();
      }
      attachCalls?.call(self!);
      // Subscribe once, before start_network. A second bootstrap replaces it.
      if (!_listening) {
        _core.listen(_onEvent);
        _listening = true;
      }
      try {
        await _core.startNetwork();
      } catch (err, stack) {
        debugLogCoreError(err, stack);
        throw const UmbraException(couldNotStartNetwork);
      }
      network = await _core.networkStatus();
      await _reloadLists();
      started = true;
      error = null;
    } catch (err, stack) {
      if (err is! UmbraException || err.message != couldNotStartNetwork) {
        debugLogCoreError(err, stack);
      }
      error = startupErrorMessage(err);
    } finally {
      _wipe(key);
      booting = false;
      notifyListeners();
    }
  }

  void _wipe(Uint8List? bytes) {
    if (bytes == null) return;
    try {
      for (var i = 0; i < bytes.length; i++) {
        bytes[i] = 0;
      }
    } catch (err, stack) {
      debugLogCoreError(err, stack);
    }
  }

  /// Drop a failed real backend and continue on the in-memory mock.
  Future<void> fallBackToMock() async {
    settings.mock = true;
    await settings.save();
    _core = MockUmbraCore();
    _listening = false;
    started = false;
    pending = const [];
    _seenRequests.clear();
    await bootstrap();
  }

  Future<void> setDisplayName(String name) async {
    final trimmed = name.trim();
    if (trimmed.isEmpty || trimmed.length > 64) {
      throw const UmbraException('display name must be 1–64 characters');
    }
    await _core.setDisplayName(trimmed);
    settings.displayName = trimmed;
    await settings.save();
    self = await _core.selfInfo();
    notifyListeners();
  }

  Future<void> acceptRequest(ContactCard card) async {
    await _core.addContact(card, verified: false);
    pending = [
      for (final item in pending)
        if (item.peerId != card.peerId) item,
    ];
    await _reloadLists();
    notifyListeners();
  }

  void dismissRequest(ContactCard card) {
    _dismissed.add(card.peerId);
    pending = [
      for (final item in pending)
        if (item.peerId != card.peerId) item,
    ];
    notifyListeners();
  }

  Future<Contact> addFromQr(String payload) async {
    final card = await _core.parseQr(payload);
    final contact = await _core.addContact(card, verified: false);
    pending = [
      for (final item in pending)
        if (item.peerId != card.peerId) item,
    ];
    await _reloadLists();
    notifyListeners();
    return contact;
  }

  Future<void> acceptKeyChange(String peerId) async {
    await _core.acceptKeyChange(peerId);
    keyChanges.remove(peerId);
    await _reloadLists();
    notifyListeners();
  }

  Future<void> rejectKeyChange(String peerId) async {
    await _core.rejectKeyChange(peerId);
    keyChanges.remove(peerId);
    await _reloadLists();
    notifyListeners();
  }

  Future<void> setVerified(String peerId, {required bool verified}) async {
    await _core.setVerified(peerId, verified: verified);
    await _reloadLists();
    notifyListeners();
  }

  Future<void> removeContact(String peerId) async {
    await _core.removeContact(peerId);
    threads.remove(peerId);
    await _reloadLists();
    notifyListeners();
  }

  Future<void> sendText(String peerId, String body) async {
    await _core.sendText(peerId, body);
    await _reloadThread(peerId);
    await _reloadLists();
    notifyListeners();
  }

  void setThreadOpen(String peerId, {required bool open}) {
    if (open) {
      openThreads.add(peerId);
      unawaited(markThreadRead(peerId));
    } else {
      openThreads.remove(peerId);
    }
  }

  Future<void> markThreadRead(String peerId) async {
    await _reloadThread(peerId);
    final messages = threads[peerId] ?? const <Message>[];
    if (messages.isNotEmpty) {
      await _core.markRead(peerId, messages.last.id);
      await _reloadThread(peerId);
      await _reloadLists();
    }
    notifyListeners();
  }

  List<Message> messagesFor(String peerId) {
    return threads[peerId] ?? const [];
  }

  Contact? contactFor(String peerId) {
    for (final contact in contacts) {
      if (contact.peerId == peerId) return contact;
    }
    return null;
  }

  Future<void> updateSettings({
    bool? mock,
    ThemeMode? themeMode,
    bool? enableMdns,
    String? torSocks,
  }) async {
    if (mock != null) settings.mock = mock;
    if (themeMode != null) settings.themeMode = themeMode;
    if (enableMdns != null) settings.enableMdns = enableMdns;
    if (torSocks != null) settings.torSocks = torSocks;
    await settings.save();
    if (torSocks != null) {
      applyTorMode?.call(settings.torSocks.trim().isNotEmpty);
    }
    notifyListeners();
  }

  void _onEvent(CoreEvent event) {
    // Hard lock: nothing arrives. NetworkStatus still reports the stop.
    if (appLock.hardLocked && event is! NetworkStatus) return;
    switch (event) {
      case ContactRequest(:final card):
        if (_dismissed.contains(card.peerId) ||
            _seenRequests.contains(card.peerId)) {
          return;
        }
        if (contacts.any((c) => c.peerId == card.peerId)) return;
        _seenRequests.add(card.peerId);
        pending = [...pending, card];
      case MessageReceived(:final message):
        if (appLock.softLocked) appLock.noteLockedTraffic();
        unawaited(_refreshThread(message.peerId));
        return;
      case MessageStatusChanged(:final peerId):
        unawaited(_refreshThread(peerId));
        return;
      case ContactOnline() || ContactOffline():
        unawaited(_refreshLists());
        return;
      case SignalEvent(
        :final peerId,
        :final callId,
        :final kind,
        :final payload,
      ):
        if (appLock.hardLocked) return;
        // Sole ingress into calls/. Do not branch on Offer/Answer/ICE here.
        final forward = handleCoreSignal;
        if (forward != null) {
          unawaited(() async {
            try {
              await forward(
                peerId: peerId,
                callId: callId,
                kindWire: coreSignalKindWire(kind),
                payload: payload,
              );
            } catch (err, stack) {
              debugLogCoreError(err, stack);
              final stillUp = outgoing?.client.hasActiveCall ?? false;
              if (isAudioSetupFailure(err) ||
                  (activeCallPeerId != null && !stillUp)) {
                noteCallFailed(err);
              }
            }
          }());
        }
      case NetworkStatus(:final info):
        network = info;
      case KeyChanged(
        :final peerId,
        :final oldIdentityKey,
        :final newIdentityKey,
        :final oldSafetyNumber,
        :final newSafetyNumber,
        :final source,
      ):
        keyChanges[peerId] = KeyChangeNotice(
          peerId: peerId,
          oldIdentityKey: oldIdentityKey,
          newIdentityKey: newIdentityKey,
          oldSafetyNumber: oldSafetyNumber,
          newSafetyNumber: newSafetyNumber,
          source: source,
        );
        unawaited(_refreshLists());
    }
    notifyListeners();
  }

  Future<void> _refreshThread(String peerId) async {
    await _reloadThread(peerId);
    if (openThreads.contains(peerId)) {
      final messages = threads[peerId] ?? const <Message>[];
      if (messages.isNotEmpty) {
        await _core.markRead(peerId, messages.last.id);
        await _reloadThread(peerId);
      }
    }
    await _reloadLists();
    notifyListeners();
  }

  Future<void> _refreshLists() async {
    await _reloadLists();
    notifyListeners();
  }

  Future<void> _reloadLists() async {
    contacts = await _core.listContacts();
    conversations = await _core.listConversations();
  }

  Future<void> _reloadThread(String peerId) async {
    final newestFirst = await _core.listMessages(peerId, limit: 200);
    threads[peerId] = newestFirst.reversed.toList();
  }
}
