import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:isolate';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:recorder_bridge/recorder_bridge.dart';
import 'creator_layers.dart';

class RecorderController extends ChangeNotifier {
  RecorderController() {
    creatorLayers.addListener(notifyListeners);
  }
  final creatorLayers = CreatorLayers();
  NativeRecorder? _native;
  Timer? _timer;
  DateTime _lastSourceRefresh = DateTime.fromMillisecondsSinceEpoch(0);
  Timer? _countdownTimer, _scheduleTimer;
  int countdown = 0;
  DateTime? scheduledAt;
  Map<String, dynamic> preferences = {};
  List<Map<String, dynamic>> library = [], audioDevices = [];
  List<String> hotkeyConflicts = [];
  int? freeBytes;
  VoidCallback? onStartRequested, onReplayRequested;
  String? notice;
  String? _indexedOutput;
  String? _indexedReplay;
  void notifyLibraryChanged() => notifyListeners();
  Map<String, dynamic> status = {'state': 'idle'};
  List<Map<String, dynamic>> sources = [];
  Map<String, dynamic> machine = {};
  List<Map<String, dynamic>> cameras = [];
  bool camerasLoading = false, _disposed = false;
  String? cameraError;
  String? error;
  String? output;
  Future<WebcamPreviewFrame?> webcamPreviewFrame(
    Map<String, dynamic> webcam,
    bool lowResource,
  ) async {
    _native?.requestWebcamPreview(webcam, lowResource);
    return _native?.webcamPreviewFrame();
  }

  void stopWebcamPreview() {
    try {
      _native?.requestWebcamPreview({'enabled': false}, false);
    } catch (_) {
      /* Optional preview cleanup. */
    }
  }

  bool setWebcamVisible(bool enabled) {
    try {
      _native?.command({'action': 'webcam_visible', 'enabled': enabled});
      status['webcam_visible'] = enabled;
      notifyListeners();
      return true;
    } catch (e) {
      showError(e.toString());
      return false;
    }
  }

  static const host = MethodChannel('org.screenrecorder/host');

  String get state =>
      countdown > 0 ? 'countdown' : status['state'] as String? ?? 'idle';
  bool get active => [
    'countdown',
    'preparing',
    'recording',
    'paused',
    'stopping',
  ].contains(state);
  void showError(String message) {
    error = message;
    notifyListeners();
  }

  void initialize() {
    host.setMethodCallHandler((call) async {
      if (call.method == 'hostAction') {
        switch (call.arguments) {
          case 'toggle':
            if (active) {
              action('stop');
            } else {
              onStartRequested?.call();
            }
          case 'pause':
            if (state == 'paused') {
              action('resume');
            } else if (state == 'recording') {
              action('pause');
            }
          case 'replay':
            onReplayRequested?.call();
          case 'restoreControls':
            await enableControls();
        }
      }
    });
    enableControls();
    refreshAudioDevices();
    try {
      _native = NativeRecorder.open();
      refreshSources();
      _timer = Timer.periodic(const Duration(milliseconds: 200), (_) {
        try {
          status = _native!.status();
          if (!active &&
              DateTime.now().difference(_lastSourceRefresh).inSeconds >= 3) {
            refreshSources(automatic: true);
          }
          _indexFinishedRecording();
          final replay = status['replay_output'] as String?;
          if (replay != null && replay != _indexedReplay) {
            _indexedReplay = replay;
            library.insert(0, {
              'path': replay,
              'date': DateTime.now().toIso8601String(),
              'seconds': status['replay_seconds'] ?? 0,
            });
            if (library.length > 1000) library.removeLast();
            savePreferences();
            notice = 'Replay saved';
          }
          if (status['replay_error'] is String) {
            error = status['replay_error'] as String;
          }
          if (state == 'failed') error = status['error'] as String?;
          notifyListeners();
        } catch (e) {
          error = e.toString();
          _timer?.cancel();
          notifyListeners();
        }
      });
    } catch (e) {
      error =
          'Recorder engine could not load. Build the native bundle first.\n$e';
      notifyListeners();
    }
  }

  Future<void> enableControls() async {
    try {
      final value = await host.invokeListMethod<String>('enableControls');
      hotkeyConflicts = value ?? [];
      notifyListeners();
    } catch (_) {
      /* Controls are optional on unsupported platforms. */
    }
  }

  Future<void> refreshAudioDevices() async {
    try {
      final value = await host.invokeListMethod<dynamic>('audioDevices') ?? [];
      audioDevices = value
          .whereType<Map>()
          .map((v) => Map<String, dynamic>.from(v))
          .toList();
      notifyListeners();
    } catch (_) {
      /* Recording will still attempt the default native devices. */
    }
  }

  /// Device enumeration stays outside Flutter's rendering isolate.
  Future<void> refreshCameras() async {
    if (camerasLoading || active) return;
    camerasLoading = true;
    cameraError = null;
    notifyListeners();
    try {
      final value = await Isolate.run(() => NativeRecorder.open().cameras());
      if (_disposed) return;
      if (value['ok'] != true) {
        throw StateError(
          value['error'] as String? ?? 'Camera discovery failed',
        );
      }
      cameras = (value['devices'] as List)
          .whereType<Map>()
          .map((v) => Map<String, dynamic>.from(v))
          .toList();
    } catch (e) {
      if (!_disposed) cameraError = e.toString();
    } finally {
      if (!_disposed) {
        camerasLoading = false;
        notifyListeners();
      }
    }
  }

  Future<void> loadPreferences() async {
    try {
      final value = await host.invokeMethod<String>('getPreferences');
      final decoded = jsonDecode(value ?? '{}');
      if (decoded is Map) {
        preferences = Map<String, dynamic>.from(decoded);
        library = (preferences['library'] as List? ?? [])
            .whereType<Map>()
            .where((v) => v['path'] is String && v['seconds'] is num)
            .map((v) => Map<String, dynamic>.from(v))
            .take(1000)
            .toList();
      }
    } catch (_) {
      preferences = {};
    }
  }

  Future<void> savePreferences() async {
    preferences['library'] = library;
    try {
      await host.invokeMethod<void>('setPreferences', {
        'json': jsonEncode(preferences),
      });
    } catch (_) {
      showError('Could not save recording preferences.');
    }
  }

  void _indexFinishedRecording() {
    if (state != 'stopped' ||
        status['output'] == null ||
        _indexedOutput == status['output']) {
      return;
    }
    final path = status['output'] as String;
    _indexedOutput = path;
    final outputs = (status['outputs'] as List? ?? [path]).whereType<String>();
    bool indexed = false;
    for (final path in outputs) {
      if (!File(path).existsSync() || library.any((r) => r['path'] == path)) {
        continue;
      }
      indexed = true;
      library.insert(0, {
        'path': path,
        'date': DateTime.now().toIso8601String(),
        'seconds':
            (status['segment_durations'] as Map?)?[path] ??
            status['seconds'] ??
            0,
        'frames': status['frames'] ?? 0,
      });
      if (library.length > 1000) library.removeLast();
    }
    if (indexed) {
      savePreferences();
      notice = 'Recording saved';
    }
  }

  void begin(Map<String, dynamic> config, int delay) {
    if (active) return;
    error = null;
    if (delay <= 0) {
      start(config);
      return;
    }
    countdown = delay;
    notifyListeners();
    _countdownTimer = Timer.periodic(const Duration(seconds: 1), (timer) {
      countdown--;
      if (countdown <= 0) {
        timer.cancel();
        _countdownTimer = null;
        start(config);
      }
      notifyListeners();
    });
  }

  void schedule(Map<String, dynamic> config, DateTime when, int delay) {
    if (active || !when.isAfter(DateTime.now())) {
      showError('Choose a future time while recording is stopped.');
      return;
    }
    _scheduleTimer?.cancel();
    scheduledAt = when;
    notifyListeners();
    _scheduleTimer = Timer.periodic(const Duration(seconds: 1), (timer) {
      if (!DateTime.now().isBefore(when)) {
        timer.cancel();
        scheduledAt = null;
        if (active) {
          showError(
            'Scheduled recording skipped because another recording is active.',
          );
        } else {
          begin(config, delay);
        }
        notifyListeners();
      }
    });
  }

  void cancelSchedule() {
    _scheduleTimer?.cancel();
    scheduledAt = null;
    notifyListeners();
  }

  void updateAudio(Map<String, dynamic> controls) {
    if (['recording', 'paused', 'preparing'].contains(state)) {
      _send({'action': 'audio', 'controls': controls});
    }
  }

  void updateAnnotations(List<Map<String, dynamic>> items) {
    if (status['annotations_enabled'] == true &&
        ['recording', 'paused'].contains(state)) {
      _send({'action': 'annotations', 'items': items});
    }
  }

  Future<void> checkDiskSpace() async {
    if (output == null) return;
    try {
      freeBytes = await host.invokeMethod<int>('diskSpace', {'path': output});
      notifyListeners();
    } catch (_) {}
  }

  Future<void> screenshot(Map<String, dynamic> source) async {
    try {
      final path = await host.invokeMethod<String>('chooseOutput', {
        'format': 'png',
      });
      if (path == null) return;
      await host.invokeMethod<void>('saveScreenshot', {
        ...source,
        'path': path,
        'max_width': 7680,
        'max_height': 4320,
      });
      notice = 'Screenshot saved';
      notifyListeners();
    } catch (e) {
      showError(e.toString());
    }
  }

  Future<void> openRecording(String path, {bool reveal = false}) async {
    try {
      await host.invokeMethod<void>(reveal ? 'revealOutput' : 'openRecording', {
        'path': path,
      });
    } catch (e) {
      showError(e.toString());
    }
  }

  Future<void> saveReplay() async {
    if (status['replay_enabled'] != true ||
        !['recording', 'paused'].contains(state)) {
      showError('Enable replay in Settings before recording.');
      return;
    }
    try {
      final path = await host.invokeMethod<String>('chooseOutput', {
        'format': 'mkv',
      });
      if (path != null) _send({'action': 'save_replay', 'path': path});
    } catch (e) {
      showError(e.toString());
    }
  }

  void refreshSources({bool automatic = false}) {
    _lastSourceRefresh = DateTime.now();
    try {
      final result = _native!.sources();
      machine = Map<String, dynamic>.from(result['performance'] as Map? ?? {});
      sources = (result['sources'] as List)
          .cast<Map>()
          .map((s) => Map<String, dynamic>.from(s))
          .toList();
      if (!automatic) error = null;
    } catch (e) {
      error = e.toString();
    }
    notifyListeners();
  }

  Future<void> chooseOutput(String format) async {
    try {
      final selected = await host.invokeMethod<String>('chooseOutput', {
        'format': format,
      });
      if (selected != null) {
        output = selected;
        error = null;
        checkDiskSpace();
      }
    } catch (e) {
      error = e.toString();
    }
    notifyListeners();
  }

  void start(Map<String, dynamic> config) {
    if (active) return;
    if (_native == null) {
      error = 'The native recording engine is unavailable.';
      notifyListeners();
      return;
    }
    try {
      _native!.command({'action': 'clear_layers'});
      if (config['layers_enabled'] == true) {
        if (!creatorLayers.ready || creatorLayers.entries.isEmpty) {
          throw StateError(
            'Wait for overlays to load or remove unavailable images',
          );
        }
        for (var i = 0; i < creatorLayers.entries.length; i++) {
          final entry = creatorLayers.entries[i];
          final image = entry.image!;
          _native!.uploadLayer({
            'id': i,
            'pixel_width': image.width,
            'pixel_height': image.height,
            'x': entry.spec.x,
            'y': entry.spec.y,
            'width': entry.spec.width,
            'opacity': entry.spec.opacity,
          }, entry.rgba!);
        }
      }
      _native!.command({'action': 'start', 'config': config});
      error = null;
    } catch (e) {
      error = e.toString();
    }
    status = _native!.status();
    notifyListeners();
  }

  void action(String action) {
    if (action == 'stop' && countdown > 0) {
      _countdownTimer?.cancel();
      countdown = 0;
      notifyListeners();
      return;
    }
    _send({'action': action});
  }

  /// Finalizes the current MKV part before reducing recording quality.
  void lowerQuality() => action('reduce_quality');

  void _send(Map<String, dynamic> message) {
    try {
      _native!.command(message);
      error = null;
    } catch (e) {
      error = e.toString();
    }
    notifyListeners();
  }

  Future<void> revealOutput() async {
    final path = status['output'] as String? ?? output;
    if (path != null) {
      try {
        await host.invokeMethod<void>('revealOutput', {'path': path});
      } catch (e) {
        error = e.toString();
        notifyListeners();
      }
    }
  }

  @override
  void dispose() {
    _disposed = true;
    stopWebcamPreview();
    _timer?.cancel();
    _countdownTimer?.cancel();
    _scheduleTimer?.cancel();
    host.setMethodCallHandler(null);
    creatorLayers.removeListener(notifyListeners);
    creatorLayers.dispose();
    super.dispose();
  }
}
