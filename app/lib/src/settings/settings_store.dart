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

  /// Default on so the UI runs before a native core is linked.
  bool mock = true;
  String displayName = '';
  ThemeMode themeMode = ThemeMode.dark;
  bool enableMdns = false;
  String torSocks = '';

  static Future<SettingsStore> load() async {
    final prefs = await SharedPreferences.getInstance();
    final store = SettingsStore(prefs: prefs);
    store.mock = prefs.getBool(mockKey) ?? true;
    store.displayName = prefs.getString(nameKey) ?? '';
    store.themeMode = _theme(prefs.getString(themeKey));
    store.enableMdns = prefs.getBool(mdnsKey) ?? false;
    store.torSocks = prefs.getString(torKey) ?? '';
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
  }

  static ThemeMode _theme(String? raw) {
    return switch (raw) {
      'light' => ThemeMode.light,
      'system' => ThemeMode.system,
      _ => ThemeMode.dark,
    };
  }
}
