/// Persistent optional recording features. Heavy processing is opt-in.
class FeatureOptions {
  int countdown = 0, maxSeconds = 0, maxFileMiB = 0;
  int replaySeconds = 0, replayMiB = 32;
  int segmentSeconds = 0;
  String systemDevice = '', microphoneDevice = '';
  double systemGain = 1, microphoneGain = 1;
  bool systemMuted = false, microphoneMuted = false;
  bool mixAudio = true, denoise = false, gate = false, limiter = true;
  bool captureCursor = true;
  bool annotations = false;
  bool cursorHighlight = false, clickEffects = false;
  String performanceMode = 'automatic';
  bool adaptiveQuality = true;
  Map<String, dynamic> get audioControls => {
    'system_gain': systemGain,
    'microphone_gain': microphoneGain,
    'system_muted': systemMuted,
    'microphone_muted': microphoneMuted,
  };
  Map<String, dynamic> toJson() => {
    'countdown': countdown,
    'max_seconds': maxSeconds,
    'max_file_mib': maxFileMiB,
    'system_device': systemDevice,
    'microphone_device': microphoneDevice,
    'audio_controls': audioControls,
    'mix_audio': mixAudio,
    'noise_suppression': denoise,
    'noise_gate': gate,
    'limiter': limiter,
    'capture_cursor': captureCursor,
    'replay_seconds': replaySeconds,
    'replay_mib': replayMiB,
    'annotations_enabled': annotations,
    'cursor_highlight': cursorHighlight,
    'click_effects': clickEffects,
    'segment_seconds': segmentSeconds,
    'performance_mode': performanceMode,
    'adaptive_quality': adaptiveQuality,
  };
  void restore(Map<String, dynamic> data) {
    int bounded(String key, int fallback, int max) =>
        data[key] is num ? (data[key] as num).toInt().clamp(0, max) : fallback;
    countdown = bounded('countdown', 0, 10);
    if (![0, 3, 5, 10].contains(countdown)) countdown = 0;
    maxSeconds = bounded('max_seconds', 0, 86400);
    maxFileMiB = bounded('max_file_mib', 0, 1048576);
    replaySeconds = bounded('replay_seconds', 0, 120);
    if (![0, 15, 30, 60, 120].contains(replaySeconds)) replaySeconds = 0;
    replayMiB = bounded('replay_mib', 32, 128).clamp(4, 128);
    segmentSeconds = bounded('segment_seconds', 0, 3600);
    if (![0, 60, 300, 900].contains(segmentSeconds)) segmentSeconds = 0;
    performanceMode =
        ['automatic', 'eco', 'manual'].contains(data['performance_mode'])
        ? data['performance_mode'] as String
        : 'automatic';
    adaptiveQuality = data['adaptive_quality'] is bool
        ? data['adaptive_quality'] as bool
        : true;
    systemDevice = data['system_device'] is String
        ? data['system_device'] as String
        : '';
    microphoneDevice = data['microphone_device'] is String
        ? data['microphone_device'] as String
        : '';
    mixAudio = data['mix_audio'] as bool? ?? true;
    denoise = data['noise_suppression'] as bool? ?? false;
    gate = data['noise_gate'] as bool? ?? false;
    limiter = data['limiter'] as bool? ?? true;
    captureCursor = data['capture_cursor'] as bool? ?? true;
    annotations = data['annotations_enabled'] as bool? ?? false;
    cursorHighlight = data['cursor_highlight'] as bool? ?? false;
    clickEffects = data['click_effects'] as bool? ?? false;
    final audio = data['audio_controls'];
    if (audio is Map) {
      double gain(String key) =>
          audio[key] is num && (audio[key] as num).toDouble().isFinite
          ? (audio[key] as num).toDouble().clamp(0, 2)
          : 1;
      systemGain = gain('system_gain');
      microphoneGain = gain('microphone_gain');
      systemMuted = audio['system_muted'] as bool? ?? false;
      microphoneMuted = audio['microphone_muted'] as bool? ?? false;
    }
  }
}
