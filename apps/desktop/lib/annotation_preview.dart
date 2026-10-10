import 'dart:async';
import 'dart:math' as math;
import 'package:flutter/material.dart';
import 'capture_preview.dart';
import 'controller.dart';
import 'creator_layers.dart';
import 'webcam_preview.dart';

/// Draw inside the letterboxed source, with bounded normalized geometry sent
/// to Rust. Recorded pixels remain entirely in the native media pipeline.
class AnnotationPreview extends StatefulWidget {
  const AnnotationPreview({
    super.key,
    required this.recorder,
    required this.source,
    required this.enabled,
    this.layersEnabled = false,
    this.webcam = const {},
    this.webcamPreviewEnabled = false,
    this.lowResource = false,
  });
  final RecorderController recorder;
  final Map<String, dynamic>? source;
  final bool enabled;
  final bool layersEnabled;
  final Map<String, dynamic> webcam;
  final bool webcamPreviewEnabled, lowResource;
  @override
  State<AnnotationPreview> createState() => _AnnotationPreviewState();
}

class _AnnotationPreviewState extends State<AnnotationPreview> {
  final List<Map<String, dynamic>> marks = [];
  String tool = 'draw';
  Color color = const Color(0xffffcc33);
  Timer? pending;
  bool gestureActive = false;
  bool get drawing =>
      widget.recorder.status['annotations_enabled'] == true &&
      widget.recorder.state == 'recording';
  @override
  void didUpdateWidget(AnnotationPreview oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!widget.recorder.active && marks.isNotEmpty) marks.clear();
  }

  void send() {
    pending?.cancel();
    pending = Timer(
      const Duration(milliseconds: 50),
      () => widget.recorder.updateAnnotations(marks),
    );
  }

  @override
  void dispose() {
    pending?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => Column(
    children: [
      if (widget.recorder.status['annotations_enabled'] == true &&
          widget.recorder.active)
        Row(
          children: [
            for (final option in [
              ('draw', Icons.draw_outlined),
              ('arrow', Icons.north_east),
              ('box', Icons.crop_square),
              ('spotlight', Icons.highlight),
            ])
              IconButton(
                tooltip: option.$1,
                color: tool == option.$1
                    ? Theme.of(context).colorScheme.primary
                    : null,
                onPressed: drawing
                    ? () => setState(() => tool = option.$1)
                    : null,
                icon: Icon(option.$2, size: 20),
              ),
            for (final swatch in [
              const Color(0xffffcc33),
              const Color(0xffff5555),
              const Color(0xff55eeaa),
            ])
              IconButton(
                tooltip: 'Annotation color',
                onPressed: () => setState(() => color = swatch),
                icon: Icon(
                  Icons.circle,
                  color: swatch,
                  size: color == swatch ? 18 : 12,
                ),
              ),
            const Spacer(),
            IconButton(
              tooltip: 'Undo annotation',
              onPressed: marks.isEmpty
                  ? null
                  : () {
                      setState(() => marks.removeLast());
                      send();
                    },
              icon: const Icon(Icons.undo, size: 20),
            ),
            IconButton(
              tooltip: 'Clear annotations',
              onPressed: marks.isEmpty
                  ? null
                  : () {
                      setState(marks.clear);
                      send();
                    },
              icon: const Icon(Icons.clear_all, size: 20),
            ),
          ],
        ),
      Expanded(
        child: LayoutBuilder(
          builder: (context, limits) {
            final source = widget.source;
            final ratio =
                ((source?['frame_width'] as num? ?? 16) /
                        (source?['frame_height'] as num? ?? 9))
                    .clamp(.01, 100.0);
            final available = Size(
              math.max(1, limits.maxWidth - 24),
              math.max(1, limits.maxHeight - 24),
            );
            final width = math.min(available.width, available.height * ratio);
            final height = width / ratio;
            final rect = Rect.fromLTWH(
              (limits.maxWidth - width) / 2,
              (limits.maxHeight - height) / 2,
              width,
              height,
            );
            List<double> point(Offset p) => [
              ((p.dx - rect.left) / rect.width).clamp(0, 1),
              ((p.dy - rect.top) / rect.height).clamp(0, 1),
            ];
            return Stack(
              fit: StackFit.expand,
              children: [
                CapturePreview(source: source, enabled: widget.enabled),
                WebcamPreview(
                  recorder: widget.recorder,
                  webcam: widget.webcam,
                  enabled: widget.webcamPreviewEnabled,
                  canvas: rect,
                  lowResource: widget.lowResource,
                ),
                if (widget.enabled &&
                    widget.layersEnabled &&
                    widget.recorder.creatorLayers.entries.isNotEmpty &&
                    (!widget.recorder.active ||
                        widget.recorder.status['layers_enabled'] == true))
                  IgnorePointer(
                    child: CustomPaint(
                      painter: CreatorLayerPainter(
                        widget.recorder.creatorLayers,
                        rect,
                      ),
                    ),
                  ),
                if (widget.recorder.active &&
                    widget.recorder.status['shortcut_label'] is String &&
                    widget.enabled)
                  Positioned(
                    left: widget.recorder.status['shortcut_right'] == true
                        ? null
                        : rect.left,
                    right: widget.recorder.status['shortcut_right'] == true
                        ? limits.maxWidth - rect.right
                        : null,
                    bottom: limits.maxHeight - rect.bottom,
                    child: IgnorePointer(
                      child: Container(
                        padding: const EdgeInsets.symmetric(
                          horizontal: 12,
                          vertical: 6,
                        ),
                        color: Colors.black87,
                        child: Text(
                          widget.recorder.status['shortcut_label'] as String,
                          style: const TextStyle(color: Colors.white),
                        ),
                      ),
                    ),
                  ),
                if (drawing)
                  GestureDetector(
                    behavior: HitTestBehavior.translucent,
                    onPanStart: (event) {
                      gestureActive = false;
                      if (!rect.contains(event.localPosition) ||
                          marks.length >= 64) {
                        return;
                      }
                      gestureActive = true;
                      setState(() {
                        if (tool == 'spotlight') {
                          marks.removeWhere((m) => m['kind'] == 'spotlight');
                        }
                        marks.add({
                          'kind': tool,
                          'points': <List<double>>[point(event.localPosition)],
                          'rgb': [
                            (color.r * 255).round(),
                            (color.g * 255).round(),
                            (color.b * 255).round(),
                          ],
                          'width': 3,
                        });
                      });
                      send();
                    },
                    onPanUpdate: (event) {
                      if (!gestureActive || marks.isEmpty) return;
                      setState(() {
                        final points =
                            marks.last['points'] as List<List<double>>;
                        if (tool == 'spotlight') {
                          points[0] = point(event.localPosition);
                        } else if (tool != 'draw') {
                          if (points.length == 1) {
                            points.add(point(event.localPosition));
                          } else {
                            points[1] = point(event.localPosition);
                          }
                        } else {
                          if (points.length >= 256) {
                            final reduced = [
                              for (var i = 0; i < points.length; i += 2)
                                points[i],
                            ];
                            points
                              ..clear()
                              ..addAll(reduced);
                          }
                          points.add(point(event.localPosition));
                        }
                      });
                      send();
                    },
                    onPanEnd: (_) {
                      gestureActive = false;
                      send();
                    },
                    onPanCancel: () {
                      gestureActive = false;
                      send();
                    },
                    child: CustomPaint(
                      painter: _AnnotationPainter(marks, rect),
                    ),
                  ),
              ],
            );
          },
        ),
      ),
    ],
  );
}

class _AnnotationPainter extends CustomPainter {
  _AnnotationPainter(this.marks, this.rect);
  final List<Map<String, dynamic>> marks;
  final Rect rect;
  @override
  void paint(Canvas canvas, Size size) {
    canvas.save();
    canvas.clipRect(rect);
    for (final mark in marks) {
      final rgb = mark['rgb'] as List<int>;
      final paint = Paint()
        ..color = Color.fromARGB(255, rgb[0], rgb[1], rgb[2])
        ..strokeWidth = 3
        ..style = PaintingStyle.stroke
        ..strokeCap = StrokeCap.round;
      final points = (mark['points'] as List<List<double>>)
          .map(
            (p) => Offset(
              rect.left + p[0] * rect.width,
              rect.top + p[1] * rect.height,
            ),
          )
          .toList();
      if (points.isEmpty) continue;
      if (mark['kind'] == 'spotlight') {
        final dim = Path()
          ..fillType = PathFillType.evenOdd
          ..addRect(rect)
          ..addOval(
            Rect.fromCircle(
              center: points.first,
              radius: math.min(rect.width, rect.height) / 7,
            ),
          );
        canvas.drawPath(
          dim,
          Paint()..color = Colors.black.withValues(alpha: .65),
        );
      } else if (mark['kind'] == 'box' && points.length > 1) {
        canvas.drawRect(Rect.fromPoints(points.first, points.last), paint);
      } else {
        final path = Path()..moveTo(points.first.dx, points.first.dy);
        for (final p in points.skip(1)) {
          path.lineTo(p.dx, p.dy);
        }
        canvas.drawPath(path, paint);
        if (mark['kind'] == 'arrow' && points.length > 1) {
          final delta = points.last - points.first;
          final angle = math.atan2(delta.dy, delta.dx);
          final length = math.min(rect.width, rect.height) * .04;
          for (final offset in [-.5, .5]) {
            canvas.drawLine(
              points.last,
              points.last -
                  Offset(
                    math.cos(angle + offset) * length,
                    math.sin(angle + offset) * length,
                  ),
              paint,
            );
          }
        }
      }
    }
    canvas.restore();
  }

  @override
  bool shouldRepaint(_AnnotationPainter oldDelegate) => true;
}
