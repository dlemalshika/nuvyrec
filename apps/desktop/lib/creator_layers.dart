import 'dart:io';
import 'dart:math' as math;
import 'dart:typed_data';
import 'dart:ui' as ui;
import 'package:flutter/material.dart';

/// Small, immutable layout description. No image bytes are persisted in JSON.
class OverlaySpec {
  const OverlaySpec({
    required this.kind,
    this.content = '',
    this.x = 0,
    this.y = 1,
    this.width = .45,
    this.opacity = 1,
    this.fontSize = 40,
    this.color = 0xffffffff,
    this.plate = true,
  });
  final String kind, content;
  final double x, y, width, opacity, fontSize;
  final int color;
  final bool plate;
  String get rasterKey => '$kind:$content:$fontSize:$color:$plate';
  Map<String, dynamic> toJson() => {
    'kind': kind,
    'content': content,
    'x': x,
    'y': y,
    'width': width,
    'opacity': opacity,
    'font_size': fontSize,
    'color': color,
    'plate': plate,
  };
  factory OverlaySpec.fromJson(Map data) {
    final kind = data['kind'];
    final content = data['content'];
    if (!['text', 'image'].contains(kind) ||
        content is! String ||
        content.contains('\u0000') ||
        (kind == 'text' &&
            (content.trim().isEmpty || content.runes.length > 256)) ||
        (kind == 'image' && (content.isEmpty || content.length > 32768))) {
      throw const FormatException('Invalid overlay description');
    }
    double number(String key, double fallback, double low, double high) {
      final value = data[key];
      return value is num && value.toDouble().isFinite
          ? value.toDouble().clamp(low, high)
          : fallback;
    }

    return OverlaySpec(
      kind: kind as String,
      content: content,
      x: number('x', 0, 0, 1),
      y: number('y', 1, 0, 1),
      width: number('width', .45, .02, 1),
      opacity: number('opacity', 1, 0, 1),
      fontSize: number('font_size', 40, 16, 64),
      color: data['color'] is int
          ? (data['color'] as int).clamp(0, 0xffffffff)
          : 0xffffffff,
      plate: data['plate'] != false,
    );
  }
  OverlaySpec copyWith({
    String? content,
    double? x,
    double? y,
    double? width,
    double? opacity,
    double? fontSize,
    int? color,
    bool? plate,
  }) => OverlaySpec(
    kind: kind,
    content: content ?? this.content,
    x: x ?? this.x,
    y: y ?? this.y,
    width: width ?? this.width,
    opacity: opacity ?? this.opacity,
    fontSize: fontSize ?? this.fontSize,
    color: color ?? this.color,
    plate: plate ?? this.plate,
  );
}

class OverlayEntry {
  OverlayEntry(this.spec);
  OverlaySpec spec;
  ui.Image? image;
  Uint8List? rgba;
  String? error;
  bool get ready => image != null && rgba != null && error == null;
  void dispose() {
    image?.dispose();
    image = null;
    rgba = null;
  }
}

/// Render on edits, never on recording frames. Four 768x256 images fit within
/// the native 8 MiB RGBA/fallback cache. A generation guard discards stale loads.
class CreatorLayers extends ChangeNotifier {
  final List<OverlayEntry> entries = [];
  int _generation = 0;
  bool _disposed = false;
  bool loading = false;
  bool get ready => !loading && entries.every((e) => e.ready);
  List<Map<String, dynamic>> toJson() =>
      entries.map((e) => e.spec.toJson()).toList();

  /// Transfer fully prepared assets after the native scene transaction succeeds.
  /// The previous usable UI layout survives missing files and rejected takes.
  void adopt(CreatorLayers candidate) {
    if (!candidate.ready) throw StateError('Scene assets are not ready');
    _generation++;
    for (final entry in entries) {
      entry.dispose();
    }
    entries
      ..clear()
      ..addAll(candidate.entries);
    candidate.entries.clear();
    loading = false;
    notifyListeners();
  }

  Future<void> restore(List<dynamic> data) async {
    final specs = data
        .take(4)
        .whereType<Map>()
        .map(OverlaySpec.fromJson)
        .toList();
    final generation = ++_generation;
    for (final entry in entries) {
      entry.dispose();
    }
    entries
      ..clear()
      ..addAll(specs.map(OverlayEntry.new));
    loading = true;
    notifyListeners();
    for (final entry in List<OverlayEntry>.from(entries)) {
      ui.Image? pendingImage;
      try {
        final image = await renderOverlay(entry.spec);
        pendingImage = image;
        final bytes = await image.toByteData(
          format: ui.ImageByteFormat.rawStraightRgba,
        );
        if (_disposed || generation != _generation) {
          return;
        }
        if (bytes == null) {
          throw StateError('Overlay pixels unavailable');
        }
        entry.image = image;
        pendingImage = null;
        entry.rgba = bytes.buffer.asUint8List(
          bytes.offsetInBytes,
          bytes.lengthInBytes,
        );
      } catch (e) {
        if (!_disposed && generation == _generation) entry.error = e.toString();
      } finally {
        pendingImage?.dispose();
      }
    }
    if (!_disposed && generation == _generation) {
      loading = false;
      notifyListeners();
    }
  }

  Future<void> add(OverlaySpec spec) async {
    if (loading || entries.length >= 4) {
      throw StateError('Up to four overlay layers');
    }
    await restore([...toJson(), spec.toJson()]);
  }

  Future<void> update(int index, OverlaySpec spec) async {
    if (loading || index < 0 || index >= entries.length) return;
    final entry = entries[index];
    if (entry.spec.rasterKey == spec.rasterKey) {
      entry.spec = spec;
      notifyListeners();
      return;
    }
    final descriptions = toJson();
    descriptions[index] = spec.toJson();
    await restore(descriptions);
  }

  void remove(int index) {
    if (loading) return;
    entries.removeAt(index).dispose();
    notifyListeners();
  }

  void move(int from, int to) {
    if (loading) return;
    final entry = entries.removeAt(from);
    entries.insert(to, entry);
    notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    _generation++;
    for (final entry in entries) {
      entry.dispose();
    }
    super.dispose();
  }
}

Future<ui.Image> renderOverlay(OverlaySpec spec) async {
  if (spec.kind == 'image') {
    final file = File(spec.content);
    if (await file.length() > 8 * 1024 * 1024) {
      throw StateError('Logo file must be at most 8 MiB');
    }
    final bytes = await file.readAsBytes();
    if (bytes.length > 8 * 1024 * 1024) {
      throw StateError('Logo file must be at most 8 MiB');
    }
    final buffer = await ui.ImmutableBuffer.fromUint8List(bytes);
    ui.ImageDescriptor? descriptor;
    ui.Codec? codec;
    try {
      descriptor = await ui.ImageDescriptor.encoded(buffer);
      if (descriptor.width < 1 ||
          descriptor.height < 1 ||
          descriptor.width > 8192 ||
          descriptor.height > 8192 ||
          descriptor.width * descriptor.height > 32 * 1024 * 1024) {
        throw StateError('Logo dimensions exceed the supported budget');
      }
      final scale = math.min(
        1.0,
        math.min(768 / descriptor.width, 256 / descriptor.height),
      );
      codec = await descriptor.instantiateCodec(
        targetWidth: math.max(1, (descriptor.width * scale).round()),
        targetHeight: math.max(1, (descriptor.height * scale).round()),
      );
      return (await codec.getNextFrame()).image;
    } finally {
      codec?.dispose();
      descriptor?.dispose();
      buffer.dispose();
    }
  }
  final painter = TextPainter(
    text: TextSpan(
      text: spec.content,
      style: TextStyle(
        color: Color(spec.color),
        fontSize: spec.fontSize,
        fontFamily: 'Segoe UI',
        fontFamilyFallback: const ['Nirmala UI', 'Arial'],
        height: 1.2,
      ),
    ),
    textDirection: TextDirection.ltr,
    maxLines: 3,
    ellipsis: '…',
  )..layout(minWidth: 744, maxWidth: 744);
  final height = (painter.height + 24).ceil().clamp(32, 256);
  final recording = ui.PictureRecorder();
  final canvas = Canvas(recording);
  if (spec.plate) {
    canvas.drawRRect(
      RRect.fromRectAndRadius(
        Rect.fromLTWH(0, 0, 768, height.toDouble()),
        const Radius.circular(12),
      ),
      Paint()..color = const Color(0xcc0c1218),
    );
  }
  painter.paint(canvas, const Offset(12, 12));
  painter.dispose();
  final picture = recording.endRecording();
  try {
    return await picture.toImage(768, height);
  } finally {
    picture.dispose();
  }
}

/// Shared geometry for preview: anchors match the native compositor exactly.
Rect layerRect(OverlayEntry entry, Rect canvas) {
  final image = entry.image;
  if (image == null) return Rect.zero;
  final scale = math.min(
    canvas.width * entry.spec.width / image.width,
    canvas.height / image.height,
  );
  final width = image.width * scale, height = image.height * scale;
  return Rect.fromLTWH(
    canvas.left + (canvas.width - width) * entry.spec.x,
    canvas.top + (canvas.height - height) * entry.spec.y,
    width,
    height,
  );
}

class CreatorLayerPainter extends CustomPainter {
  CreatorLayerPainter(this.layers, this.rect);
  final CreatorLayers layers;
  final Rect rect;
  @override
  void paint(Canvas canvas, Size size) {
    canvas.save();
    canvas.clipRect(rect);
    for (final entry in layers.entries) {
      final image = entry.image;
      if (image == null) continue;
      canvas.drawImageRect(
        image,
        Rect.fromLTWH(0, 0, image.width.toDouble(), image.height.toDouble()),
        layerRect(entry, rect),
        Paint()
          ..color = Colors.white.withValues(alpha: entry.spec.opacity)
          ..filterQuality = FilterQuality.low,
      );
    }
    canvas.restore();
  }

  @override
  bool shouldRepaint(CreatorLayerPainter oldDelegate) => true;
}
