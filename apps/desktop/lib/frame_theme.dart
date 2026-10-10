import 'package:flutter/material.dart';

/// Semantic colors keep workspace, settings and text readable in both themes.
class FrameColors {
  const FrameColors(this.light);
  final bool light;
  static FrameColors of(BuildContext context) =>
      FrameColors(Theme.of(context).brightness == Brightness.light);
  Color get background =>
      light ? const Color(0xfff3f6f9) : const Color(0xff101820);
  Color get panel => light ? Colors.white : const Color(0xff17212a);
  Color get border => light ? const Color(0xffd8e2e9) : const Color(0xff293641);
  Color get accent => light ? const Color(0xff00694f) : const Color(0xff87e3c2);
  Color get muted => light ? const Color(0xff526474) : const Color(0xffa3b0bc);
  Color get subtle => light ? const Color(0xff657687) : const Color(0xff8393a2);
  Color get secondary =>
      light ? const Color(0xff304455) : const Color(0xffc3cdd6);
  Color get badge => light ? const Color(0xffe7eef2) : Colors.white10;
}

ThemeData frameTheme(Brightness brightness) {
  final colors = FrameColors(brightness == Brightness.light);
  return ThemeData(
    fontFamily: 'Segoe UI',
    brightness: brightness,
    useMaterial3: true,
    colorScheme: ColorScheme.fromSeed(
      seedColor: colors.accent,
      brightness: brightness,
    ),
    scaffoldBackgroundColor: colors.background,
    inputDecorationTheme: InputDecorationTheme(
      border: OutlineInputBorder(borderRadius: BorderRadius.circular(10)),
      filled: true,
      fillColor: colors.light
          ? const Color(0xfff0f4f7)
          : const Color(0xff202d38),
      contentPadding: const EdgeInsets.symmetric(horizontal: 12, vertical: 16),
    ),
  );
}
