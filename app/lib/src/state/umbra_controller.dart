import 'dart:async';
import 'dart:typed_data';

import 'package:flutter/material.dart';

import '../core/mock_umbra_core.dart';
import '../core/umbra_core.dart';
import '../keystore/db_key_store.dart';
import '../models/core_types.dart';
import '../settings/settings_store.dart';

/// Owns startup order: keystore, `init`, `events`, then `start_network`.
class UmbraController extends ChangeNotifier {
  UmbraController({
    required UmbraCore core,
    required this.keyStore,
    required this.settings,
    required this.supportDirectory,
  }) : _core = core;

  UmbraCore _core;
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
  var _listening = false;

  /// Gem's ingress for `CoreEvent::Signal` (`CallController.handleCoreSignal`).
  ///
  /// Offer, Answer, and ICE are not interpreted here. Every signal is forwarded
  /// through this function and nowhere else.
  Future<void> Function({
    required String peerId,
    required String callId,
    required String kind,
    required String payload,
  })?
  handleCoreSignal;

  /// Called after `init` and before `listen` / `start_network`, so the calls
  /// ingress is attached before any `CoreEvent::Signal` can arrive.
  void Function(SelfInfo self)? attachCalls;

  UmbraCore get core => _core;

  bool get needsOnboarding => (self?.displayName.trim().isEmpty ?? true);

  Future<void> bootstrap() async {
    if (booting) return;
    booting = true;
    error = null;
    notifyListeners();
    Uint8List? key;
    try {
      final dir = await supportDirectory();
      final material = await keyStore.getOrCreate(storageDir: dir);
      key = material.bytes;
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
      await _core.startNetwork();
      network = await _core.networkStatus();
      await _reloadLists();
      started = true;
      error = null;
    } catch (err) {
      error = err.toString();
    } finally {
      if (key != null) {
        for (var i = 0; i < key.length; i++) {
          key[i] = 0;
        }
      }
      booting = false;
      notifyListeners();
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
    notifyListeners();
  }

  void _onEvent(CoreEvent event) {
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
        // Sole ingress into calls/. Do not branch on Offer/Answer/ICE here.
        final forward = handleCoreSignal;
        if (forward != null) {
          unawaited(() async {
            try {
              await forward(
                peerId: peerId,
                callId: callId,
                kind: coreSignalKindWire(kind),
                payload: payload,
              );
            } catch (err, stack) {
              debugPrint('handleCoreSignal failed: $err\n$stack');
            }
          }());
        }
      case NetworkStatus(:final info):
        network = info;
    }
    notifyListeners();
  }

  Future<void> _refreshThread(String peerId) async {
    await _reloadThread(peerId);
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
