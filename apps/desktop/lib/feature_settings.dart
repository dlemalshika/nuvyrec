import 'package:flutter/material.dart';
import 'controller.dart';
import 'feature_options.dart';

class FeatureSettings extends StatelessWidget {
  const FeatureSettings({
    super.key,
    required this.options,
    required this.recorder,
    required this.onChanged,
    required this.onPreset,
    required this.onSavePreset,
    required this.savedPresets,
    required this.onSchedule,
  });
  final FeatureOptions options;
  final RecorderController recorder;
  final VoidCallback onChanged, onSavePreset, onSchedule;
  final void Function(Map<String, dynamic>) onPreset;
  final List<Map<String, dynamic>> savedPresets;
  void change(VoidCallback action) {
    action();
    onChanged();
  }

  @override
  Widget build(BuildContext context) {
    final active = recorder.active;
    return Column(
      children: [
        ExpansionTile(
          title: const Text('Webcam overlay & green screen'),
          subtitle: const Text(
            'Camera, placement, mirror and background removal',
          ),
          childrenPadding: const EdgeInsets.all(16),
          children: [
            const Text(
              'Live webcam appears in the main preview and saved video. Uses software H.264 encoding. Camera frames are limited to 15 updates/second; preview uses 5 fps, or 2 fps in Low resource. Enable before recording, then show/hide it from the main screen.',
            ),
            TextButton.icon(
              onPressed: active || recorder.camerasLoading
                  ? null
                  : recorder.refreshCameras,
              icon: const Icon(Icons.refresh),
              label: Text(
                recorder.camerasLoading ? 'Finding cameras…' : 'Find cameras',
              ),
            ),
            if (recorder.cameraError != null) Text(recorder.cameraError!),
            if (!recorder.camerasLoading && recorder.cameras.isEmpty)
              const Text(
                'Find cameras to choose an available device. Check Windows camera privacy permissions if capture fails.',
              ),
            DropdownButtonFormField<String>(
              key: ValueKey(
                'camera-${recorder.cameras.length}-${options.webcamDevice}',
              ),
              initialValue: options.webcamDevice,
              isExpanded: true,
              decoration: const InputDecoration(labelText: 'Camera'),
              items: [
                const DropdownMenuItem(
                  value: '',
                  child: Text('Choose a camera'),
                ),
                if (options.webcamDevice.isNotEmpty &&
                    !recorder.cameras.any(
                      (c) => c['id'] == options.webcamDevice,
                    ))
                  DropdownMenuItem(
                    value: options.webcamDevice,
                    child: const Text('Saved camera · refresh availability'),
                  ),
                for (final camera in recorder.cameras)
                  DropdownMenuItem(
                    value: camera['id'] as String,
                    child: Text(
                      camera['name'] as String,
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
              ],
              onChanged: active
                  ? null
                  : (v) => change(() => options.webcamDevice = v!),
            ),
            SwitchListTile(
              contentPadding: EdgeInsets.zero,
              title: const Text('Record webcam overlay'),
              value: options.webcamEnabled,
              onChanged: active || options.webcamDevice.isEmpty
                  ? null
                  : (v) => change(() => options.webcamEnabled = v),
            ),
            Wrap(
              spacing: 8,
              children: [
                for (final corner in [
                  ('Top left', 0.0, 0.0),
                  ('Top right', 1.0, 0.0),
                  ('Bottom left', 0.0, 1.0),
                  ('Bottom right', 1.0, 1.0),
                ])
                  ChoiceChip(
                    label: Text(corner.$1),
                    selected:
                        options.webcamX == corner.$2 &&
                        options.webcamY == corner.$3,
                    onSelected: active
                        ? null
                        : (_) => change(() {
                            options.webcamX = corner.$2;
                            options.webcamY = corner.$3;
                          }),
                  ),
              ],
            ),
            Text('Overlay width: ${(options.webcamWidth * 100).round()}%'),
            Slider(
              value: options.webcamWidth,
              min: .1,
              max: .5,
              divisions: 8,
              onChanged: active
                  ? null
                  : (v) => change(() => options.webcamWidth = v),
            ),
            SwitchListTile(
              contentPadding: EdgeInsets.zero,
              title: const Text('Mirror camera'),
              value: options.webcamMirror,
              onChanged: active
                  ? null
                  : (v) => change(() => options.webcamMirror = v),
            ),
            SwitchListTile(
              contentPadding: EdgeInsets.zero,
              title: const Text('Remove green background'),
              value: options.webcamKey,
              onChanged: active
                  ? null
                  : (v) => change(() => options.webcamKey = v),
            ),
            if (options.webcamKey) ...[
              const Text('Green similarity'),
              Slider(
                value: options.webcamSimilarity,
                min: .05,
                max: .6,
                onChanged: active
                    ? null
                    : (v) => change(() => options.webcamSimilarity = v),
              ),
              const Text('Edge softness'),
              Slider(
                value: options.webcamSoftness,
                min: .01,
                max: .3,
                onChanged: active
                    ? null
                    : (v) => change(() => options.webcamSoftness = v),
              ),
            ],
          ],
        ),
        ExpansionTile(
          title: const Text('Performance & PC compatibility'),
          subtitle: const Text(
            'Automatic budgets, low-resource mode and adaptive quality',
          ),
          childrenPadding: const EdgeInsets.all(16),
          children: [
            DropdownButtonFormField<String>(
              initialValue: options.performanceMode,
              decoration: const InputDecoration(labelText: 'Performance mode'),
              items: const [
                DropdownMenuItem(
                  value: 'automatic',
                  child: Text('Automatic · conservative PC budget'),
                ),
                DropdownMenuItem(
                  value: 'eco',
                  child: Text('Low resource · minimal recording cost'),
                ),
                DropdownMenuItem(
                  value: 'manual',
                  child: Text('Custom / maximum · use selected quality'),
                ),
              ],
              onChanged: active
                  ? null
                  : (v) => change(() => options.performanceMode = v!),
            ),
            const SizedBox(height: 12),
            if (recorder.machine.isNotEmpty)
              Text(
                '${recorder.machine['logical_cores']} logical CPUs · ${((recorder.machine['total_memory_mib'] as num? ?? 0) / 1024).toStringAsFixed(1)} GiB RAM\nSuggested budget: ${recorder.machine['recommended_height']}p / ${recorder.machine['recommended_fps']} fps',
              ),
            SwitchListTile(
              contentPadding: EdgeInsets.zero,
              title: const Text('Reduce quality under sustained overload'),
              subtitle: const Text(
                'MKV only. Saves a new part before lowering resolution/FPS/bitrate. No automatic quality increase.',
              ),
              value: options.adaptiveQuality,
              onChanged: active
                  ? null
                  : (v) => change(() => options.adaptiveQuality = v),
            ),
            const Text(
              'Automatic chooses up to 720p/24fps on constrained PCs and 1080p/30fps otherwise. Custom keeps your selected quality, including 4K/120fps; actual capability depends on the GPU/driver. Low-resource mode suspends preview while recording. Optional effects add CPU cost.',
            ),
          ],
        ),
        ExpansionTile(
          title: const Text('Presets & recording controls'),
          subtitle: const Text('Countdown, capture cursor and hotkeys'),
          childrenPadding: const EdgeInsets.all(16),
          children: [
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                for (final preset in [
                  {
                    'name': 'Low-end PC',
                    'performance_mode': 'eco',
                    'height': 720,
                    'fps': 24,
                    'bitrate': 3,
                  },
                  {
                    'name': 'Tutorial',
                    'height': 1080,
                    'fps': 30,
                    'bitrate': 8,
                    'performance_mode': 'automatic',
                  },
                  {
                    'name': 'High quality',
                    'performance_mode': 'manual',
                    'height': 2160,
                    'fps': 60,
                    'bitrate': 20,
                  },
                  ...savedPresets,
                ])
                  ActionChip(
                    label: Text(preset['name'] as String),
                    onPressed: active ? null : () => onPreset(preset),
                  ),
              ],
            ),
            Align(
              alignment: Alignment.centerLeft,
              child: TextButton.icon(
                onPressed: active ? null : onSavePreset,
                icon: const Icon(Icons.bookmark_add_outlined),
                label: const Text('Save current preset'),
              ),
            ),
            DropdownButtonFormField<int>(
              initialValue: options.countdown,
              decoration: const InputDecoration(labelText: 'Start countdown'),
              items: [
                for (final seconds in [0, 3, 5, 10])
                  DropdownMenuItem(
                    value: seconds,
                    child: Text(seconds == 0 ? 'Off' : '$seconds seconds'),
                  ),
              ],
              onChanged: active
                  ? null
                  : (value) => change(() => options.countdown = value!),
            ),
            SwitchListTile(
              contentPadding: EdgeInsets.zero,
              title: const Text('Live annotations'),
              subtitle: const Text(
                'Draw, arrows, boxes and spotlight on the preview. Uses software encoding; higher CPU cost.',
              ),
              value: options.annotations,
              onChanged: active
                  ? null
                  : (value) => change(() => options.annotations = value),
            ),
            SwitchListTile(
              contentPadding: EdgeInsets.zero,
              title: const Text('Record mouse cursor'),
              value: options.captureCursor,
              onChanged: active
                  ? null
                  : (value) => change(() => options.captureCursor = value),
            ),
            const Align(
              alignment: Alignment.centerLeft,
              child: Text(
                'Ctrl+Shift+F9  Start / stop\nCtrl+Shift+F10  Pause / resume\nCtrl+Shift+F11  Save replay',
                style: TextStyle(height: 1.8),
              ),
            ),
            SwitchListTile(
              contentPadding: EdgeInsets.zero,
              title: const Text('Cursor highlight'),
              subtitle: const Text(
                'Display/region captures · software encoding',
              ),
              value: options.cursorHighlight,
              onChanged: active
                  ? null
                  : (v) => change(() => options.cursorHighlight = v),
            ),
            SwitchListTile(
              contentPadding: EdgeInsets.zero,
              title: const Text('Mouse click rings'),
              subtitle: const Text(
                'Display/region captures · software encoding',
              ),
              value: options.clickEffects,
              onChanged: active
                  ? null
                  : (v) => change(() => options.clickEffects = v),
            ),
            if (recorder.hotkeyConflicts.isNotEmpty)
              Text(
                'Unavailable shortcuts: ${recorder.hotkeyConflicts.join(', ')}',
              ),
          ],
        ),
        ExpansionTile(
          title: const Text('Audio devices & processing'),
          subtitle: const Text(
            'Live gain/mute, mixed track and microphone cleanup',
          ),
          childrenPadding: const EdgeInsets.all(16),
          children: [
            for (final system in [true, false]) ...[
              DropdownButtonFormField<String>(
                key: ValueKey('device-$system-${recorder.audioDevices.length}'),
                initialValue: system
                    ? options.systemDevice
                    : options.microphoneDevice,
                isExpanded: true,
                decoration: InputDecoration(
                  labelText: system
                      ? 'System audio device'
                      : 'Microphone device',
                ),
                items: [
                  const DropdownMenuItem(
                    value: '',
                    child: Text('System default'),
                  ),
                  if ((system ? options.systemDevice : options.microphoneDevice)
                          .isNotEmpty &&
                      !recorder.audioDevices.any(
                        (d) =>
                            d['id'] ==
                            (system
                                ? options.systemDevice
                                : options.microphoneDevice),
                      ))
                    DropdownMenuItem(
                      value: system
                          ? options.systemDevice
                          : options.microphoneDevice,
                      enabled: false,
                      child: const Text(
                        'Saved device unavailable · choose another',
                      ),
                    ),
                  ...recorder.audioDevices
                      .where(
                        (d) => d['kind'] == (system ? 'system' : 'microphone'),
                      )
                      .map(
                        (d) => DropdownMenuItem(
                          value: d['id'] as String,
                          child: Text(
                            d['name'] as String,
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                      ),
                ],
                onChanged: active
                    ? null
                    : (value) => change(() {
                        if (system) {
                          options.systemDevice = value!;
                        } else {
                          options.microphoneDevice = value!;
                        }
                      }),
              ),
              const SizedBox(height: 8),
              Row(
                children: [
                  IconButton(
                    tooltip: system ? 'Mute system audio' : 'Mute microphone',
                    icon: Icon(
                      (system ? options.systemMuted : options.microphoneMuted)
                          ? Icons.volume_off
                          : Icons.volume_up,
                    ),
                    onPressed: () => change(() {
                      if (system) {
                        options.systemMuted = !options.systemMuted;
                      } else {
                        options.microphoneMuted = !options.microphoneMuted;
                      }
                      recorder.updateAudio(options.audioControls);
                    }),
                  ),
                  Expanded(
                    child: Slider(
                      min: 0,
                      max: 2,
                      divisions: 40,
                      value: system
                          ? options.systemGain
                          : options.microphoneGain,
                      label:
                          '${((system ? options.systemGain : options.microphoneGain) * 100).round()}%',
                      onChanged: (value) => change(() {
                        if (system) {
                          options.systemGain = value;
                        } else {
                          options.microphoneGain = value;
                        }
                        recorder.updateAudio(options.audioControls);
                      }),
                    ),
                  ),
                  Text(
                    '${((system ? options.systemGain : options.microphoneGain) * 100).round()}%',
                  ),
                ],
              ),
              const SizedBox(height: 10),
            ],
            TextButton.icon(
              onPressed: active ? null : recorder.refreshAudioDevices,
              icon: const Icon(Icons.refresh),
              label: const Text('Refresh devices'),
            ),
            for (final item in <(String, bool, void Function(bool))>[
              (
                'Combined playback track',
                options.mixAudio,
                (v) => options.mixAudio = v,
              ),
              (
                'Microphone noise suppression',
                options.denoise,
                (v) => options.denoise = v,
              ),
              ('Microphone noise gate', options.gate, (v) => options.gate = v),
              ('Audio limiter', options.limiter, (v) => options.limiter = v),
            ])
              SwitchListTile(
                contentPadding: EdgeInsets.zero,
                title: Text(item.$1),
                value: item.$2,
                onChanged: active
                    ? null
                    : (value) => change(() => item.$3(value)),
              ),
            const Text(
              'The combined track is added when both system audio and microphone are recorded. Source tracks remain separate.',
            ),
          ],
        ),
        ExpansionTile(
          title: const Text('Recording automation'),
          subtitle: const Text(
            'Scheduled start and automatic recording limits',
          ),
          childrenPadding: const EdgeInsets.all(16),
          children: [
            DropdownButtonFormField<int>(
              initialValue: options.segmentSeconds,
              decoration: const InputDecoration(
                labelText: 'Recovery segments (MKV only)',
              ),
              items: [
                for (final seconds in [0, 60, 300, 900])
                  DropdownMenuItem(
                    value: seconds,
                    child: Text(
                      seconds == 0 ? 'Off' : 'Every ${seconds ~/ 60} minutes',
                    ),
                  ),
              ],
              onChanged: active
                  ? null
                  : (v) => change(() => options.segmentSeconds = v!),
            ),
            const Padding(
              padding: EdgeInsets.symmetric(vertical: 12),
              child: Text(
                'Completed parts remain playable after a crash. The active part may be incomplete. Replay history restarts at each segment.',
              ),
            ),
            TextFormField(
              initialValue: '${options.maxSeconds ~/ 60}',
              enabled: !active,
              keyboardType: TextInputType.number,
              decoration: const InputDecoration(
                labelText: 'Stop after (minutes)',
                helperText: '0 = no duration limit',
              ),
              onChanged: (value) {
                final minutes = int.tryParse(value);
                if (minutes != null && minutes >= 0 && minutes <= 1440) {
                  options.maxSeconds = minutes * 60;
                  onChanged();
                }
              },
            ),
            const SizedBox(height: 16),
            TextFormField(
              initialValue: '${options.maxFileMiB}',
              enabled: !active,
              keyboardType: TextInputType.number,
              decoration: const InputDecoration(
                labelText: 'Stop at file size (MiB)',
                helperText:
                    '0 = no size limit; encoded packets may slightly exceed the limit',
              ),
              onChanged: (value) {
                final mib = int.tryParse(value);
                if (mib != null && mib >= 0 && mib <= 1048576) {
                  options.maxFileMiB = mib;
                  onChanged();
                }
              },
            ),
            const SizedBox(height: 16),
            OutlinedButton.icon(
              onPressed: active ? null : onSchedule,
              icon: const Icon(Icons.schedule),
              label: const Text('Schedule recording'),
            ),
            const SizedBox(height: 8),
            const Text(
              'Keep Frame open. A schedule is cancelled when the app exits.',
            ),
            if (recorder.scheduledAt != null)
              TextButton(
                onPressed: recorder.cancelSchedule,
                child: const Text('Cancel scheduled recording'),
              ),
          ],
        ),
        ExpansionTile(
          title: const Text('Replay buffer'),
          subtitle: const Text('Save the last moments while recording'),
          childrenPadding: const EdgeInsets.all(16),
          children: [
            DropdownButtonFormField<int>(
              initialValue: options.replaySeconds,
              decoration: const InputDecoration(labelText: 'Replay history'),
              items: [
                for (final seconds in [0, 15, 30, 60, 120])
                  DropdownMenuItem(
                    value: seconds,
                    child: Text(seconds == 0 ? 'Off' : '$seconds seconds'),
                  ),
              ],
              onChanged: active
                  ? null
                  : (value) => change(() => options.replaySeconds = value!),
            ),
            const SizedBox(height: 12),
            Text('Memory budget: ${options.replayMiB} MiB'),
            Slider(
              min: 4,
              max: 128,
              divisions: 31,
              value: options.replayMiB.toDouble(),
              onChanged: active
                  ? null
                  : (value) => change(() => options.replayMiB = value.round()),
            ),
            const Text(
              'Encoded packets only. The memory budget can shorten high-bitrate history. Clips start on a keyframe and save as MKV. GIF replay is unavailable.',
            ),
          ],
        ),
      ],
    );
  }
}
