import 'dart:async';
import 'dart:typed_data';
import 'package:flutter/material.dart';
import 'controller.dart';

/// One small native thumbnail per second; never queues overlapping requests.
/// These images stay in memory and are independent of recorded media frames.
class CapturePreview extends StatefulWidget {
  const CapturePreview({
    super.key,
    required this.source,
    required this.enabled,
  });
  final Map<String, dynamic>? source;
  final bool enabled;
  @override
  State<CapturePreview> createState() => _CapturePreviewState();
}

class _CapturePreviewState extends State<CapturePreview>
    with WidgetsBindingObserver {
  Timer? _timer;
  Uint8List? _image;
  String? _error;
  bool _busy = false, _visible = true;
  int _generation = 0;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    _timer = Timer.periodic(const Duration(seconds: 1), (_) => _refresh());
    _refresh();
  }

  @override
  void didUpdateWidget(CapturePreview oldWidget) {
    super.didUpdateWidget(oldWidget);
    String identity(Map<String, dynamic>? s) =>
        '${s?['id']}:${s?['kind']}:${s?['frame_width']}:${s?['frame_height']}';
    // A moving region changes coordinates frequently. Read its latest position
    // on the existing one-second timer instead of triggering extra captures.
    if (identity(oldWidget.source) != identity(widget.source) ||
        oldWidget.enabled != widget.enabled) {
      _generation++;
      _discardImage();
      _error = null;
      _refresh();
    }
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    _visible = state == AppLifecycleState.resumed;
    if (_visible) _refresh();
  }

  Future<void> _refresh() async {
    if (_busy ||
        !mounted ||
        !_visible ||
        !widget.enabled ||
        widget.source == null) {
      return;
    }
    _busy = true;
    final generation = _generation;
    try {
      final image = await RecorderController.host.invokeMethod<Uint8List>(
        'capturePreview',
        widget.source,
      );
      if (image != null &&
          image.isNotEmpty &&
          mounted &&
          generation == _generation) {
        setState(() {
          _discardImage();
          _image = image;
          _error = null;
        });
      }
    } catch (_) {
      if (mounted && generation == _generation) {
        setState(() {
          _discardImage();
          _error = 'Preview unavailable. Keep the source window open.';
        });
      }
    } finally {
      _busy = false;
    }
  }

  @override
  void dispose() {
    _generation++;
    _timer?.cancel();
    WidgetsBinding.instance.removeObserver(this);
    _discardImage();
    super.dispose();
  }

  void _discardImage() {
    final previous = _image;
    _image = null;
    // MemoryImage keys contain the bytes. Evict old thumbnails instead of
    // filling Flutter's global image cache with a long-running preview history.
    if (previous != null) MemoryImage(previous).evict();
  }

  @override
  Widget build(BuildContext context) => ClipRRect(
    borderRadius: BorderRadius.circular(14),
    child: ColoredBox(
      color: const Color(0xff080e13),
      child: Stack(
        fit: StackFit.expand,
        children: [
          if (_image != null && widget.enabled)
            Padding(
              padding: const EdgeInsets.all(12),
              child: Image.memory(
                _image!,
                fit: BoxFit.contain,
                gaplessPlayback: true,
                filterQuality: FilterQuality.medium,
                errorBuilder: (_, _, _) =>
                    const Center(child: Text('Preview unavailable')),
              ),
            )
          else
            Center(
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Icon(
                    widget.enabled
                        ? Icons.desktop_windows_rounded
                        : Icons.visibility_off_outlined,
                    size: 46,
                    color: Colors.white24,
                  ),
                  const SizedBox(height: 14),
                  Text(
                    !widget.enabled
                        ? 'Preview is off'
                        : widget.source == null
                        ? 'Choose a capture source'
                        : _error ?? 'Loading your preview…',
                    style: const TextStyle(color: Colors.white54),
                  ),
                  const SizedBox(height: 6),
                  const Text(
                    'Your recording controls are always within reach.',
                    style: TextStyle(color: Colors.white30, fontSize: 12),
                  ),
                ],
              ),
            ),
          Positioned(
            top: 16,
            left: 16,
            child: Container(
              padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 5),
              decoration: BoxDecoration(
                color: Colors.black54,
                borderRadius: BorderRadius.circular(20),
              ),
              child: Text(
                widget.enabled ? 'LIVE PREVIEW · 1 FPS' : 'PREVIEW OFF',
                style: const TextStyle(
                  fontSize: 10,
                  letterSpacing: 1,
                  color: Colors.white70,
                ),
              ),
            ),
          ),
        ],
      ),
    ),
  );
}
