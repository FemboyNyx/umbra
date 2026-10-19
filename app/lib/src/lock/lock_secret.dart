import 'dart:convert';
import 'dart:math';

import 'package:crypto/crypto.dart';

import '../models/core_types.dart';
import '../settings/settings_store.dart';

const lockPassphraseMin = 4;
const lockPassphraseMax = 128;

void validateLockPassphrase(String passphrase) {
  if (passphrase.length < lockPassphraseMin ||
      passphrase.length > lockPassphraseMax) {
    throw const UmbraException('use 4–128 characters');
  }
}

String newLockSalt() {
  final random = Random.secure();
  final bytes = List<int>.generate(16, (_) => random.nextInt(256));
  return bytes.map((b) => b.toRadixString(16).padLeft(2, '0')).join();
}

String lockVerifier(String passphrase, String saltHex) {
  final salt = _hex(saltHex);
  return Hmac(sha256, salt).convert(utf8.encode(passphrase)).toString();
}

bool lockPassphraseMatches(SettingsStore settings, String passphrase) {
  if (settings.lockSalt.isEmpty || settings.lockHash.isEmpty) return false;
  return lockVerifier(passphrase, settings.lockSalt) == settings.lockHash;
}

List<int> _hex(String value) {
  if (value.length.isOdd) return const [];
  final out = <int>[];
  for (var i = 0; i < value.length; i += 2) {
    final byte = int.tryParse(value.substring(i, i + 2), radix: 16);
    if (byte == null) return const [];
    out.add(byte);
  }
  return out;
}
