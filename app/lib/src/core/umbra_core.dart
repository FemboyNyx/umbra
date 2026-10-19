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
