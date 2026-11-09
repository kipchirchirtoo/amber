import 'package:flutter/material.dart';

/// Colours and theme for the whole app. The look follows ERP/MRP tools used on
/// the shop floor and in the office: a navy header bar, pale grey page, white
/// cards with thin borders, shaded label columns, dense grids and traffic-light
/// status colours (green = ok, amber = check, red = problem).
class Brand {
  static const navy = Color(0xFF1B3A5C);
  static const navyDark = Color(0xFF142B45);
  static const blue = Color(0xFF1F6FEB);
  static const page = Color(0xFFF2F4F8);
  static const card = Colors.white;
  static const border = Color(0xFFD9DEE6);
  static const labelBg = Color(0xFFEEF1F6);
  static const headBg = Color(0xFFE7ECF3);
  static const text = Color(0xFF1E2733);
  static const muted = Color(0xFF5E6B7C);
  static const ok = Color(0xFF2E8B57);
  static const warn = Color(0xFFE08A00);
  static const bad = Color(0xFFC62828);
  static const badTint = Color(0xFFFDECEC);
}

ThemeData buildTheme() {
  final scheme = ColorScheme.fromSeed(
    seedColor: Brand.navy,
    primary: Brand.navy,
    surface: Brand.card,
  ).copyWith(primary: Brand.navy, secondary: Brand.blue, error: Brand.bad);
  const small = TextStyle(fontSize: 13);
  // Inter everywhere, with tabular figures so columns of money and quantities
  // line up digit under digit.
  const tnum = [FontFeature.tabularFigures()];
  TextStyle? t(TextStyle? s) => s?.copyWith(fontFeatures: tnum);
  final base = Typography.blackMountainView
      .apply(
        fontFamily: 'Inter',
        bodyColor: Brand.text,
        displayColor: Brand.text,
      )
      .copyWith(
        bodyMedium: small,
        bodySmall: const TextStyle(fontSize: 12, color: Brand.muted),
      );
  final textTheme = base.copyWith(
    displayLarge: t(base.displayLarge),
    displayMedium: t(base.displayMedium),
    displaySmall: t(base.displaySmall),
    headlineLarge: t(base.headlineLarge),
    headlineMedium: t(base.headlineMedium),
    headlineSmall: t(base.headlineSmall),
    titleLarge: t(base.titleLarge),
    titleMedium: t(base.titleMedium),
    titleSmall: t(base.titleSmall),
    bodyLarge: t(base.bodyLarge),
    bodyMedium: t(base.bodyMedium),
    bodySmall: t(base.bodySmall),
    labelLarge: t(base.labelLarge),
    labelMedium: t(base.labelMedium),
    labelSmall: t(base.labelSmall),
  );
  return ThemeData(
    useMaterial3: true,
    fontFamily: 'Inter',
    colorScheme: scheme,
    scaffoldBackgroundColor: Brand.page,
    visualDensity: VisualDensity.compact,
    textTheme: textTheme,
    appBarTheme: const AppBarTheme(
      backgroundColor: Brand.navy,
      foregroundColor: Colors.white,
      elevation: 0,
      scrolledUnderElevation: 0,
      titleTextStyle: TextStyle(
        fontSize: 17,
        fontWeight: FontWeight.w600,
        color: Colors.white,
      ),
    ),
    cardTheme: CardThemeData(
      color: Brand.card,
      elevation: 0,
      margin: EdgeInsets.zero,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(8),
        side: const BorderSide(color: Brand.border),
      ),
    ),
    dividerTheme: const DividerThemeData(
      color: Brand.border,
      space: 1,
      thickness: 1,
    ),
    inputDecorationTheme: InputDecorationTheme(
      isDense: true,
      filled: true,
      fillColor: Colors.white,
      contentPadding: const EdgeInsets.symmetric(horizontal: 10, vertical: 10),
      border: OutlineInputBorder(
        borderRadius: BorderRadius.circular(6),
        borderSide: const BorderSide(color: Brand.border),
      ),
      enabledBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(6),
        borderSide: const BorderSide(color: Brand.border),
      ),
      focusedBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(6),
        borderSide: const BorderSide(color: Brand.blue, width: 1.5),
      ),
    ),
    filledButtonTheme: FilledButtonThemeData(
      style: FilledButton.styleFrom(
        backgroundColor: Brand.blue,
        foregroundColor: Colors.white,
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(6)),
      ),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        foregroundColor: Brand.text,
        backgroundColor: Colors.white,
        side: const BorderSide(color: Brand.border),
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(6)),
      ),
    ),
    listTileTheme: const ListTileThemeData(dense: true),
  );
}
