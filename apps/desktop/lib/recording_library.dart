import 'dart:io';
import 'package:flutter/material.dart';
import 'controller.dart';
import 'media_jobs.dart';
import 'export_presets.dart';

Future<void> showRecordingLibrary(
  BuildContext context,
  RecorderController recorder,
  MediaJobs jobs,
) => showDialog<void>(
  context: context,
  builder: (context) => Dialog(
    child: SizedBox(
      width: 820,
      height: 580,
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: AnimatedBuilder(
          animation: Listenable.merge([recorder, jobs]),
          builder: (context, _) => Column(
            children: [
              Row(
                children: [
                  const Icon(Icons.video_library_outlined),
                  const SizedBox(width: 12),
                  const Text('Recordings', style: TextStyle(fontSize: 24)),
                  const Spacer(),
                  TextButton.icon(
                    icon: const Icon(Icons.file_open_outlined),
                    label: const Text('Import'),
                    onPressed: jobs.busy
                        ? null
                        : () async {
                            try {
                              final path = await RecorderController.host
                                  .invokeMethod<String>('chooseRecording');
                              if (path == null) return;
                              final info = await jobs.probe(path);
                              if (!recorder.library.any(
                                (entry) => entry['path'] == path,
                              )) {
                                recorder.library.insert(0, {
                                  'path': path,
                                  'date': DateTime.now().toIso8601String(),
                                  'seconds':
                                      double.tryParse(
                                        '${(info['format'] as Map?)?['duration']}',
                                      ) ??
                                      0,
                                });
                                if (recorder.library.length > 1000) {
                                  recorder.library.removeLast();
                                }
                                await recorder.savePreferences();
                                recorder.notifyLibraryChanged();
                              }
                            } catch (e) {
                              jobs.status = e.toString();
                              jobs.notifyJobChanged();
                            }
                          },
                  ),
                  IconButton(
                    onPressed: () => Navigator.pop(context),
                    icon: const Icon(Icons.close),
                  ),
                ],
              ),
              const SizedBox(height: 16),
              if (jobs.busy) ...[
                LinearProgressIndicator(value: jobs.progress),
                Row(
                  children: [
                    Expanded(child: Text(jobs.status)),
                    TextButton(
                      onPressed: jobs.cancel,
                      child: const Text('Cancel export'),
                    ),
                  ],
                ),
              ],
              Expanded(
                child: recorder.library.isEmpty
                    ? const Center(
                        child: Text('Your saved recordings will appear here.'),
                      )
                    : ListView.separated(
                        itemCount: recorder.library.length,
                        separatorBuilder: (_, _) => const Divider(),
                        itemBuilder: (context, index) {
                          final entry = recorder.library[index];
                          final path = entry['path'] as String;
                          return ListTile(
                            leading: const Icon(Icons.movie_outlined),
                            title: Text(path.split('\\').last),
                            subtitle: Text(
                              '${entry['date']} · ${(entry['seconds'] as num? ?? 0).round()} seconds',
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                            ),
                            onTap: () => recorder.openRecording(path),
                            trailing: PopupMenuButton<String>(
                              enabled: !jobs.busy,
                              onSelected: (action) async {
                                if (action == 'reveal') {
                                  recorder.openRecording(path, reveal: true);
                                  return;
                                }
                                if (action == 'forget') {
                                  recorder.library.removeAt(index);
                                  recorder.savePreferences();
                                  recorder.notifyLibraryChanged();
                                  return;
                                }
                                await showExportDialog(context, path, jobs);
                              },
                              itemBuilder: (_) => const [
                                PopupMenuItem(
                                  value: 'reveal',
                                  child: Text('Show in folder'),
                                ),
                                PopupMenuItem(
                                  value: 'export',
                                  child: Text('Trim / crop / export'),
                                ),
                                PopupMenuItem(
                                  value: 'forget',
                                  child: Text(
                                    'Remove from library (keep file)',
                                  ),
                                ),
                              ],
                            ),
                          );
                        },
                      ),
              ),
              if (jobs.status.isNotEmpty && !jobs.busy)
                Text(jobs.status, maxLines: 2, overflow: TextOverflow.ellipsis),
            ],
          ),
        ),
      ),
    ),
  ),
);

Future<void> showExportDialog(
  BuildContext context,
  String input,
  MediaJobs jobs,
) async {
  String mode = 'remux';
  ExportPreset preset = ExportPreset.original;
  ExportAudio audio = ExportAudio.all;
  String? validationError;
  bool choosingOutput = false;
  final start = TextEditingController(text: '0'),
      duration = TextEditingController();
  final crop = List.generate(
    4,
    (i) => TextEditingController(text: i < 2 ? '0' : ''),
  );
  try {
    await showDialog<void>(
      context: context,
      builder: (context) => StatefulBuilder(
        builder: (context, update) => AlertDialog(
          title: const Text('Export recording'),
          content: SizedBox(
            width: 430,
            child: SingleChildScrollView(
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  DropdownButtonFormField<String>(
                    initialValue: mode,
                    isExpanded: true,
                    decoration: const InputDecoration(
                      labelText: 'Export operation',
                    ),
                    items: const [
                      DropdownMenuItem(
                        value: 'remux',
                        child: Text('MP4 remux · no quality loss'),
                      ),
                      DropdownMenuItem(
                        value: 'trim',
                        child: Text('Trim · precise re-encode'),
                      ),
                      DropdownMenuItem(
                        value: 'crop',
                        child: Text('Crop and trim'),
                      ),
                      DropdownMenuItem(
                        value: 'preset',
                        child: Text('Share / compress'),
                      ),
                    ],
                    onChanged: choosingOutput
                        ? null
                        : (value) => update(() {
                            mode = value!;
                            validationError = null;
                            if (mode == 'preset') {
                              preset = ExportPreset.youtube;
                              audio = ExportAudio.first;
                            } else {
                              preset = ExportPreset.original;
                              audio = ExportAudio.all;
                            }
                          }),
                  ),
                  if (mode != 'remux') ...[
                    const SizedBox(height: 16),
                    DropdownButtonFormField<ExportPreset>(
                      key: ValueKey('preset-$mode'),
                      initialValue: preset,
                      isExpanded: true,
                      decoration: const InputDecoration(
                        labelText: 'Export preset',
                      ),
                      items: ExportPreset.values
                          .map(
                            (p) => DropdownMenuItem(
                              value: p,
                              child: Text(p.label),
                            ),
                          )
                          .toList(),
                      onChanged: choosingOutput
                          ? null
                          : (p) => update(() => preset = p!),
                    ),
                    const SizedBox(height: 8),
                    Text(preset.description),
                    const SizedBox(height: 16),
                    TextField(
                      controller: start,
                      decoration: const InputDecoration(
                        labelText: 'Start (seconds)',
                      ),
                      keyboardType: TextInputType.number,
                    ),
                    const SizedBox(height: 16),
                    TextField(
                      controller: duration,
                      decoration: const InputDecoration(
                        labelText: 'Duration (seconds)',
                        helperText: 'Blank = to the end',
                      ),
                      keyboardType: TextInputType.number,
                    ),
                  ],
                  if (mode == 'crop') ...[
                    const SizedBox(height: 16),
                    for (var i = 0; i < 4; i++)
                      Padding(
                        padding: const EdgeInsets.only(bottom: 12),
                        child: TextField(
                          controller: crop[i],
                          keyboardType: TextInputType.number,
                          decoration: InputDecoration(
                            labelText: [
                              'Left',
                              'Top',
                              'Width (even)',
                              'Height (even)',
                            ][i],
                          ),
                        ),
                      ),
                  ],
                  const SizedBox(height: 16),
                  DropdownButtonFormField<ExportAudio>(
                    key: ValueKey('audio-$mode'),
                    initialValue: audio,
                    isExpanded: true,
                    decoration: const InputDecoration(
                      labelText: 'Export audio',
                    ),
                    items: ExportAudio.values
                        .map(
                          (a) =>
                              DropdownMenuItem(value: a, child: Text(a.label)),
                        )
                        .toList(),
                    onChanged: choosingOutput
                        ? null
                        : (a) => update(() => audio = a!),
                  ),
                  if (audio == ExportAudio.first)
                    const Padding(
                      padding: EdgeInsets.only(top: 8),
                      child: Text(
                        'Uses the first recorded audio track. Separate system/mic tracks are not mixed during export.',
                      ),
                    ),
                  const SizedBox(height: 16),
                  const Text(
                    'Local MP4 export • H.264 / AAC • bounded video threads. Your source file is kept.',
                  ),
                  if (validationError != null)
                    Padding(
                      padding: const EdgeInsets.only(top: 12),
                      child: Text(
                        validationError!,
                        style: TextStyle(
                          color: Theme.of(context).colorScheme.error,
                        ),
                      ),
                    ),
                ],
              ),
            ),
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.pop(context),
              child: const Text('Cancel'),
            ),
            FilledButton(
              onPressed: choosingOutput
                  ? null
                  : () async {
                      update(() {
                        choosingOutput = true;
                        validationError = null;
                      });
                      try {
                        if (!await File(input).exists()) {
                          throw StateError('Recording file is missing');
                        }
                        if (!context.mounted) return;
                        final from = double.tryParse(start.text);
                        final seconds = duration.text.trim().isEmpty
                            ? null
                            : double.tryParse(duration.text);
                        if (mode != 'remux' &&
                            (from == null ||
                                !from.isFinite ||
                                from < 0 ||
                                (duration.text.trim().isNotEmpty &&
                                    (seconds == null ||
                                        !seconds.isFinite ||
                                        seconds <= 0)))) {
                          throw ArgumentError('Enter valid trim times');
                        }
                        final area = mode == 'crop'
                            ? crop.map((c) => int.tryParse(c.text)).toList()
                            : null;
                        if (area != null &&
                            (area.any((v) => v == null) ||
                                area[0]! < 0 ||
                                area[1]! < 0 ||
                                area[2]! < 2 ||
                                area[3]! < 2 ||
                                area[2]! % 2 != 0 ||
                                area[3]! % 2 != 0)) {
                          throw ArgumentError(
                            'Crop needs non-negative offsets and even dimensions of at least 2',
                          );
                        }
                        final selectedMode = mode,
                            selectedPreset = preset,
                            selectedAudio = audio;
                        final output = await RecorderController.host
                            .invokeMethod<String>('chooseOutput', {
                              'format': 'mp4',
                            });
                        if (output == null || !context.mounted) return;
                        Navigator.pop(context);
                        await jobs.export(
                          input,
                          output,
                          mode: selectedMode,
                          preset: selectedPreset,
                          audio: selectedAudio,
                          start: selectedMode == 'remux' ? 0 : from!,
                          duration: selectedMode == 'remux' ? null : seconds,
                          crop: area?.cast<int>(),
                        );
                      } catch (error) {
                        if (context.mounted) {
                          update(() => validationError = error.toString());
                        }
                        jobs.status = error.toString();
                        jobs.notifyJobChanged();
                      } finally {
                        if (context.mounted) {
                          update(() => choosingOutput = false);
                        }
                      }
                    },
              child: const Text('Export'),
            ),
          ],
        ),
      ),
    );
  } finally {
    start.dispose();
    duration.dispose();
    for (final field in crop) {
      field.dispose();
    }
  }
}
