import 'dart:typed_data';

import '../models/core_types.dart';

/// The coarse FFI surface from `docs/core-api.md`.
///
/// [MockUmbraCore] implements it in Dart. [FrbUmbraCore] forwards to the
/// generated flutter_rust_bridge bindings. Reads return futures because the
/// generated bridge crosses the FFI thread pool.
abstract class UmbraCore {
  Future<SelfInfo> init({
    required String dataDir,
    required Uint8List dbKey,
    required CoreConfig cfg,
  });

  Future<void> startNetwork();

  Future<void> stopNetwork();

  /// Hard lock. Phase 0's FFI has no `lock()`.
  ///
  /// `docs/message-protection.md` says this stops the network, closes
  /// SQLCipher, and zeroizes `db_key` and the pickle keys. A passphrase
  /// would also wrap `db_key` with Argon2id. [FrbUmbraCore] can only stop
  /// the network. [MockUmbraCore] stops the network and drops events.
  Future<void> lock();

  /// Reverse [lock] as far as the backend can. The real core cannot reopen
  /// a database it never closed.
  Future<void> unlock();

  /// Subscribe once. A second call replaces the previous listener.
  ///
  /// Call this before [startNetwork].
  void listen(void Function(CoreEvent event) onEvent);

  Future<NetworkInfo> networkStatus();

  Future<SelfInfo> selfInfo();

  Future<void> setDisplayName(String name);

  Future<ContactCard> parseQr(String payload);

  Future<Contact> addContact(ContactCard card, {required bool verified});

  Future<List<Contact>> listContacts();

  Future<void> setVerified(String peerId, {required bool verified});

  /// Trust the pending key. The contact comes back with `verified` false.
  Future<Contact> acceptKeyChange(String peerId);

  /// Keep the old key and remember the refused one so it does not alert again.
  Future<void> rejectKeyChange(String peerId);

  Future<void> removeContact(String peerId);

  Future<String> sendText(String peerId, String body);

  Future<List<ConversationSummary>> listConversations();

  /// Newest first. [before] is an exclusive `timestampMs` cursor.
  Future<List<Message>> listMessages(
    String peerId, {
    int? before,
    int limit = 50,
  });

  Future<void> markRead(String peerId, String upTo);

  Future<void> sendSignal({
    required String peerId,
    required String callId,
    required SignalKind kind,
    required String payload,
  });
}
