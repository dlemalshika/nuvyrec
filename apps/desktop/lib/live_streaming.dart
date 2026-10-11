import 'package:flutter/material.dart';
import 'controller.dart';

/// In-memory only: intentionally excluded from preferences and creator layouts.
class LiveDestination {
  LiveDestination(this.platform, this.name);
  final String platform, name;
  bool enabled = false;
  String server = '', key = '';

  String? validate() {
    final uri = Uri.tryParse(server.trim());
    if (uri == null ||
        !['rtmp', 'rtmps'].contains(uri.scheme) ||
        uri.host.isEmpty ||
        uri.userInfo.isNotEmpty ||
        uri.hasQuery ||
        uri.hasFragment ||
        server.length > 4096 ||
        key.isEmpty ||
        key.length > 2048 ||
        RegExp(r'[\s\x00-\x1f\x7f]').hasMatch(server + key)) {
      return '$name: enter a valid RTMP/RTMPS server and stream key.';
    }
    return null;
  }

  Map<String, dynamic> toRequest() => {
    'platform': platform,
    'server': server.trim(),
    'key': key,
  };
}

class LiveStreamingSetup extends StatefulWidget {
  const LiveStreamingSetup({
    super.key,
    required this.destinations,
    required this.recorder,
    required this.onStart,
  });
  final List<LiveDestination> destinations;
  final RecorderController recorder;
  final VoidCallback onStart;
  @override
  State<LiveStreamingSetup> createState() => _LiveStreamingSetupState();
}

class _LiveStreamingSetupState extends State<LiveStreamingSetup> {
  late final servers = widget.destinations
      .map((d) => TextEditingController(text: d.server))
      .toList();
  late final keys = widget.destinations
      .map((d) => TextEditingController(text: d.key))
      .toList();
  String? error;
  @override
  void dispose() {
    for (final controller in [...servers, ...keys]) {
      controller.dispose();
    }
    super.dispose();
  }

  void _start() {
    final targets = widget.destinations.where((d) => d.enabled).toList();
    String? problem = targets.isEmpty ? 'Select at least one platform.' : null;
    for (final target in targets) {
      problem ??= target.validate();
    }
    if (problem != null) {
      setState(() => error = problem);
      return;
    }
    Navigator.pop(context);
    widget.onStart();
  }

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: widget.recorder,
    builder: (context, _) {
      final recorder = widget.recorder;
      final statuses = (recorder.status['streaming'] as List? ?? [])
          .whereType<Map>()
          .toList();
      return AlertDialog(
        title: const Text('Live streaming'),
        content: SizedBox(
          width: 560,
          child: SingleChildScrollView(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                const Text(
                  'One shared picture on up to three platforms. Encode once, '
                  'keep a local recording, reconnect each destination independently.',
                ),
                const SizedBox(height: 12),
                const Text(
                  'Live preset: up to 720p / 30 fps / 4 Mbps. Eco mode can lower this. '
                  'Three destinations need roughly 13–15 Mbps of stable upload. '
                  'All receive the same aspect ratio and mixed audio.',
                ),
                const SizedBox(height: 8),
                const Text(
                  'Get each server and key from the platform’s LIVE dashboard. '
                  'TikTok must give your account encoder access. Check the platform '
                  'preview and press its Go Live button if required. Keys stay in '
                  'memory for this app session and are never saved to settings.',
                ),
                for (
                  var index = 0;
                  index < widget.destinations.length;
                  index++
                ) ...[
                  const SizedBox(height: 16),
                  CheckboxListTile(
                    contentPadding: EdgeInsets.zero,
                    title: Text(widget.destinations[index].name),
                    value: widget.destinations[index].enabled,
                    onChanged: recorder.active
                        ? null
                        : (value) => setState(
                            () => widget.destinations[index].enabled = value!,
                          ),
                  ),
                  if (widget.destinations[index].enabled) ...[
                    TextField(
                      controller: servers[index],
                      enabled: !recorder.active,
                      maxLength: 4096,
                      autocorrect: false,
                      enableSuggestions: false,
                      decoration: const InputDecoration(
                        labelText: 'Server URL',
                        hintText: 'rtmps://your-platform-server/live',
                        counterText: '',
                      ),
                      onChanged: (v) => widget.destinations[index].server = v,
                    ),
                    const SizedBox(height: 8),
                    TextField(
                      controller: keys[index],
                      enabled: !recorder.active,
                      obscureText: true,
                      maxLength: 2048,
                      autocorrect: false,
                      enableSuggestions: false,
                      decoration: const InputDecoration(
                        labelText: 'Stream key',
                        counterText: '',
                      ),
                      onChanged: (v) => widget.destinations[index].key = v,
                    ),
                  ],
                ],
                if (statuses.isNotEmpty) ...[
                  const Divider(height: 28),
                  const Text('Destination health'),
                  for (final status in statuses)
                    ListTile(
                      contentPadding: EdgeInsets.zero,
                      leading: Icon(
                        status['state'] == 'sending'
                            ? Icons.sensors
                            : Icons.sync,
                        color: status['state'] == 'sending'
                            ? Colors.green
                            : Colors.orange,
                      ),
                      title: Text('${status['platform']} · ${status['state']}'),
                      subtitle: Text(
                        '${status['packets'] ?? 0} packets · '
                        '${status['reconnects'] ?? 0} reconnects\n'
                        '${status['error'] ?? 'Check the platform dashboard for broadcast status.'}',
                      ),
                    ),
                ],
                if (error != null)
                  Padding(
                    padding: const EdgeInsets.only(top: 12),
                    child: Text(
                      error!,
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
            child: const Text('Close'),
          ),
          if (recorder.status['streaming_active'] == true)
            OutlinedButton(
              onPressed: () => recorder.action('stop_streaming'),
              child: const Text('Stop live · keep recording'),
            ),
          FilledButton.icon(
            onPressed: recorder.active || recorder.scheduledAt != null
                ? null
                : _start,
            icon: const Icon(Icons.sensors),
            label: const Text('Start live + record'),
          ),
        ],
      );
    },
  );
}
