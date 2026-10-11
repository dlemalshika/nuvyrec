import 'dart:io';
import 'package:flutter/material.dart';
import 'controller.dart';
import 'creator_layers.dart';
import 'feature_options.dart';

class CreatorStudio extends StatefulWidget {
  const CreatorStudio({
    super.key,
    required this.recorder,
    required this.options,
    required this.onChanged,
    required this.layouts,
    required this.onSaveLayout,
    required this.onLoadLayout,
    required this.onDeleteLayout,
  });
  final RecorderController recorder;
  final FeatureOptions options;
  final VoidCallback onChanged, onSaveLayout;
  final List<Map<String, dynamic>> layouts;
  final Future<void> Function(Map<String, dynamic>) onLoadLayout;
  final void Function(int) onDeleteLayout;
  @override
  State<CreatorStudio> createState() => _CreatorStudioState();
}

class _CreatorStudioState extends State<CreatorStudio> {
  int selected = 0;
  CreatorLayers get layers => widget.recorder.creatorLayers;
  bool get locked =>
      widget.recorder.active ||
      widget.recorder.scheduledAt != null ||
      layers.loading;
  Future<void> perform(Future<void> Function() operation) async {
    try {
      await operation();
      if (mounted) {
        setState(() {});
        widget.onChanged();
      }
    } catch (e) {
      if (mounted) widget.recorder.showError(e.toString());
    }
  }

  Future<void> text({OverlaySpec? existing, int? index}) async {
    final value = await showTextOverlayEditor(context, existing);
    if (value == null || !mounted) return;
    await perform(() async {
      if (index == null) {
        await layers.add(value);
        selected = layers.entries.length - 1;
      } else {
        await layers.update(index, value);
      }
      widget.options.layersEnabled = true;
    });
  }

  Future<void> logo() async {
    await perform(() async {
      final path = await RecorderController.host.invokeMethod<String>(
        'chooseOverlayImage',
      );
      if (path == null || !mounted) return;
      await layers.add(
        OverlaySpec(kind: 'image', content: path, width: .15, x: 1, y: 0),
      );
      selected = layers.entries.length - 1;
      widget.options.layersEnabled = true;
    });
  }

  @override
  Widget build(BuildContext context) {
    selected = selected.clamp(0, (layers.entries.length - 1).clamp(0, 3));
    final entry = layers.entries.isEmpty ? null : layers.entries[selected];
    void update(OverlaySpec spec) =>
        perform(() => layers.update(selected, spec));
    return ExpansionTile(
      title: const Text('Text, logos & creator layouts'),
      subtitle: const Text('Unicode titles, watermarks and reusable layouts'),
      childrenPadding: const EdgeInsets.all(16),
      children: [
        const Text(
          'Up to four cached overlays. Uses GPU composition when available, with CPU fallback. Prepare your layout before recording; logo files stay on your PC.',
        ),
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          title: const Text('Record text and logo layers'),
          value: widget.options.layersEnabled,
          onChanged: locked || layers.entries.isEmpty
              ? null
              : (v) {
                  widget.options.layersEnabled = v;
                  widget.onChanged();
                },
        ),
        Wrap(
          spacing: 8,
          children: [
            OutlinedButton.icon(
              onPressed: locked || layers.entries.length >= 4
                  ? null
                  : () => text(),
              icon: const Icon(Icons.title),
              label: const Text('Add text'),
            ),
            OutlinedButton.icon(
              onPressed: locked || layers.entries.length >= 4 ? null : logo,
              icon: const Icon(Icons.image_outlined),
              label: const Text('Add logo'),
            ),
          ],
        ),
        if (layers.loading)
          const Padding(
            padding: EdgeInsets.all(8),
            child: LinearProgressIndicator(),
          ),
        for (var i = 0; i < layers.entries.length; i++)
          ListTile(
            contentPadding: EdgeInsets.zero,
            selected: selected == i,
            leading: Icon(
              layers.entries[i].spec.kind == 'text'
                  ? Icons.title
                  : Icons.image_outlined,
            ),
            title: Text(
              layers.entries[i].spec.kind == 'text'
                  ? layers.entries[i].spec.content
                  : File(layers.entries[i].spec.content).uri.pathSegments.last,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
            ),
            subtitle: layers.entries[i].error == null
                ? null
                : Text(
                    layers.entries[i].error!,
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                  ),
            onTap: () => setState(() => selected = i),
            trailing: IconButton(
              tooltip: 'Remove layer',
              onPressed: locked
                  ? null
                  : () {
                      layers.remove(i);
                      widget.onChanged();
                      setState(() {});
                    },
              icon: const Icon(Icons.delete_outline),
            ),
          ),
        if (entry != null) ...[
          if (entry.spec.kind == 'text')
            TextButton.icon(
              onPressed: locked
                  ? null
                  : () => text(existing: entry.spec, index: selected),
              icon: const Icon(Icons.edit_outlined),
              label: const Text('Edit text & style'),
            ),
          Wrap(
            spacing: 6,
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
                      entry.spec.x == corner.$2 && entry.spec.y == corner.$3,
                  onSelected: locked
                      ? null
                      : (_) => update(
                          entry.spec.copyWith(x: corner.$2, y: corner.$3),
                        ),
                ),
            ],
          ),
          Text('Layer width: ${(entry.spec.width * 100).round()}%'),
          Slider(
            value: entry.spec.width,
            min: .02,
            max: 1,
            onChanged: locked
                ? null
                : (v) => update(entry.spec.copyWith(width: v)),
          ),
          const Text('Horizontal position'),
          Slider(
            value: entry.spec.x,
            onChanged: locked ? null : (v) => update(entry.spec.copyWith(x: v)),
          ),
          const Text('Vertical position'),
          Slider(
            value: entry.spec.y,
            onChanged: locked ? null : (v) => update(entry.spec.copyWith(y: v)),
          ),
          Text('Opacity: ${(entry.spec.opacity * 100).round()}%'),
          Slider(
            value: entry.spec.opacity,
            onChanged: locked
                ? null
                : (v) => update(entry.spec.copyWith(opacity: v)),
          ),
          Row(
            children: [
              TextButton.icon(
                onPressed: locked || selected == 0
                    ? null
                    : () {
                        layers.move(selected, selected - 1);
                        setState(() => selected--);
                        widget.onChanged();
                      },
                icon: const Icon(Icons.arrow_downward),
                label: const Text('Send backward'),
              ),
              TextButton.icon(
                onPressed: locked || selected == layers.entries.length - 1
                    ? null
                    : () {
                        layers.move(selected, selected + 1);
                        setState(() => selected++);
                        widget.onChanged();
                      },
                icon: const Icon(Icons.arrow_upward),
                label: const Text('Bring forward'),
              ),
            ],
          ),
        ],
        const Divider(height: 24),
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          title: const Text('Show keyboard shortcuts'),
          subtitle: const Text(
            'Ctrl/Alt/Win combinations and F1–F12 while recording. Ordinary typed text is excluded.',
          ),
          value: widget.options.shortcutOverlay,
          onChanged: locked
              ? null
              : (v) {
                  widget.options.shortcutOverlay = v;
                  widget.onChanged();
                },
        ),
        DropdownButtonFormField<int>(
          key: ValueKey('shortcut-${widget.options.shortcutSeconds}'),
          initialValue: widget.options.shortcutSeconds,
          decoration: const InputDecoration(
            labelText: 'Shortcut display duration',
          ),
          items: [
            for (final seconds in [1, 2, 3, 4, 5])
              DropdownMenuItem(value: seconds, child: Text('$seconds seconds')),
          ],
          onChanged: locked
              ? null
              : (v) {
                  widget.options.shortcutSeconds = v!;
                  widget.onChanged();
                },
        ),
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          title: const Text('Shortcut HUD on the right'),
          value: widget.options.shortcutRight,
          onChanged: locked
              ? null
              : (v) {
                  widget.options.shortcutRight = v;
                  widget.onChanged();
                },
        ),
        const Divider(height: 24),
        const Text(
          'Creator layouts save overlays, webcam/audio choices and quality. Choose your capture source and output file separately.',
        ),
        TextButton.icon(
          onPressed: locked ? null : widget.onSaveLayout,
          icon: const Icon(Icons.bookmark_add_outlined),
          label: const Text('Save creator layout'),
        ),
        for (var i = 0; i < widget.layouts.length; i++)
          ListTile(
            contentPadding: EdgeInsets.zero,
            title: Text(
              widget.layouts[i]['name'] as String,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
            ),
            onTap: locked
                ? null
                : () => perform(() => widget.onLoadLayout(widget.layouts[i])),
            trailing: IconButton(
              tooltip: 'Delete saved layout',
              onPressed: locked ? null : () => widget.onDeleteLayout(i),
              icon: const Icon(Icons.delete_outline),
            ),
          ),
      ],
    );
  }
}

Future<OverlaySpec?> showTextOverlayEditor(
  BuildContext context,
  OverlaySpec? existing,
) async {
  final controller = TextEditingController(
    text: existing?.content ?? 'Your title',
  );
  var spec = existing ?? const OverlaySpec(kind: 'text');
  String? error;
  try {
    return await showDialog<OverlaySpec>(
      context: context,
      builder: (context) => StatefulBuilder(
        builder: (context, update) => AlertDialog(
          title: const Text('Text overlay'),
          content: SizedBox(
            width: 420,
            child: SingleChildScrollView(
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  TextField(
                    controller: controller,
                    minLines: 2,
                    maxLines: 3,
                    maxLength: 256,
                    decoration: InputDecoration(
                      labelText: 'Text · Unicode supported',
                      errorText: error,
                    ),
                  ),
                  Text('Font size: ${spec.fontSize.round()}'),
                  Slider(
                    value: spec.fontSize,
                    min: 16,
                    max: 64,
                    divisions: 24,
                    onChanged: (v) =>
                        update(() => spec = spec.copyWith(fontSize: v)),
                  ),
                  Wrap(
                    spacing: 8,
                    children: [
                      for (final color in [
                        0xffffffff,
                        0xffffcc33,
                        0xff55eeaa,
                        0xffff5555,
                        0xff111111,
                      ])
                        ChoiceChip(
                          label: Icon(
                            Icons.circle,
                            color: Color(color),
                            size: 18,
                          ),
                          selected: spec.color == color,
                          onSelected: (_) =>
                              update(() => spec = spec.copyWith(color: color)),
                        ),
                    ],
                  ),
                  SwitchListTile(
                    contentPadding: EdgeInsets.zero,
                    title: const Text('Readable background'),
                    value: spec.plate,
                    onChanged: (v) =>
                        update(() => spec = spec.copyWith(plate: v)),
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
              onPressed: () {
                if (controller.text.trim().isEmpty ||
                    controller.text.runes.length > 256) {
                  update(() => error = 'Enter 1–256 Unicode characters');
                  return;
                }
                Navigator.pop(context, spec.copyWith(content: controller.text));
              },
              child: const Text('Apply'),
            ),
          ],
        ),
      ),
    );
  } finally {
    controller.dispose();
  }
}
