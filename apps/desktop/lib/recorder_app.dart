import 'dart:math' as math;
import 'dart:async';
import 'package:flutter/material.dart';
import 'controller.dart';
import 'workspace_view.dart';
import 'frame_theme.dart';
import 'feature_options.dart';
import 'feature_settings.dart';
import 'media_jobs.dart';
import 'recording_library.dart';

class RecorderApp extends StatefulWidget {
  const RecorderApp({super.key, this.controller});
  final RecorderController? controller;
  @override
  State<RecorderApp> createState() => _RecorderAppState();
}

class _RecorderAppState extends State<RecorderApp> {
  late final RecorderController recorder;
  String mode = 'display', format = 'mkv', encoder = 'auto';
  int fps = 30, maxHeight = 1080, bitrate = 8;
  bool systemAudio = false, microphone = false, previewEnabled = true;
  int? sourceId;
  ThemeMode _themeMode = ThemeMode.dark;
  int _themeRevision = 0;
  final features = FeatureOptions();
  final mediaJobs = MediaJobs();
  List<Map<String, dynamic>> savedPresets = [];
  Timer? _preferenceTimer;
  FrameColors get _colors => FrameColors(_themeMode == ThemeMode.light);
  final region = List.generate(4, (_) => TextEditingController());
  @override
  void initState() {
    super.initState();
    recorder = widget.controller ?? RecorderController();
    if (widget.controller == null) recorder.initialize();
    _loadTheme();
    _loadFeatures();
    recorder.onStartRequested = start;
    recorder.onReplayRequested = recorder.saveReplay;
  }

  Future<void> _loadFeatures() async {
    await recorder.loadPreferences();
    if (!mounted) return;
    try {
      final saved = recorder.preferences['features'];
      if (saved is Map) features.restore(Map<String, dynamic>.from(saved));
      savedPresets = (recorder.preferences['presets'] as List? ?? [])
          .whereType<Map>()
          .where(
            (p) =>
                p['name'] is String &&
                (p['name'] as String).isNotEmpty &&
                (p['name'] as String).length <= 48 &&
                p['height'] is num &&
                p['fps'] is num &&
                p['bitrate'] is num,
          )
          .map((p) => Map<String, dynamic>.from(p))
          .take(50)
          .toList();
      setState(() {});
    } catch (_) {
      /* Invalid optional preferences do not prevent recording. */
    }
  }

  void _saveFeatures() {
    _preferenceTimer?.cancel();
    _preferenceTimer = Timer(const Duration(milliseconds: 300), () {
      recorder.preferences['features'] = features.toJson();
      recorder.preferences['presets'] = savedPresets;
      recorder.savePreferences();
    });
  }

  void _preset(Map<String, dynamic> preset) {
    if (['automatic', 'eco', 'manual'].contains(preset['performance_mode'])) {
      features.performanceMode = preset['performance_mode'] as String;
    }
    maxHeight = (preset['height'] as num? ?? 1080).toInt();
    fps = (preset['fps'] as num? ?? 30).toInt();
    bitrate = (preset['bitrate'] as num? ?? 8).toInt();
    if (![720, 1080, 2160].contains(maxHeight)) maxHeight = 1080;
    if (![10, 24, 30, 60, 120].contains(fps)) fps = 30;
    bitrate = bitrate.clamp(1, 80);
    if (format == 'gif') {
      maxHeight = 720;
      fps = 10;
    }
  }

  Future<void> _savePreset(BuildContext context) async {
    final input = TextEditingController();
    final name = await showDialog<String>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Save preset'),
        content: TextField(
          controller: input,
          autofocus: true,
          maxLength: 48,
          decoration: const InputDecoration(labelText: 'Preset name'),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(context, input.text.trim()),
            child: const Text('Save'),
          ),
        ],
      ),
    );
    input.dispose();
    if (!mounted || name == null || name.isEmpty) return;
    setState(() {
      savedPresets.removeWhere((p) => p['name'] == name);
      savedPresets.add({
        'name': name,
        'height': maxHeight,
        'fps': fps,
        'bitrate': bitrate,
        'performance_mode': features.performanceMode,
      });
      if (savedPresets.length > 50) savedPresets.removeAt(0);
    });
    _saveFeatures();
  }

  Future<void> _schedule(BuildContext context) async {
    final config = _recordingConfig();
    if (config == null) return;
    final now = DateTime.now();
    final date = await showDatePicker(
      context: context,
      initialDate: now,
      firstDate: now,
      lastDate: now.add(const Duration(days: 30)),
    );
    if (date == null || !context.mounted) return;
    final time = await showTimePicker(
      context: context,
      initialTime: TimeOfDay.fromDateTime(now.add(const Duration(minutes: 1))),
    );
    if (time == null) return;
    recorder.schedule(
      config,
      DateTime(date.year, date.month, date.day, time.hour, time.minute),
      features.countdown,
    );
  }

  Future<void> _loadTheme() async {
    final revision = _themeRevision;
    try {
      final theme = await RecorderController.host.invokeMethod<String>(
        'getTheme',
      );
      if (mounted &&
          revision == _themeRevision &&
          (theme == 'light' || theme == 'dark')) {
        setState(
          () =>
              _themeMode = theme == 'light' ? ThemeMode.light : ThemeMode.dark,
        );
      }
    } catch (_) {
      /* Keep the default if a platform preference is unavailable. */
    }
  }

  Future<void> _saveTheme() async {
    try {
      await RecorderController.host.invokeMethod<void>('setTheme', {
        'theme': _themeMode == ThemeMode.light ? 'light' : 'dark',
      });
    } catch (_) {
      if (mounted) {
        recorder.showError(
          'Theme changed, but could not be saved for the next launch.',
        );
      }
    }
  }

  @override
  void dispose() {
    mediaJobs.dispose();
    _preferenceTimer?.cancel();
    recorder.onStartRequested = null;
    recorder.onReplayRequested = null;
    if (widget.controller == null) recorder.dispose();
    for (final field in region) {
      field.dispose();
    }
    super.dispose();
  }

  Map<String, dynamic>? get selected {
    final kind = mode == 'window' ? 'window' : 'display';
    final choices = recorder.sources.where((s) => s['kind'] == kind).toList();
    if (choices.isEmpty) return null;
    return choices.firstWhere(
      (s) => s['id'] == sourceId,
      orElse: () => choices.first,
    );
  }

  void start() {
    final config = _recordingConfig();
    if (config != null) {
      recorder.begin(config, features.countdown);
      _saveFeatures();
    }
  }

  Map<String, dynamic>? _recordingConfig() {
    final source = selected;
    if (source == null || recorder.output == null) {
      recorder.showError(
        'Choose a capture source and a new output file first.',
      );
      return null;
    }
    List<int>? crop;
    if (mode == 'region') {
      crop = region.map((c) => int.tryParse(c.text)).whereType<int>().toList();
      if (crop.length != 4) {
        recorder.showError('Enter all four region values in display pixels.');
        return null;
      }
    }
    final width = crop?[2] ?? source['width'] as int;
    final height = crop?[3] ?? source['height'] as int;
    if (width < 2 || height < 2) {
      recorder.showError('The capture area is too small.');
      return null;
    }
    final ratio = math.min(1.0, maxHeight / height);
    final outWidth = math.max(2, (width * ratio / 2).floor() * 2);
    final outHeight = math.max(2, (height * ratio / 2).floor() * 2);
    final extras = features.toJson()..remove('countdown');
    return {
      ...extras,
      'replay_seconds': format == 'gif' ? 0 : features.replaySeconds,
      'annotations_enabled': format != 'gif' && features.annotations,
      'segment_seconds': format == 'mkv' ? features.segmentSeconds : 0,
      'adaptive_quality': format == 'mkv' && features.adaptiveQuality,
      'cursor_highlight':
          format != 'gif' && mode != 'window' && features.cursorHighlight,
      'click_effects':
          format != 'gif' && mode != 'window' && features.clickEffects,
      'output': recorder.output,
      'container': format,
      'source_id': source['id'],
      'source_kind': source['kind'],
      'width': outWidth,
      'height': outHeight,
      'fps': fps,
      'bitrate': bitrate * 1000000,
      'encoder':
          (features.annotations ||
                  ((features.cursorHighlight || features.clickEffects) &&
                      mode != 'window')) &&
              format != 'gif'
          ? 'libx264'
          : encoder,
      'system_audio': systemAudio && format != 'gif',
      'microphone': microphone && format != 'gif',
      'region': ?crop,
    };
  }

  Map<String, dynamic>? get previewSource {
    final source = selected;
    if (source == null) return null;
    final args = <String, dynamic>{
      'id': source['id'],
      'kind': source['kind'],
      'frame_width': source['width'],
      'frame_height': source['height'],
    };
    if (mode == 'region') {
      final values = region.map((c) => int.tryParse(c.text)).toList();
      if (values.any((v) => v == null)) return null;
      args.addAll({
        'left': values[0],
        'top': values[1],
        'width': values[2],
        'height': values[3],
        'frame_width': values[2],
        'frame_height': values[3],
      });
    }
    return args;
  }

  @override
  Widget build(BuildContext context) => MaterialApp(
    title: 'Frame — Screen Recorder',
    debugShowCheckedModeBanner: false,
    theme: frameTheme(Brightness.light),
    darkTheme: frameTheme(Brightness.dark),
    themeMode: _themeMode,
    home: AnimatedBuilder(
      animation: recorder,
      builder: (context, _) => RecorderWorkspace(
        recorder: recorder,
        sourceControls: _sourceControls(),
        previewSource: previewSource,
        previewEnabled: previewEnabled,
        performanceMode: features.performanceMode,
        onPreviewToggle: () => setState(() => previewEnabled = !previewEnabled),
        onSettings: () => _showSettings(context),
        onLibrary: () => showRecordingLibrary(context, recorder, mediaJobs),
        format: format,
        onStart: start,
        summary:
            '${maxHeight == 2160 ? '4K' : '${maxHeight}p'} · $fps fps · $bitrate Mbps · ${format.toUpperCase()}',
      ),
    ),
  );

  Widget _sourceControls() {
    final active = recorder.active;
    final options = recorder.sources
        .where((s) => s['kind'] == (mode == 'window' ? 'window' : 'display'))
        .toList();
    return _panel('CAPTURE SOURCE', [
      SizedBox(
        width: double.infinity,
        child: SegmentedButton<String>(
          showSelectedIcon: false,
          style: const ButtonStyle(visualDensity: VisualDensity.compact),
          segments: const [
            ButtonSegment(value: 'display', label: Text('Display')),
            ButtonSegment(value: 'window', label: Text('Window')),
            ButtonSegment(value: 'region', label: Text('Region')),
          ],
          selected: {mode},
          onSelectionChanged: active
              ? null
              : (v) => setState(() {
                  mode = v.first;
                  sourceId = null;
                  if (mode == 'region' && region.every((c) => c.text.isEmpty)) {
                    final source = selected;
                    final defaults = [
                      0,
                      0,
                      math.min(640, source?['width'] as int? ?? 640),
                      math.min(360, source?['height'] as int? ?? 360),
                    ];
                    for (var i = 0; i < 4; i++) {
                      region[i].text = '${defaults[i]}';
                    }
                  }
                }),
        ),
      ),
      const SizedBox(height: 16),
      DropdownButtonFormField<int>(
        key: ValueKey('$mode-${selected?['id']}-${options.length}'),
        initialValue: selected?['id'] as int?,
        isExpanded: true,
        decoration: const InputDecoration(labelText: 'Source'),
        items: options
            .map(
              (s) => DropdownMenuItem<int>(
                value: s['id'] as int,
                child: Text(
                  s['name'] as String,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            )
            .toList(),
        onChanged: active ? null : (v) => setState(() => sourceId = v),
      ),
      if (mode == 'region') ...[
        const SizedBox(height: 12),
        OutlinedButton.icon(
          onPressed: active
              ? null
              : () async {
                  try {
                    final values = await RecorderController.host
                        .invokeListMethod<int>('selectRegion', {
                          'id': selected?['id'],
                        });
                    if (values != null && values.length == 4 && mounted) {
                      setState(() {
                        for (var i = 0; i < 4; i++) {
                          region[i].text = '${values[i]}';
                        }
                      });
                    }
                  } catch (e) {
                    recorder.showError(e.toString());
                  }
                },
          icon: const Icon(Icons.crop_free),
          label: const Text('Select area with mouse'),
        ),
        const SizedBox(height: 16),
        Wrap(
          spacing: 12,
          runSpacing: 12,
          children: List.generate(
            4,
            (i) => SizedBox(
              width: 134,
              child: TextField(
                controller: region[i],
                enabled: !active,
                keyboardType: TextInputType.number,
                onChanged: (_) => setState(() {}),
                decoration: InputDecoration(
                  labelText: ['Left', 'Top', 'Width', 'Height'][i],
                  suffixText: 'px',
                ),
              ),
            ),
          ),
        ),
      ],
      const SizedBox(height: 8),
      Row(
        children: [
          Expanded(
            child: Text(
              selected == null
                  ? 'No available source'
                  : '${selected!['width']} × ${selected!['height']}',
              style: TextStyle(color: _colors.subtle, fontSize: 12),
            ),
          ),
          IconButton(
            onPressed: active ? null : recorder.refreshSources,
            tooltip: 'Refresh sources',
            icon: const Icon(Icons.refresh_rounded, size: 18),
          ),
        ],
      ),
    ]);
  }

  Future<void> _showSettings(BuildContext context) => showDialog<void>(
    context: context,
    builder: (context) => StatefulBuilder(
      builder: (context, updateDialog) {
        final active = recorder.active;
        void change(VoidCallback update) {
          setState(update);
          updateDialog(() {});
        }

        return Dialog(
          alignment: Alignment.centerRight,
          insetPadding: const EdgeInsets.all(20),
          backgroundColor: _colors.background,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(22),
          ),
          child: SizedBox(
            width: 510,
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxHeight: 700),
              child: Padding(
                padding: const EdgeInsets.all(24),
                child: Column(
                  children: [
                    Row(
                      children: [
                        Icon(Icons.tune_rounded, color: _colors.accent),
                        const SizedBox(width: 12),
                        const Text(
                          'Recording settings',
                          style: TextStyle(
                            fontSize: 22,
                            fontWeight: FontWeight.w600,
                          ),
                        ),
                        const Spacer(),
                        IconButton(
                          onPressed: () => Navigator.pop(context),
                          tooltip: 'Close settings',
                          icon: const Icon(Icons.close),
                        ),
                      ],
                    ),
                    const SizedBox(height: 4),
                    Align(
                      alignment: Alignment.centerLeft,
                      child: Text(
                        active
                            ? 'Stop recording to change encoding settings.'
                            : 'Make it smooth. Make it yours.',
                        style: TextStyle(color: _colors.muted, fontSize: 13),
                      ),
                    ),
                    const SizedBox(height: 22),
                    Expanded(
                      child: ListView(
                        children: [
                          _panel('APPEARANCE', [
                            const Text('Theme'),
                            const SizedBox(height: 12),
                            SizedBox(
                              width: double.infinity,
                              child: SegmentedButton<ThemeMode>(
                                segments: const [
                                  ButtonSegment(
                                    value: ThemeMode.dark,
                                    icon: Icon(Icons.dark_mode_outlined),
                                    label: Text('Dark'),
                                  ),
                                  ButtonSegment(
                                    value: ThemeMode.light,
                                    icon: Icon(Icons.light_mode_outlined),
                                    label: Text('Light'),
                                  ),
                                ],
                                selected: {_themeMode},
                                onSelectionChanged: (value) {
                                  change(() {
                                    _themeRevision++;
                                    _themeMode = value.first;
                                  });
                                  _saveTheme();
                                },
                              ),
                            ),
                          ]),
                          const SizedBox(height: 16),
                          _quality(active, change),
                          const SizedBox(height: 16),
                          AnimatedBuilder(
                            animation: recorder,
                            builder: (context, _) => FeatureSettings(
                              options: features,
                              recorder: recorder,
                              savedPresets: savedPresets,
                              onChanged: () => change(() {
                                _saveFeatures();
                              }),
                              onPreset: (preset) => change(() {
                                _preset(preset);
                                _saveFeatures();
                              }),
                              onSavePreset: () => _savePreset(context),
                              onSchedule: () => _schedule(context),
                            ),
                          ),
                          const SizedBox(height: 16),
                          _panel('OUTPUT & PREVIEW', [
                            _select<String>(
                              'File format',
                              format,
                              {
                                'mkv': 'MKV · flexible',
                                'mp4': 'MP4 · widely compatible',
                                'avi': 'AVI',
                                'gif': 'GIF · animated image',
                              },
                              active,
                              (v) => change(() {
                                format = v;
                                recorder.output = null;
                                if (v == 'gif') {
                                  fps = 10;
                                  maxHeight = 720;
                                }
                              }),
                            ),
                            const SizedBox(height: 16),
                            SwitchListTile(
                              contentPadding: EdgeInsets.zero,
                              title: const Text('Live preview'),
                              subtitle: const Text(
                                'Lightweight 1 fps preview. Turn off for lower resource use.',
                              ),
                              value: previewEnabled,
                              onChanged: (v) =>
                                  change(() => previewEnabled = v),
                            ),
                          ]),
                        ],
                      ),
                    ),
                    const SizedBox(height: 18),
                    SizedBox(
                      width: double.infinity,
                      child: FilledButton(
                        onPressed: () => Navigator.pop(context),
                        child: const Text('Done'),
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ),
        );
      },
    ),
  );

  Widget _quality(bool active, StateSetter change) => _panel('VIDEO & AUDIO', [
    Wrap(
      spacing: 16,
      runSpacing: 16,
      children: [
        _select<int>(
          'Maximum resolution',
          maxHeight,
          {
            720: '720p · lighter load',
            1080: '1080p · balanced',
            2160: '4K · demanding',
          },
          active,
          (v) => change(() => maxHeight = v),
        ),
        _select<int>(
          'Frame rate',
          fps,
          {
            10: '10 fps · GIF',
            if (format != 'gif') 24: '24 fps',
            if (format != 'gif') 30: '30 fps',
            if (format != 'gif') 60: '60 fps',
            if (format != 'gif') 120: '120 fps',
          },
          active,
          (v) => change(() => fps = v),
        ),
        _select<String>(
          'Encoder',
          encoder,
          {
            'auto': 'Automatic hardware / software',
            'h264_nvenc': 'NVIDIA NVENC',
            'h264_amf': 'AMD AMF',
            'h264_qsv': 'Intel Quick Sync',
            'libx264': 'Software · lower compatibility risk',
          },
          active,
          (v) => change(() => encoder = v),
        ),
      ],
    ),
    const SizedBox(height: 12),
    Row(
      children: [
        Text('Bitrate: $bitrate Mbps'),
        Expanded(
          child: Slider(
            value: bitrate.toDouble(),
            min: 1,
            max: 80,
            divisions: 79,
            onChanged: active ? null : (v) => change(() => bitrate = v.round()),
          ),
        ),
      ],
    ),
    Wrap(
      spacing: 24,
      children: [
        SizedBox(
          width: 280,
          child: CheckboxListTile(
            contentPadding: EdgeInsets.zero,
            title: const Text('Record system audio'),
            value: systemAudio,
            onChanged: active || format == 'gif'
                ? null
                : (v) => change(() => systemAudio = v!),
          ),
        ),
        SizedBox(
          width: 280,
          child: CheckboxListTile(
            contentPadding: EdgeInsets.zero,
            title: const Text('Record microphone'),
            value: microphone,
            onChanged: active || format == 'gif'
                ? null
                : (v) => change(() => microphone = v!),
          ),
        ),
      ],
    ),
    Text(
      format == 'gif'
          ? 'GIF recordings contain no audio.'
          : 'Audio sources are saved as separate tracks.',
      style: TextStyle(color: _colors.muted),
    ),
  ]);
  Widget _panel(String title, List<Widget> children) => Container(
    padding: const EdgeInsets.all(18),
    decoration: BoxDecoration(
      color: _colors.panel,
      borderRadius: BorderRadius.circular(16),
      border: Border.all(color: _colors.border),
    ),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          title,
          style: TextStyle(
            color: _colors.accent,
            fontSize: 13,
            fontWeight: FontWeight.w600,
            letterSpacing: .6,
          ),
        ),
        const SizedBox(height: 20),
        ...children,
      ],
    ),
  );
  Widget _select<T>(
    String label,
    T value,
    Map<T, String> choices,
    bool active,
    ValueChanged<T> changed,
  ) => SizedBox(
    width: double.infinity,
    child: DropdownButtonFormField<T>(
      initialValue: value,
      isExpanded: true,
      decoration: InputDecoration(labelText: label),
      items: choices.entries
          .map(
            (e) => DropdownMenuItem<T>(
              value: e.key,
              child: Text(e.value, overflow: TextOverflow.ellipsis),
            ),
          )
          .toList(),
      onChanged: active
          ? null
          : (v) {
              if (v != null) changed(v);
            },
    ),
  );
}
