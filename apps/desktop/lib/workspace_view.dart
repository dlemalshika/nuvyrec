import 'dart:io';
import 'package:flutter/material.dart';
import 'annotation_preview.dart';
import 'controller.dart';
import 'frame_theme.dart';

/// The everyday recording workspace. Detailed encoding controls live in settings.
class RecorderWorkspace extends StatelessWidget {
  const RecorderWorkspace({
    super.key,
    required this.recorder,
    required this.sourceControls,
    required this.previewSource,
    required this.previewEnabled,
    required this.onPreviewToggle,
    required this.onSettings,
    required this.onLibrary,
    required this.summary,
    required this.format,
    required this.onStart,
  });
  final RecorderController recorder;
  final Widget sourceControls;
  final Map<String, dynamic>? previewSource;
  final bool previewEnabled;
  final VoidCallback onPreviewToggle, onSettings, onStart, onLibrary;
  final String summary, format;

  @override
  Widget build(BuildContext context) {
    final active = recorder.active;
    final colors = FrameColors.of(context);
    final seconds = (recorder.status['seconds'] as num? ?? 0).toInt();
    final time =
        '${(seconds ~/ 3600).toString().padLeft(2, '0')}:${(seconds ~/ 60 % 60).toString().padLeft(2, '0')}:${(seconds % 60).toString().padLeft(2, '0')}';
    return Scaffold(
      bottomNavigationBar: Container(
        decoration: BoxDecoration(
          color: colors.panel,
          border: Border(top: BorderSide(color: colors.border)),
        ),
        padding: const EdgeInsets.symmetric(horizontal: 28, vertical: 18),
        child: SafeArea(
          child: Row(
            children: [
              Container(
                width: 10,
                height: 10,
                decoration: BoxDecoration(
                  shape: BoxShape.circle,
                  color: recorder.state == 'recording'
                      ? Colors.redAccent
                      : colors.accent,
                ),
              ),
              const SizedBox(width: 14),
              Text(
                recorder.countdown > 0 ? '${recorder.countdown}' : time,
                style: const TextStyle(
                  fontSize: 30,
                  fontWeight: FontWeight.w500,
                  fontFeatures: [FontFeature.tabularFigures()],
                ),
              ),
              const SizedBox(width: 22),
              Expanded(
                child: Text(
                  '${recorder.status['frames'] ?? 0} frames\n${recorder.status['encoder'] ?? 'Ready to record'}',
                  style: TextStyle(color: colors.muted, fontSize: 12),
                ),
              ),
              if (['recording', 'paused'].contains(recorder.state)) ...[
                OutlinedButton.icon(
                  onPressed: () => recorder.action(
                    recorder.state == 'paused' ? 'resume' : 'pause',
                  ),
                  icon: Icon(
                    recorder.state == 'paused'
                        ? Icons.play_arrow_rounded
                        : Icons.pause_rounded,
                  ),
                  label: Text(recorder.state == 'paused' ? 'Resume' : 'Pause'),
                ),
                const SizedBox(width: 12),
              ],
              FilledButton.icon(
                style: FilledButton.styleFrom(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 26,
                    vertical: 20,
                  ),
                  backgroundColor: active
                      ? const Color(0xffffb4ab)
                      : colors.accent,
                  foregroundColor: active
                      ? const Color(0xff11241f)
                      : Theme.of(context).colorScheme.onPrimary,
                ),
                onPressed: active
                    ? (recorder.state == 'stopping'
                          ? null
                          : () => recorder.action('stop'))
                    : (previewSource == null || recorder.output == null
                          ? null
                          : onStart),
                icon: Icon(
                  active ? Icons.stop_rounded : Icons.fiber_manual_record,
                ),
                label: Text(
                  recorder.countdown > 0
                      ? 'Cancel countdown'
                      : active
                      ? 'Stop & save'
                      : 'Start recording',
                  style: const TextStyle(fontWeight: FontWeight.w700),
                ),
              ),
            ],
          ),
        ),
      ),
      body: SafeArea(
        child: Padding(
          padding: const EdgeInsets.fromLTRB(28, 20, 28, 20),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  Container(
                    padding: const EdgeInsets.all(10),
                    decoration: BoxDecoration(
                      color: colors.accent.withValues(alpha: .12),
                      borderRadius: BorderRadius.circular(12),
                    ),
                    child: Icon(
                      Icons.radio_button_checked,
                      color: colors.accent,
                    ),
                  ),
                  const SizedBox(width: 12),
                  Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        'FRAME',
                        style: TextStyle(
                          fontSize: 20,
                          fontWeight: FontWeight.w800,
                          letterSpacing: 3,
                        ),
                      ),
                      Text(
                        'Your screen. Your story.',
                        style: TextStyle(color: colors.subtle, fontSize: 11),
                      ),
                    ],
                  ),
                  const Spacer(),
                  Chip(
                    label: Text(
                      recorder.state.toUpperCase(),
                      style: const TextStyle(fontSize: 11),
                    ),
                    avatar: Icon(
                      active
                          ? Icons.fiber_manual_record
                          : Icons.check_circle_outline,
                      size: 14,
                      color: active ? Colors.redAccent : colors.accent,
                    ),
                  ),
                  const SizedBox(width: 16),
                  if (recorder.status['replay_enabled'] == true && active)
                    IconButton(
                      onPressed: recorder.saveReplay,
                      tooltip: 'Save replay · Ctrl+Shift+F11',
                      icon: const Icon(Icons.history),
                    ),
                  IconButton(
                    onPressed: onLibrary,
                    tooltip: 'Recording library',
                    icon: const Icon(Icons.video_library_outlined),
                  ),
                  IconButton(
                    onPressed: () => RecorderController.host.invokeMethod<void>(
                      'hideToTray',
                    ),
                    tooltip: 'Hide to tray',
                    icon: const Icon(Icons.minimize),
                  ),
                  OutlinedButton.icon(
                    onPressed: onSettings,
                    icon: const Icon(Icons.tune_rounded, size: 18),
                    label: const Text('Settings'),
                  ),
                ],
              ),
              const SizedBox(height: 24),
              if (recorder.scheduledAt != null)
                Padding(
                  padding: const EdgeInsets.only(bottom: 12),
                  child: Row(
                    children: [
                      const Icon(Icons.schedule, size: 18),
                      const SizedBox(width: 8),
                      Expanded(
                        child: Text(
                          'Scheduled: ${recorder.scheduledAt!.toLocal()} · keep Frame open',
                        ),
                      ),
                      TextButton(
                        onPressed: recorder.cancelSchedule,
                        child: const Text('Cancel'),
                      ),
                    ],
                  ),
                ),
              if (recorder.error != null) ...[
                Container(
                  width: double.infinity,
                  padding: const EdgeInsets.all(12),
                  decoration: BoxDecoration(
                    color: Theme.of(context).colorScheme.errorContainer,
                    borderRadius: BorderRadius.circular(12),
                  ),
                  child: Text(
                    recorder.error!,
                    maxLines: 3,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                      color: Theme.of(context).colorScheme.onErrorContainer,
                    ),
                  ),
                ),
                const SizedBox(height: 14),
              ],
              Expanded(
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Row(
                            children: [
                              const Text(
                                'Capture preview',
                                style: TextStyle(
                                  fontSize: 18,
                                  fontWeight: FontWeight.w600,
                                ),
                              ),
                              const Spacer(),
                              IconButton(
                                onPressed: previewSource == null
                                    ? null
                                    : () => recorder.screenshot(previewSource!),
                                tooltip: 'Save screenshot',
                                icon: const Icon(
                                  Icons.photo_camera_outlined,
                                  size: 19,
                                ),
                              ),
                              IconButton(
                                onPressed: onPreviewToggle,
                                tooltip: previewEnabled
                                    ? 'Turn preview off'
                                    : 'Turn preview on',
                                icon: Icon(
                                  previewEnabled
                                      ? Icons.visibility_outlined
                                      : Icons.visibility_off_outlined,
                                  size: 19,
                                  color: colors.muted,
                                ),
                              ),
                            ],
                          ),
                          const SizedBox(height: 8),
                          Expanded(
                            child: AnnotationPreview(
                              recorder: recorder,
                              source: previewSource,
                              enabled: previewEnabled,
                            ),
                          ),
                          const SizedBox(height: 14),
                          Row(
                            children: [
                              Icon(
                                Icons.high_quality_outlined,
                                color: colors.subtle,
                                size: 18,
                              ),
                              const SizedBox(width: 8),
                              Expanded(
                                child: Text(
                                  summary,
                                  style: TextStyle(
                                    fontSize: 12,
                                    color: colors.muted,
                                  ),
                                ),
                              ),
                              TextButton(
                                onPressed: onSettings,
                                child: const Text('Change'),
                              ),
                            ],
                          ),
                        ],
                      ),
                    ),
                    const SizedBox(width: 22),
                    SizedBox(
                      width: 320,
                      child: SingleChildScrollView(
                        child: Column(
                          children: [
                            sourceControls,
                            const SizedBox(height: 16),
                            _RecordingHealth(recorder: recorder),
                            const SizedBox(height: 16),
                            Container(
                              padding: const EdgeInsets.all(20),
                              decoration: BoxDecoration(
                                color: colors.panel,
                                borderRadius: BorderRadius.circular(16),
                                border: Border.all(color: colors.border),
                              ),
                              child: Column(
                                crossAxisAlignment: CrossAxisAlignment.start,
                                children: [
                                  Row(
                                    children: [
                                      Icon(
                                        Icons.folder_outlined,
                                        size: 18,
                                        color: colors.accent,
                                      ),
                                      SizedBox(width: 8),
                                      Text(
                                        'Save recording',
                                        style: TextStyle(
                                          fontWeight: FontWeight.w600,
                                        ),
                                      ),
                                    ],
                                  ),
                                  const SizedBox(height: 18),
                                  Row(
                                    children: [
                                      Container(
                                        padding: const EdgeInsets.symmetric(
                                          horizontal: 8,
                                          vertical: 4,
                                        ),
                                        decoration: BoxDecoration(
                                          color: colors.badge,
                                          borderRadius: BorderRadius.circular(
                                            5,
                                          ),
                                        ),
                                        child: Text(
                                          format.toUpperCase(),
                                          style: const TextStyle(
                                            fontSize: 11,
                                            fontWeight: FontWeight.w700,
                                          ),
                                        ),
                                      ),
                                      const SizedBox(width: 10),
                                      Expanded(
                                        child: Text(
                                          recorder.output == null
                                              ? 'Choose where to save'
                                              : _filename(recorder.output!),
                                          maxLines: 2,
                                          overflow: TextOverflow.ellipsis,
                                          style: TextStyle(
                                            color: colors.secondary,
                                            fontSize: 12,
                                          ),
                                        ),
                                      ),
                                    ],
                                  ),
                                  const SizedBox(height: 16),
                                  SizedBox(
                                    width: double.infinity,
                                    child: OutlinedButton.icon(
                                      onPressed: active
                                          ? null
                                          : () => recorder.chooseOutput(format),
                                      icon: const Icon(
                                        Icons.folder_open_rounded,
                                        size: 18,
                                      ),
                                      label: const Text('Choose file'),
                                    ),
                                  ),
                                  if (recorder.state == 'stopped')
                                    TextButton.icon(
                                      onPressed: recorder.revealOutput,
                                      icon: const Icon(
                                        Icons.open_in_new,
                                        size: 16,
                                      ),
                                      label: const Text('Show saved recording'),
                                    ),
                                ],
                              ),
                            ),
                            const SizedBox(height: 16),
                            Padding(
                              padding: EdgeInsets.symmetric(horizontal: 8),
                              child: Text(
                                'Choose a source, check the preview, then start recording. Turn preview off to save resources.',
                                style: TextStyle(
                                  color: colors.subtle,
                                  fontSize: 12,
                                  height: 1.5,
                                ),
                              ),
                            ),
                          ],
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  String _filename(String path) =>
      path.split(Platform.pathSeparator).last.split('\\').last;
}

class _RecordingHealth extends StatelessWidget {
  const _RecordingHealth({required this.recorder});
  final RecorderController recorder;
  @override
  Widget build(BuildContext context) {
    final colors = FrameColors.of(context);
    final status = recorder.status;
    return Container(
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: colors.panel,
        border: Border.all(color: colors.border),
        borderRadius: BorderRadius.circular(16),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const Text(
            'Recording health',
            style: TextStyle(fontWeight: FontWeight.w600),
          ),
          const SizedBox(height: 12),
          for (final item in [
            ('System audio', 'system_peak'),
            ('Microphone', 'microphone_peak'),
          ]) ...[
            Text(item.$1, style: TextStyle(fontSize: 11, color: colors.muted)),
            const SizedBox(height: 5),
            LinearProgressIndicator(
              value: (status[item.$2] as num? ?? 0).toDouble().clamp(0, 1),
            ),
            const SizedBox(height: 10),
          ],
          Text(
            '${((status['file_bytes'] as num? ?? 0) / 1048576).toStringAsFixed(1)} MiB recorded · ${status['skipped_frame_slots'] ?? 0} skipped frame slots',
            style: TextStyle(fontSize: 11, color: colors.muted),
          ),
          if (recorder.freeBytes != null)
            Text(
              '${(recorder.freeBytes! / 1073741824).toStringAsFixed(1)} GiB free when file selected',
              style: TextStyle(fontSize: 11, color: colors.muted),
            ),
          if (status['replay_enabled'] == true)
            Text(
              'Replay: ${(status['replay_seconds'] as num? ?? 0).toStringAsFixed(1)} s · ${((status['replay_bytes'] as num? ?? 0) / 1048576).toStringAsFixed(1)} MiB',
              style: TextStyle(fontSize: 11, color: colors.muted),
            ),
          if (recorder.notice != null)
            Padding(
              padding: const EdgeInsets.only(top: 8),
              child: Text(
                recorder.notice!,
                style: TextStyle(fontSize: 12, color: colors.accent),
              ),
            ),
        ],
      ),
    );
  }
}
