import 'package:flutter/services.dart';

import '../core/core_errors.dart';

/// OS biometric prompt. Soft lock only. A hard lock never calls this.
abstract class SoftLockBiometric {
  Future<bool> available();

  /// True when the OS accepted the biometric. False if it failed, was
  /// cancelled, or this device has none.
  Future<bool> authenticate();
}

class UnavailableBiometric implements SoftLockBiometric {
  const UnavailableBiometric();

  @override
  Future<bool> available() async => false;

  @override
  Future<bool> authenticate() async => false;
}

class PlatformBiometric implements SoftLockBiometric {
  static const _channel = MethodChannel('io.github.femboynyx.umbra/biometric');

  @override
  Future<bool> available() => _bool('available');

  @override
  Future<bool> authenticate() => _bool('authenticate');

  Future<bool> _bool(String method) async {
    try {
      final value = await _channel.invokeMethod<bool>(method);
      return value ?? false;
    } on MissingPluginException {
      return false;
    } on PlatformException catch (err, stack) {
      debugLogCoreError(err, stack);
      return false;
    }
  }
}

class ScreenSecurityResult {
  const ScreenSecurityResult({required this.native});

  /// True when the OS excludes this window from capture.
  /// Android `FLAG_SECURE`, Windows `WDA_EXCLUDEFROMCAPTURE`.
  /// Linux has no such API and returns false.
  final bool native;
}

abstract class ScreenSecurity {
  Future<ScreenSecurityResult> setEnabled(bool enabled);
}

class PlatformScreenSecurity implements ScreenSecurity {
  static const _channel = MethodChannel('io.github.femboynyx.umbra/screen');

  @override
  Future<ScreenSecurityResult> setEnabled(bool enabled) async {
    try {
      final result = await _channel.invokeMapMethod<String, Object?>(
        'setSecure',
        {'enabled': enabled},
      );
      return ScreenSecurityResult(native: result?['native'] == true);
    } on MissingPluginException {
      return const ScreenSecurityResult(native: false);
    } on PlatformException catch (err, stack) {
      debugLogCoreError(err, stack);
      return const ScreenSecurityResult(native: false);
    }
  }
}

/// Test double. Does not talk to the OS.
class RecordingScreenSecurity implements ScreenSecurity {
  RecordingScreenSecurity({this.native = true});

  bool native;
  bool enabled = false;
  var calls = 0;

  @override
  Future<ScreenSecurityResult> setEnabled(bool enabled) async {
    calls += 1;
    this.enabled = enabled;
    return ScreenSecurityResult(native: native);
  }
}
