import 'dart:async';
import 'dart:ui' as ui;
import 'dart:math' as math;
import 'package:flutter/material.dart';
import 'controller.dart';

/// Independent latest-only camera thumbnail: 5 fps, or 2 fps in Low resource.
/// One decoding operation and one ui.Image; old images are disposed immediately.
class WebcamPreview extends StatefulWidget {
  const WebcamPreview({
    super.key,
    required this.recorder,
    required this.webcam,
    required this.enabled,
    required this.canvas,
    this.lowResource = false,
  });
  final RecorderController recorder;
  final Map<String, dynamic> webcam;
  final bool enabled, lowResource;
  final Rect canvas;
  @override
  State<WebcamPreview> createState() => _WebcamPreviewState();
}

class _WebcamPreviewState extends State<WebcamPreview>
    with WidgetsBindingObserver {
  Timer? timer;
  ui.Image? image;
  String? error;
  bool busy = false, visible = true;
  int generation = 0;
  bool get enabled =>
      widget.enabled && widget.webcam['enabled'] == true && visible;
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    restart();
  }

  void restart() {
    timer?.cancel();
    generation++;
    image?.dispose();
    image = null;
    error = null;
    if (!enabled) {
      widget.recorder.stopWebcamPreview();
      return;
    }
    timer = Timer.periodic(
      Duration(milliseconds: widget.lowResource ? 500 : 200),
      (_) => refresh(),
    );
    refresh();
  }

  @override
  void didUpdateWidget(WebcamPreview oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.enabled != widget.enabled ||
        oldWidget.lowResource != widget.lowResource ||
        oldWidget.webcam['enabled'] != widget.webcam['enabled'] ||
        oldWidget.webcam['device'] != widget.webcam['device']) {
      restart();
    }
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    visible = state == AppLifecycleState.resumed;
    restart();
    if (mounted) setState(() {});
  }

  Future<void> refresh() async {
    if (!mounted || !enabled || busy) return;
    busy = true;
    final token = generation;
    ui.Image? decoded;
    try {
      final frame = await widget.recorder.webcamPreviewFrame(
        widget.webcam,
        widget.lowResource,
      );
      if (frame != null) {
        final result = Completer<ui.Image>();
        ui.decodeImageFromPixels(
          frame.rgba,
          frame.width,
          frame.height,
          ui.PixelFormat.rgba8888,
          result.complete,
        );
        decoded = await result.future;
      }
      if (!mounted || token != generation) return;
      setState(() {
        image?.dispose();
        image = decoded;
        decoded = null;
        error = null;
      });
    } catch (e) {
      if (mounted && token == generation) {
        setState(() {
          image?.dispose();
          image = null;
          error = 'Camera unavailable · check Settings';
        });
      }
    } finally {
      decoded?.dispose();
      busy = false;
    }
  }

  @override
  void dispose() {
    generation++;
    timer?.cancel();
    WidgetsBinding.instance.removeObserver(this);
    image?.dispose();
    widget.recorder.stopWebcamPreview();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (!enabled) return const SizedBox.shrink();
    final ratio = image == null ? 4 / 3 : image!.width / image!.height;
    final width = math.min(
      widget.canvas.width * (widget.webcam['width'] as num? ?? .25),
      widget.canvas.height * ratio,
    );
    final height = width / ratio;
    final left =
        widget.canvas.left +
        (widget.canvas.width - width) * (widget.webcam['x'] as num? ?? 1);
    final top =
        widget.canvas.top +
        (widget.canvas.height - height) * (widget.webcam['y'] as num? ?? 1);
    return IgnorePointer(
      child: Stack(
        children: [
          Positioned(
            left: left,
            top: top,
            width: width,
            height: height,
            child: image != null
                ? RawImage(
                    key: const Key('live-webcam-image'),
                    image: image,
                    fit: BoxFit.fill,
                    filterQuality: FilterQuality.low,
                  )
                : ColoredBox(
                    color: Colors.black87,
                    child: Center(
                      child: Text(
                        error ?? 'Connecting camera…',
                        textAlign: TextAlign.center,
                        style: const TextStyle(
                          color: Colors.white70,
                          fontSize: 11,
                        ),
                      ),
                    ),
                  ),
          ),
        ],
      ),
    );
  }
}
