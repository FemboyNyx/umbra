import 'dart:async';

import 'package:flutter/widgets.dart';

import '../core/core_errors.dart';
import '../core/umbra_core.dart';
import '../models/core_types.dart';
import '../settings/settings_store.dart';
import 'lock_copy.dart';
import 'lock_secret.dart';
import 'platform_lock.dart';

/// Soft lock (1 minute in the background) and hard lock (1 hour idle, or
/// on demand). Both are opt-in.
///
/// Soft lock is a UI gate. Core keeps running. Hard lock calls [UmbraCore.lock].
/// That API is not in the FFI, so the real backend only stops the network.
class AppLock extends ChangeNotifier {
  AppLock({
    required this.settings,
    required UmbraCore Function() core,
    SoftLockBiometric? biometric,
    ScreenSecurity? screen,
    this.softAfter = const Duration(minutes: 1),
    this.hardAfter = const Duration(hours: 1),
    this.coverOnBlur = false,
    this.captureLimit,
    DateTime Function()? clock,
  }) : _core = core,
       _biometric = biometric ?? const UnavailableBiometric(),
       _screen = screen ?? PlatformScreenSecurity(),
       _clock = clock ?? DateTime.now;

  final SettingsStore settings;
  final UmbraCore Function() _core;
  final SoftLockBiometric _biometric;
  final ScreenSecurity _screen;
  final Duration softAfter;
  final Duration hardAfter;
  final DateTime Function() _clock;

  /// True when focus loss should hide the window contents. Set for Linux,
  /// and for any device whose screen-security call reports no native exclude.
  bool coverOnBlur;

  /// Extra sentence under screen security when the OS cannot exclude the
  /// window. Null when native exclusion is in effect.
  String? captureLimit;

  bool softLocked = false;
  bool hardLocked = false;
  bool nativeScreenSecurity = true;
  String? lockedNotice;
  String? lastError;

  bool _focused = true;
  DateTime? _backgroundedAt;
  Timer? _softTimer;
  Timer? _idleTimer;

  bool get hasPassphrase =>
      settings.lockSalt.isNotEmpty && settings.lockHash.isNotEmpty;

  bool get locked => softLocked || hardLocked;

  /// Opaque cover while the window is unfocused and screen security cannot
  /// exclude it. Separate from the lock screen.
  bool get obscured =>
      !locked && coverOnBlur && settings.screenSecurity && !_focused;

  /// Why capture cannot be blocked here, when the OS has no exclude API.
  String? get captureLimitText {
    if (nativeScreenSecurity && !coverOnBlur) return null;
    return captureLimit ?? genericCaptureLimit;
  }

  SoftLockBiometric get biometric => _biometric;

  Future<void> setPassphrase(String passphrase, {String? previous}) async {
    validateLockPassphrase(passphrase);
    if (hasPassphrase &&
        (previous == null || !lockPassphraseMatches(settings, previous))) {
      throw const UmbraException('wrong passphrase');
    }
    final salt = newLockSalt();
    settings.lockSalt = salt;
    settings.lockHash = lockVerifier(passphrase, salt);
    await settings.save();
    notifyListeners();
  }

  Future<void> setSoftLock(bool enabled) async {
    if (enabled && !hasPassphrase) {
      throw const UmbraException('choose a PIN or passphrase first');
    }
    settings.softLock = enabled;
    await settings.save();
    if (!enabled) _softTimer?.cancel();
    notifyListeners();
  }

  Future<void> setHardLock(bool enabled) async {
    if (enabled && !hasPassphrase) {
      throw const UmbraException('choose a PIN or passphrase first');
    }
    settings.hardLock = enabled;
    await settings.save();
    if (enabled) {
      _armIdle();
    } else {
      _idleTimer?.cancel();
    }
    notifyListeners();
  }

  Future<void> setScreenSecurity(bool enabled) async {
    settings.screenSecurity = enabled;
    await settings.save();
    await applyScreenSecurity();
  }

  /// Global switch only. Disappearing-timer chats and the view-once viewer
  /// would also enable this. Those screens are not in the app yet.
  Future<void> applyScreenSecurity() async {
    try {
      final result = await _screen.setEnabled(settings.screenSecurity);
      nativeScreenSecurity = result.native;
      coverOnBlur = !result.native;
    } catch (err, stack) {
      debugLogCoreError(err, stack);
      nativeScreenSecurity = false;
      coverOnBlur = true;
    }
    notifyListeners();
  }

  void noteActivity() {
    if (hardLocked) return;
    _armIdle();
  }

  void noteLifecycle(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) {
      final started = _backgroundedAt;
      _backgroundedAt = null;
      _softTimer?.cancel();
      _focused = true;
      if (settings.softLock &&
          started != null &&
          _clock().difference(started) >= softAfter) {
        softLocked = true;
      }
      notifyListeners();
      return;
    }
    final background =
        state == AppLifecycleState.inactive ||
        state == AppLifecycleState.paused ||
        state == AppLifecycleState.hidden ||
        state == AppLifecycleState.detached;
    if (!background) return;
    _focused = false;
    _backgroundedAt ??= _clock();
    _armSoft();
    notifyListeners();
  }

  /// A message arrived while the soft lock is up. The text is not kept here.
  void noteLockedTraffic() {
    if (!softLocked || hardLocked) return;
    if (lockedNotice == newMessageWhileLocked) return;
    lockedNotice = newMessageWhileLocked;
    notifyListeners();
  }

  Future<void> lockNow() async {
    if (settings.hardLock) {
      await _engageHardLock();
      return;
    }
    if (settings.softLock) {
      softLocked = true;
      notifyListeners();
    }
  }

  /// Null when the lock opened. Biometrics are refused for a hard lock.
  Future<String?> unlock(String passphrase, {bool biometric = false}) async {
    if (!locked) return null;
    if (hardLocked) {
      if (biometric) {
        return 'A hard lock needs the passphrase.';
      }
      if (!lockPassphraseMatches(settings, passphrase)) {
        return 'Wrong passphrase.';
      }
      try {
        await _core().unlock();
      } catch (err, stack) {
        debugLogCoreError(err, stack);
        return coreErrorMayContainAddress(err.toString())
            ? "couldn't unlock"
            : "couldn't unlock";
      }
      hardLocked = false;
      softLocked = false;
      lockedNotice = null;
      _armIdle();
      notifyListeners();
      return null;
    }
    if (biometric) {
      final ok = await _biometric.authenticate();
      if (!ok) return 'Biometrics did not unlock.';
    } else if (!lockPassphraseMatches(settings, passphrase)) {
      return 'Wrong passphrase.';
    }
    softLocked = false;
    lockedNotice = null;
    notifyListeners();
    return null;
  }

  bool passphraseMatches(String passphrase) =>
      lockPassphraseMatches(settings, passphrase);

  Future<void> _engageHardLock() async {
    if (hardLocked) return;
    try {
      await _core().lock();
    } catch (err, stack) {
      debugLogCoreError(err, stack);
      lastError = coreErrorMayContainAddress(err.toString())
          ? "couldn't lock"
          : "couldn't lock";
      notifyListeners();
      return;
    }
    lastError = null;
    hardLocked = true;
    softLocked = false;
    lockedNotice = null;
    _softTimer?.cancel();
    _idleTimer?.cancel();
    notifyListeners();
  }

  void _armSoft() {
    _softTimer?.cancel();
    if (!settings.softLock || _backgroundedAt == null || softLocked) return;
    final elapsed = _clock().difference(_backgroundedAt!);
    final remaining = softAfter - elapsed;
    if (remaining <= Duration.zero) {
      softLocked = true;
      return;
    }
    _softTimer = Timer(remaining, () {
      if (_backgroundedAt != null && settings.softLock && !hardLocked) {
        softLocked = true;
        notifyListeners();
      }
    });
  }

  void _armIdle() {
    _idleTimer?.cancel();
    if (!settings.hardLock || hardLocked) return;
    _idleTimer = Timer(hardAfter, () {
      unawaited(_engageHardLock());
    });
  }

  @override
  void dispose() {
    _softTimer?.cancel();
    _idleTimer?.cancel();
    super.dispose();
  }
}
