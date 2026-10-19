import 'package:flutter/services.dart';

/// 32-byte SQLCipher key plus the backend that stored it.
///
/// The bytes are the only secret the Dart side holds, and only long enough
/// to pass them to `init`. Identity private keys stay in Rust.
class DbKeyMaterial {
  const DbKeyMaterial({required this.bytes, required this.backend});

  final Uint8List bytes;
  final String backend;
}

abstract class DbKeyStore {
  Future<DbKeyMaterial> getOrCreate({required String storageDir});
}

/// In-memory store for tests. The key is generated once per instance.
class MemoryDbKeyStore implements DbKeyStore {
  MemoryDbKeyStore({Uint8List? seed}) : _seed = seed;

  final Uint8List? _seed;
  Uint8List? _key;

  @override
  Future<DbKeyMaterial> getOrCreate({required String storageDir}) async {
    _key ??= _seed == null
        ? Uint8List.fromList(List<int>.generate(32, (i) => i + 1))
        : Uint8List.fromList(_seed);
    if (_key!.length != 32) {
      throw StateError('db_key must be 32 bytes');
    }
    return DbKeyMaterial(bytes: Uint8List.fromList(_key!), backend: 'memory');
  }
}

/// OS keystore via the platform channel.
///
/// Android Keystore, Windows DPAPI, Linux libsecret. If libsecret is missing
/// at runtime the Linux side falls back to a mode-0600 file and reports
/// `backend: file`.
class PlatformDbKeyStore implements DbKeyStore {
  static const _channel = MethodChannel('io.github.femboynyx.umbra/keystore');

  @override
  Future<DbKeyMaterial> getOrCreate({required String storageDir}) async {
    final result = await _channel.invokeMapMethod<String, Object?>(
      'getOrCreateDbKey',
      {'storageDir': storageDir},
    );
    if (result == null) {
      throw StateError('keystore returned nothing');
    }
    final bytes = result['bytes'];
    final backend = result['backend'];
    if (bytes is! Uint8List || bytes.length != 32) {
      throw StateError('keystore returned a db_key that is not 32 bytes');
    }
    if (backend is! String || backend.isEmpty) {
      throw StateError('keystore did not name a backend');
    }
    return DbKeyMaterial(bytes: bytes, backend: backend);
  }
}
