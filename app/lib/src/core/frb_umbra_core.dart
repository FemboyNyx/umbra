import 'dart:async';
import 'dart:typed_data';

import '../models/core_types.dart' as app;
import '../rust/api/types.dart' as rust;
import '../rust/api/umbra.dart' as rust_api;
import '../rust/frb_generated.dart';
import 'umbra_core.dart';

/// Forwards to the generated `flutter_rust_bridge` 2.13.0 bindings in
/// `lib/src/rust/`.
class FrbUmbraCore implements UmbraCore {
  StreamSubscription<rust.CoreEvent>? _events;
  var _libraryReady = false;

  Future<void> _ensureLibrary() async {
    if (_libraryReady) return;
    await RustLib.init();
    _libraryReady = true;
  }

  @override
  Future<app.SelfInfo> init({
    required String dataDir,
    required Uint8List dbKey,
    required app.CoreConfig cfg,
  }) async {
    await _ensureLibrary();
    return _guard(() async {
      final info = await rust_api.init(
        dataDir: dataDir,
        dbKey: dbKey,
        cfg: rust.CoreConfig(
          bootstrapAddrs: cfg.bootstrapAddrs,
          relayAddrs: cfg.relayAddrs,
          enableMdns: cfg.enableMdns,
          torSocks: cfg.torSocks,
          listenAddrs: cfg.listenAddrs,
          mock: cfg.mock,
        ),
      );
      return _self(info);
    });
  }

  @override
  Future<void> startNetwork() => _guard(rust_api.startNetwork);

  @override
  Future<void> stopNetwork() => _guard(rust_api.stopNetwork);

  @override
  void listen(void Function(app.CoreEvent event) onEvent) {
    _events?.cancel();
    _events = rust_api.events().listen((event) {
      onEvent(_event(event));
    });
  }

  @override
  Future<app.NetworkInfo> networkStatus() =>
      _guard(() async => _network(await rust_api.networkStatus()));

  @override
  Future<app.SelfInfo> selfInfo() =>
      _guard(() async => _self(await rust_api.selfInfo()));

  @override
  Future<void> setDisplayName(String name) =>
      _guard(() => rust_api.setDisplayName(name: name));

  @override
  Future<app.ContactCard> parseQr(String payload) async {
    await _ensureLibrary();
    return _guard(() async => _card(await rust_api.parseQr(payload: payload)));
  }

  @override
  Future<app.Contact> addContact(
    app.ContactCard card, {
    required bool verified,
  }) => _guard(
    () async => _contact(
      await rust_api.addContact(card: _rustCard(card), verified: verified),
    ),
  );

  @override
  Future<List<app.Contact>> listContacts() => _guard(
    () async => [
      for (final contact in await rust_api.listContacts()) _contact(contact),
    ],
  );

  @override
  Future<void> setVerified(String peerId, {required bool verified}) =>
      _guard(() => rust_api.setVerified(peerId: peerId, verified: verified));

  @override
  Future<app.Contact> acceptKeyChange(String peerId) => _guard(
    () async => _contact(await rust_api.acceptKeyChange(peerId: peerId)),
  );

  @override
  Future<void> rejectKeyChange(String peerId) =>
      _guard(() => rust_api.rejectKeyChange(peerId: peerId));

  @override
  Future<void> removeContact(String peerId) =>
      _guard(() => rust_api.removeContact(peerId: peerId));

  @override
  Future<String> sendText(String peerId, String body) =>
      _guard(() => rust_api.sendText(peerId: peerId, body: body));

  @override
  Future<List<app.ConversationSummary>> listConversations() => _guard(
    () async => [
      for (final item in await rust_api.listConversations())
        _conversation(item),
    ],
  );

  @override
  Future<List<app.Message>> listMessages(
    String peerId, {
    int? before,
    int limit = 50,
  }) => _guard(
    () async => [
      for (final message in await rust_api.listMessages(
        peerId: peerId,
        before: before,
        limit: limit,
      ))
        _message(message),
    ],
  );

  @override
  Future<void> markRead(String peerId, String upTo) =>
      _guard(() => rust_api.markRead(peerId: peerId, upTo: upTo));

  @override
  Future<void> sendSignal({
    required String peerId,
    required String callId,
    required app.SignalKind kind,
    required String payload,
  }) => _guard(
    () => rust_api.sendSignal(
      peerId: peerId,
      callId: callId,
      kind: _rustKind(kind),
      payload: payload,
    ),
  );

  Future<T> _guard<T>(Future<T> Function() body) async {
    try {
      return await body();
    } on app.UmbraException {
      rethrow;
    } catch (err) {
      throw app.UmbraException(err.toString());
    }
  }
}

app.CoreEvent _event(rust.CoreEvent event) {
  return event.when(
    messageReceived: (message) => app.MessageReceived(_message(message)),
    messageStatus: (id, peerId, status) => app.MessageStatusChanged(
      id: id,
      peerId: peerId,
      status: _status(status),
    ),
    contactOnline: app.ContactOnline.new,
    contactOffline: app.ContactOffline.new,
    contactRequest: (card) => app.ContactRequest(_card(card)),
    signal: (peerId, callId, kind, payload) => app.SignalEvent(
      peerId: peerId,
      callId: callId,
      kind: _kind(kind),
      payload: payload,
    ),
    networkStatus: (info) => app.NetworkStatus(_network(info)),
    keyChanged:
        (
          peerId,
          oldIdentityKey,
          newIdentityKey,
          oldSafetyNumber,
          newSafetyNumber,
          source,
        ) => app.KeyChanged(
          peerId: peerId,
          oldIdentityKey: oldIdentityKey,
          newIdentityKey: newIdentityKey,
          oldSafetyNumber: oldSafetyNumber,
          newSafetyNumber: newSafetyNumber,
          source: _source(source),
        ),
  );
}

app.SelfInfo _self(rust.SelfInfo info) {
  return app.SelfInfo(
    peerId: info.peerId,
    fingerprint: info.fingerprint,
    qrPayload: info.qrPayload,
    displayName: info.displayName,
    identityKey: info.identityKey,
    ed25519Key: info.ed25519Key,
    addrs: info.addrs,
  );
}

app.ContactCard _card(rust.ContactCard card) {
  return app.ContactCard(
    peerId: card.peerId,
    identityKey: card.identityKey,
    ed25519Key: card.ed25519Key,
    addrs: card.addrs,
    displayName: card.displayName,
  );
}

rust.ContactCard _rustCard(app.ContactCard card) {
  return rust.ContactCard(
    peerId: card.peerId,
    identityKey: card.identityKey,
    ed25519Key: card.ed25519Key,
    addrs: card.addrs,
    displayName: card.displayName,
  );
}

app.Contact _contact(rust.Contact contact) {
  return app.Contact(
    peerId: contact.peerId,
    displayName: contact.displayName,
    identityKey: contact.identityKey,
    ed25519Key: contact.ed25519Key,
    addrs: contact.addrs,
    verified: contact.verified,
    safetyNumber: contact.safetyNumber,
    addedAtMs: contact.addedAtMs,
    online: contact.online,
    keyChanged: contact.keyChanged,
    pendingIdentityKey: contact.pendingIdentityKey,
  );
}

app.Message _message(rust.Message message) {
  return app.Message(
    id: message.id,
    peerId: message.peerId,
    outgoing: message.outgoing,
    body: message.body,
    timestampMs: message.timestampMs,
    status: _status(message.status),
    read: message.read,
  );
}

app.ConversationSummary _conversation(rust.ConversationSummary summary) {
  return app.ConversationSummary(
    peerId: summary.peerId,
    displayName: summary.displayName,
    lastMessage: summary.lastMessage == null
        ? null
        : _message(summary.lastMessage!),
    unreadCount: summary.unreadCount,
    updatedAtMs: summary.updatedAtMs,
  );
}

app.NetworkInfo _network(rust.NetworkInfo info) {
  return app.NetworkInfo(
    running: info.running,
    listenAddrs: info.listenAddrs,
    connectedPeers: info.connectedPeers,
    relayReservations: info.relayReservations,
  );
}

app.DeliveryStatus _status(rust.DeliveryStatus status) {
  return switch (status) {
    rust.DeliveryStatus.sent => app.DeliveryStatus.sent,
    rust.DeliveryStatus.delivered => app.DeliveryStatus.delivered,
    rust.DeliveryStatus.failed => app.DeliveryStatus.failed,
    rust.DeliveryStatus.received => app.DeliveryStatus.received,
  };
}

app.SignalKind _kind(rust.SignalKind kind) {
  return switch (kind) {
    rust.SignalKind.offer => app.SignalKind.offer,
    rust.SignalKind.answer => app.SignalKind.answer,
    rust.SignalKind.iceCandidate => app.SignalKind.iceCandidate,
    rust.SignalKind.hangup => app.SignalKind.hangup,
    rust.SignalKind.ringing => app.SignalKind.ringing,
    rust.SignalKind.reject => app.SignalKind.reject,
    rust.SignalKind.accept => app.SignalKind.accept,
    rust.SignalKind.busy => app.SignalKind.busy,
  };
}

rust.SignalKind _rustKind(app.SignalKind kind) {
  return switch (kind) {
    app.SignalKind.offer => rust.SignalKind.offer,
    app.SignalKind.answer => rust.SignalKind.answer,
    app.SignalKind.iceCandidate => rust.SignalKind.iceCandidate,
    app.SignalKind.hangup => rust.SignalKind.hangup,
    app.SignalKind.ringing => rust.SignalKind.ringing,
    app.SignalKind.reject => rust.SignalKind.reject,
    app.SignalKind.accept => rust.SignalKind.accept,
    app.SignalKind.busy => rust.SignalKind.busy,
  };
}

app.KeyChangeSource _source(rust.KeyChangeSource source) {
  return switch (source) {
    rust.KeyChangeSource.hello => app.KeyChangeSource.hello,
    rust.KeyChangeSource.keyClaim => app.KeyChangeSource.keyClaim,
    rust.KeyChangeSource.contactCard => app.KeyChangeSource.contactCard,
  };
}
