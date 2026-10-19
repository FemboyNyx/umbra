import 'package:flutter/material.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// UI preferences. The database key is never stored here.
class SettingsStore {
  SettingsStore({SharedPreferences? prefs}) : _prefs = prefs;

  final SharedPreferences? _prefs;

  static const mockKey = 'umbra.mock';
  static const nameKey = 'umbra.displayName';
  static const themeKey = 'umbra.theme';
  static const mdnsKey = 'umbra.enableMdns';
  static const torKey = 'umbra.torSocks';
  static const softLockKey = 'umbra.softLock';
  static const hardLockKey = 'umbra.hardLock';
  static const screenSecurityKey = 'umbra.screenSecurity';
  static const lockSaltKey = 'umbra.lockSalt';
  static const lockHashKey = 'umbra.lockHash';

  /// Default on so the UI runs before a native core is linked.
  bool mock = true;
  String displayName = '';
  ThemeMode themeMode = ThemeMode.dark;
  bool enableMdns = false;
  String torSocks = '';

  /// Both locks are opt-in. Off until the user sets a PIN or passphrase.
  bool softLock = false;
  bool hardLock = false;

  /// Global screen security. Off until the user turns it on. A disappearing
  /// chat or the view-once viewer would also turn it on; those are not built.
  bool screenSecurity = false;

  /// HMAC-SHA256 verifier for the UI gate. Not the Argon2id wrap, which
  /// core `lock()` would do and which is not in the FFI. The passphrase
  /// itself is never stored.
  String lockSalt = '';
  String lockHash = '';

  static Future<SettingsStore> load() async {
    final prefs = await SharedPreferences.getInstance();
    final store = SettingsStore(prefs: prefs);
    store.mock = prefs.getBool(mockKey) ?? true;
    store.displayName = prefs.getString(nameKey) ?? '';
    store.themeMode = _theme(prefs.getString(themeKey));
    store.enableMdns = prefs.getBool(mdnsKey) ?? false;
    store.torSocks = prefs.getString(torKey) ?? '';
    store.softLock = prefs.getBool(softLockKey) ?? false;
    store.hardLock = prefs.getBool(hardLockKey) ?? false;
    store.screenSecurity = prefs.getBool(screenSecurityKey) ?? false;
    store.lockSalt = prefs.getString(lockSaltKey) ?? '';
    store.lockHash = prefs.getString(lockHashKey) ?? '';
    return store;
  }

  Future<void> save() async {
    final prefs = _prefs;
    if (prefs == null) return;
    await prefs.setBool(mockKey, mock);
    await prefs.setString(nameKey, displayName);
    await prefs.setString(themeKey, themeMode.name);
    await prefs.setBool(mdnsKey, enableMdns);
    await prefs.setString(torKey, torSocks);
    await prefs.setBool(softLockKey, softLock);
    await prefs.setBool(hardLockKey, hardLock);
    await prefs.setBool(screenSecurityKey, screenSecurity);
    await prefs.setString(lockSaltKey, lockSalt);
    await prefs.setString(lockHashKey, lockHash);
  }

  static ThemeMode _theme(String? raw) {
    return switch (raw) {
      'light' => ThemeMode.light,
      'system' => ThemeMode.system,
      _ => ThemeMode.dark,
    };
  }
}
