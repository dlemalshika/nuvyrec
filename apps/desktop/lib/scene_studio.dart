import 'dart:io';
import 'package:flutter/material.dart';
import 'controller.dart';
import 'creator_layers.dart';
import 'creator_studio.dart';
import 'feature_options.dart';

/// A draft never alters the live recording. Prepare assets, then explicitly
/// take a scene through one bounded native transaction. Stored scenes contain
/// visual descriptions only, with no output, encoder, audio or stream secrets.
class SceneStudio extends StatefulWidget {
  const SceneStudio({
    super.key,
    required this.recorder,
    required this.options,
    required this.scenes,
    required this.onChanged,
  });
  final RecorderController recorder;
  final FeatureOptions options;
  final List<Map<String, dynamic>> scenes;
  final VoidCallback onChanged;
  @override
  State<SceneStudio> createState() => _SceneStudioState();
}

class _SceneStudioState extends State<SceneStudio> {
  final draft = CreatorLayers();
  final camera = FeatureOptions();
  bool busy = true, layersEnabled = true;
  int selected = 0, duration = 0;
  String name = 'Current scene';
  String? error;
  bool get locked => busy || widget.recorder.sceneBusy || draft.loading;

  @override
  void initState() {
    super.initState();
    camera.restore({'webcam': widget.options.webcam});
    layersEnabled = widget.options.layersEnabled;
    draft.restore(widget.recorder.creatorLayers.toJson()).whenComplete(() {
      if (mounted) setState(() => busy = false);
    });
  }

  Future<void> run(Future<void> Function() operation) async {
    if (locked) return;
    setState(() {
      busy = true;
      error = null;
    });
    try {
      await operation();
    } catch (e) {
      if (mounted) error = e.toString();
    } finally {
      if (mounted) setState(() => busy = false);
    }
  }

  Future<void> load(Map<String, dynamic> scene) async {
    final candidate = CreatorLayers();
    try {
      await candidate.restore(scene['overlays'] as List);
      if (!candidate.ready) {
        throw StateError(
          'A scene logo is unavailable. Restore its file before taking this scene.',
        );
      }
      if (!mounted) return;
      draft.adopt(candidate);
      camera.restore({'webcam': scene['webcam']});
      layersEnabled = scene['layers_enabled'] != false;
      name = scene['name'] as String;
      selected = 0;
    } finally {
      candidate.dispose();
    }
  }

  Future<void> take() async {
    await widget.recorder.applyScene(
      overlays: draft.toJson(),
      webcam: camera.webcam,
      layersEnabled: layersEnabled,
      name: name,
      durationMs: duration,
    );
    if (!mounted) return;
    final options = widget.options;
    options.layersEnabled = layersEnabled;
    // The camera already opened by the session stays selected during a take.
    if (!widget.recorder.active) options.webcamDevice = camera.webcamDevice;
    options.webcamEnabled = camera.webcamEnabled;
    options.webcamX = camera.webcamX;
    options.webcamY = camera.webcamY;
    options.webcamWidth = camera.webcamWidth;
    options.webcamMirror = camera.webcamMirror;
    options.webcamKey = camera.webcamKey;
    options.webcamSimilarity = camera.webcamSimilarity;
    options.webcamSoftness = camera.webcamSoftness;
    widget.onChanged();
  }

  Future<void> save() async {
    if (!draft.ready) throw StateError('Wait for usable scene assets');
    final input = TextEditingController(
      text: name == 'Current scene' ? '' : name,
    );
    String? next;
    try {
      next = await showDialog<String>(
        context: context,
        builder: (context) => AlertDialog(
          title: const Text('Save visual scene'),
          content: TextField(
            controller: input,
            maxLength: 48,
            decoration: const InputDecoration(labelText: 'Scene name'),
            autofocus: true,
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
    } finally {
      input.dispose();
    }
    if (!mounted || next == null || next.isEmpty || next.runes.length > 48) {
      return;
    }
    final index = widget.scenes.indexWhere((s) => s['name'] == next);
    if (index < 0 && widget.scenes.length >= 20) {
      throw StateError('Up to 20 visual scenes');
    }
    final scene = <String, dynamic>{
      'name': next,
      'overlays': draft.toJson(),
      'layers_enabled': layersEnabled,
      'webcam': camera.webcam,
    };
    if (index < 0) {
      widget.scenes.add(scene);
    } else {
      widget.scenes[index] = scene;
    }
    name = next;
    if (!widget.recorder.active && widget.recorder.scheduledAt == null) {
      widget.options.scenesEnabled = true;
    }
    widget.onChanged();
  }

  Future<void> text({int? index}) async {
    final spec = await showTextOverlayEditor(
      context,
      index == null ? null : draft.entries[index].spec,
    );
    if (spec == null || !mounted) return;
    if (index == null) {
      await draft.add(spec);
      selected = draft.entries.length - 1;
    } else {
      await draft.update(index, spec);
    }
    layersEnabled = true;
  }

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: widget.recorder,
    builder: (context, _) {
      final active = widget.recorder.active;
      final scheduled = widget.recorder.scheduledAt != null;
      final onAir = widget.recorder.status['scene_name'];
      final ready =
          !locked &&
          !scheduled &&
          draft.ready &&
          (!active || widget.recorder.canEditScene);
      selected = selected.clamp(0, (draft.entries.length - 1).clamp(0, 3));
      final entry = draft.entries.isEmpty ? null : draft.entries[selected];
      void edit(OverlaySpec spec) => run(() => draft.update(selected, spec));
      return AlertDialog(
        title: const Text('Scenes & live overlays'),
        content: SizedBox(
          width: 600,
          child: SingleChildScrollView(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                const Text(
                  'Scenes share the selected capture source. Draft edits stay here until you press Apply / Take live. Audio, quality and streaming continue unchanged.',
                ),
                SwitchListTile(
                  contentPadding: EdgeInsets.zero,
                  title: const Text(
                    'Enable live scenes for the next recording',
                  ),
                  subtitle: const Text(
                    'Optional compositor work. Keep off for the lightest recording. Live scenes require H.264; GIF is excluded.',
                  ),
                  value: widget.options.scenesEnabled,
                  onChanged: active || scheduled || locked
                      ? null
                      : (v) => setState(() {
                          widget.options.scenesEnabled = v;
                          widget.onChanged();
                        }),
                ),
                if (active &&
                    widget.recorder.status['scenes_available'] != true)
                  const Text(
                    'Enable scenes before starting an H.264 recording to take live edits.',
                  ),
                if (scheduled)
                  const Text(
                    'Cancel the scheduled recording before applying a new visual layout.',
                  ),
                if (active)
                  Text(
                    'On air: ${onAir == null || onAir == '' ? 'Initial layout' : onAir}',
                  ),
                if (error != null)
                  Text(
                    error!,
                    style: TextStyle(
                      color: Theme.of(context).colorScheme.error,
                    ),
                  ),
                if (locked) const LinearProgressIndicator(),
                for (final scene in widget.scenes)
                  ListTile(
                    contentPadding: EdgeInsets.zero,
                    title: Text(scene['name'] as String),
                    subtitle: const Text('Load into draft'),
                    onTap: locked ? null : () => run(() => load(scene)),
                    trailing: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        IconButton(
                          tooltip: 'Take this scene',
                          icon: const Icon(Icons.play_arrow),
                          onPressed: ready
                              ? () => run(() async {
                                  await load(scene);
                                  await take();
                                })
                              : null,
                        ),
                        IconButton(
                          tooltip: 'Delete saved scene',
                          icon: const Icon(Icons.delete_outline),
                          onPressed: locked
                              ? null
                              : () => setState(() {
                                  widget.scenes.remove(scene);
                                  widget.onChanged();
                                }),
                        ),
                      ],
                    ),
                  ),
                const Divider(),
                Text(
                  'Draft: $name',
                  style: Theme.of(context).textTheme.titleMedium,
                ),
                DropdownButtonFormField<int>(
                  initialValue: duration,
                  decoration: const InputDecoration(labelText: 'Transition'),
                  items: const [
                    DropdownMenuItem(
                      value: 0,
                      child: Text('Cut · lowest cost'),
                    ),
                    DropdownMenuItem(
                      value: 300,
                      child: Text('Fade through black · 300 ms'),
                    ),
                    DropdownMenuItem(
                      value: 600,
                      child: Text('Fade through black · 600 ms'),
                    ),
                    DropdownMenuItem(
                      value: 1000,
                      child: Text('Fade through black · 1 second'),
                    ),
                  ],
                  onChanged: locked
                      ? null
                      : (v) => setState(() => duration = v!),
                ),
                SwitchListTile(
                  contentPadding: EdgeInsets.zero,
                  title: const Text('Show text and logos'),
                  value: layersEnabled,
                  onChanged: locked
                      ? null
                      : (v) => setState(() => layersEnabled = v),
                ),
                Wrap(
                  spacing: 8,
                  children: [
                    OutlinedButton.icon(
                      onPressed: locked || draft.entries.length >= 4
                          ? null
                          : () => run(() => text()),
                      icon: const Icon(Icons.title),
                      label: const Text('Add text'),
                    ),
                    OutlinedButton.icon(
                      onPressed: locked || draft.entries.length >= 4
                          ? null
                          : () => run(() async {
                              final path = await RecorderController.host
                                  .invokeMethod<String>('chooseOverlayImage');
                              if (path == null || !mounted) return;
                              await draft.add(
                                OverlaySpec(
                                  kind: 'image',
                                  content: path,
                                  width: .15,
                                  x: 1,
                                ),
                              );
                              selected = draft.entries.length - 1;
                              layersEnabled = true;
                            }),
                      icon: const Icon(Icons.image_outlined),
                      label: const Text('Add logo'),
                    ),
                  ],
                ),
                for (var i = 0; i < draft.entries.length; i++)
                  ListTile(
                    contentPadding: EdgeInsets.zero,
                    selected: i == selected,
                    title: Text(
                      draft.entries[i].spec.kind == 'text'
                          ? draft.entries[i].spec.content
                          : File(
                              draft.entries[i].spec.content,
                            ).uri.pathSegments.last,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                    subtitle: draft.entries[i].error == null
                        ? null
                        : Text(draft.entries[i].error!),
                    onTap: locked ? null : () => setState(() => selected = i),
                    trailing: IconButton(
                      tooltip: 'Remove draft layer',
                      icon: const Icon(Icons.delete_outline),
                      onPressed: locked
                          ? null
                          : () => setState(() => draft.remove(i)),
                    ),
                  ),
                if (entry != null) ...[
                  if (entry.spec.kind == 'text')
                    TextButton.icon(
                      onPressed: locked
                          ? null
                          : () => run(() => text(index: selected)),
                      icon: const Icon(Icons.edit),
                      label: const Text('Edit draft text & style'),
                    ),
                  const Text('Layer position · horizontal / vertical'),
                  Slider(
                    value: entry.spec.x,
                    onChanged: locked
                        ? null
                        : (v) => edit(entry.spec.copyWith(x: v)),
                  ),
                  Slider(
                    value: entry.spec.y,
                    onChanged: locked
                        ? null
                        : (v) => edit(entry.spec.copyWith(y: v)),
                  ),
                  Text('Layer width ${(entry.spec.width * 100).round()}%'),
                  Slider(
                    value: entry.spec.width,
                    min: .02,
                    max: 1,
                    onChanged: locked
                        ? null
                        : (v) => edit(entry.spec.copyWith(width: v)),
                  ),
                  Text('Opacity ${(entry.spec.opacity * 100).round()}%'),
                  Slider(
                    value: entry.spec.opacity,
                    onChanged: locked
                        ? null
                        : (v) => edit(entry.spec.copyWith(opacity: v)),
                  ),
                ],
                const Divider(),
                SwitchListTile(
                  contentPadding: EdgeInsets.zero,
                  title: const Text('Show webcam in this scene'),
                  subtitle: const Text(
                    'Choose and enable the camera in Settings before recording. A live scene keeps that device.',
                  ),
                  value: camera.webcamEnabled,
                  onChanged:
                      locked ||
                          (active
                              ? widget.recorder.status['webcam_available'] !=
                                    true
                              : camera.webcamDevice.isEmpty)
                      ? null
                      : (v) => setState(() => camera.webcamEnabled = v),
                ),
                const Text('Webcam position · horizontal / vertical'),
                Slider(
                  value: camera.webcamX,
                  onChanged: locked
                      ? null
                      : (v) => setState(() => camera.webcamX = v),
                ),
                Slider(
                  value: camera.webcamY,
                  onChanged: locked
                      ? null
                      : (v) => setState(() => camera.webcamY = v),
                ),
                Text('Webcam width ${(camera.webcamWidth * 100).round()}%'),
                Slider(
                  value: camera.webcamWidth,
                  min: .1,
                  max: .5,
                  onChanged: locked
                      ? null
                      : (v) => setState(() => camera.webcamWidth = v),
                ),
                SwitchListTile(
                  contentPadding: EdgeInsets.zero,
                  title: const Text('Mirror webcam'),
                  value: camera.webcamMirror,
                  onChanged: locked
                      ? null
                      : (v) => setState(() => camera.webcamMirror = v),
                ),
                TextButton.icon(
                  onPressed: locked || !draft.ready ? null : () => run(save),
                  icon: const Icon(Icons.bookmark_add_outlined),
                  label: const Text('Save draft as scene'),
                ),
              ],
            ),
          ),
        ),
        actions: [
          TextButton(
            onPressed: locked ? null : () => Navigator.pop(context),
            child: const Text('Close'),
          ),
          FilledButton.icon(
            onPressed: ready ? () => run(take) : null,
            icon: const Icon(Icons.play_arrow),
            label: Text(active ? 'Take live' : 'Apply layout'),
          ),
        ],
      );
    },
  );

  @override
  void dispose() {
    draft.dispose();
    super.dispose();
  }
}
