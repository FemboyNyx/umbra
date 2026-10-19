import 'package:flutter/material.dart';

/// Night palette drawn from the crescent mark: black field, silver edge,
/// violet eyes.
abstract final class UmbraColors {
  static const background = Color(0xFF0C0D12);
  static const surface = Color(0xFF161822);
  static const surfaceHigh = Color(0xFF222433);
  static const violet = Color(0xFF8B7CFF);
  static const violetDeep = Color(0xFF5C4ED0);
  static const silver = Color(0xFFE6E2F2);
  static const mist = Color(0xFFB7B3C7);
  static const online = Color(0xFF7DDEA5);
  static const danger = Color(0xFFE07A8A);
}

abstract final class UmbraTheme {
  static ThemeData dark() {
    final scheme =
        ColorScheme.fromSeed(
          seedColor: UmbraColors.violet,
          brightness: Brightness.dark,
        ).copyWith(
          surface: UmbraColors.surface,
          primary: UmbraColors.violet,
          onPrimary: Colors.white,
          secondary: UmbraColors.silver,
          error: UmbraColors.danger,
        );
    return _base(scheme, UmbraColors.background);
  }

  static ThemeData light() {
    final scheme = ColorScheme.fromSeed(
      seedColor: UmbraColors.violetDeep,
      brightness: Brightness.light,
    );
    return _base(scheme, scheme.surface);
  }

  static ThemeData _base(ColorScheme scheme, Color scaffold) {
    return ThemeData(
      useMaterial3: true,
      colorScheme: scheme,
      scaffoldBackgroundColor: scaffold,
      appBarTheme: AppBarTheme(
        backgroundColor: scaffold,
        foregroundColor: scheme.onSurface,
        elevation: 0,
        centerTitle: false,
      ),
      cardTheme: CardThemeData(
        color: scheme.brightness == Brightness.dark
            ? UmbraColors.surfaceHigh
            : scheme.surfaceContainerHighest,
        elevation: 0,
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(16)),
      ),
      inputDecorationTheme: InputDecorationTheme(
        filled: true,
        border: OutlineInputBorder(borderRadius: BorderRadius.circular(14)),
      ),
      snackBarTheme: SnackBarThemeData(
        behavior: SnackBarBehavior.floating,
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
      ),
    );
  }
}
