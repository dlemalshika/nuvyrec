import 'package:flutter/material.dart';
import 'app_updates.dart';
import 'controller.dart';

Future<void> openUpdateRelease(UpdateController updates) async {
  final release = updates.available;
  if (release == null) return;
  await RecorderController.host.invokeMethod<void>('openReleasePage', {
    'url': release.url.toString(),
  });
}

class UpdatePanel extends StatefulWidget {
  const UpdatePanel({super.key, required this.updates});
  final UpdateController updates;
  @override
  State<UpdatePanel> createState() => _UpdatePanelState();
}

class _UpdatePanelState extends State<UpdatePanel> {
  late final TextEditingController repository;
  String? formError;
  @override
  void initState() {
    super.initState();
    repository = TextEditingController(text: widget.updates.repository);
  }

  @override
  void dispose() {
    repository.dispose();
    super.dispose();
  }

  void configure({bool? beta, bool? auto}) {
    try {
      widget.updates.configure(
        repository.text,
        beta ?? widget.updates.beta,
        auto ?? widget.updates.automatic,
      );
      setState(() => formError = null);
    } catch (e) {
      setState(() => formError = e.toString());
    }
  }

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: widget.updates,
    builder: (context, _) {
      final updates = widget.updates, release = widget.updates.available;
      return Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text('Installed version: ${updates.currentVersion ?? 'Unavailable'}'),
          const SizedBox(height: 12),
          TextField(
            controller: repository,
            autocorrect: false,
            decoration: InputDecoration(
              labelText: 'Public GitHub repository',
              hintText: 'owner/repository',
              errorText: formError,
            ),
          ),
          Align(
            alignment: Alignment.centerLeft,
            child: TextButton(
              onPressed: updates.checking ? null : configure,
              child: const Text('Save update source'),
            ),
          ),
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            title: const Text('Automatic update checks'),
            subtitle: const Text(
              'Check once a day while Nuvyrec is open and idle. No account or token needed.',
            ),
            value: updates.automatic,
            onChanged: updates.checking ? null : (v) => configure(auto: v),
          ),
          DropdownButtonFormField<bool>(
            key: ValueKey(updates.beta),
            initialValue: updates.beta,
            decoration: const InputDecoration(labelText: 'Update channel'),
            items: const [
              DropdownMenuItem(value: false, child: Text('Stable')),
              DropdownMenuItem(
                value: true,
                child: Text('Beta · includes prereleases'),
              ),
            ],
            onChanged: updates.checking ? null : (v) => configure(beta: v),
          ),
          const SizedBox(height: 12),
          OutlinedButton.icon(
            onPressed: updates.checking
                ? null
                : () async {
                    try {
                      updates.configure(
                        repository.text,
                        updates.beta,
                        updates.automatic,
                      );
                      setState(() => formError = null);
                      await updates.check(manual: true);
                    } catch (e) {
                      if (mounted) setState(() => formError = e.toString());
                    }
                  },
            icon: const Icon(Icons.refresh),
            label: Text(updates.checking ? 'Checking…' : 'Check now'),
          ),
          if (updates.checking) const LinearProgressIndicator(),
          if (updates.lastCheck != null)
            Text(
              'Last checked: ${updates.lastCheck!.toLocal().toString().split('.').first}',
            ),
          if (updates.error != null)
            Text(
              updates.error!,
              style: TextStyle(color: Theme.of(context).colorScheme.error),
            ),
          if (updates.message != null) Text(updates.message!),
          if (release != null) ...[
            const Divider(),
            Text(
              'Available: ${release.tag}',
              style: Theme.of(context).textTheme.titleMedium,
            ),
            if (release.tag == updates.skipped)
              const Text(
                'This version is skipped for automatic notifications.',
              ),
            if (release.notes.isNotEmpty)
              Padding(
                padding: const EdgeInsets.symmetric(vertical: 8),
                child: SelectableText(release.notes),
              ),
            FilledButton.icon(
              onPressed: () async {
                try {
                  await openUpdateRelease(updates);
                } catch (_) {
                  if (mounted) {
                    setState(
                      () =>
                          formError = 'Could not open your browser. Try again.',
                    );
                  }
                }
              },
              icon: const Icon(Icons.open_in_new),
              label: Text(
                release.hasWindowsBundle
                    ? 'Download on GitHub'
                    : 'View release on GitHub',
              ),
            ),
            if (!release.hasWindowsBundle)
              const Text(
                'No recognized Windows bundle is attached to this release yet.',
              ),
            Wrap(
              spacing: 8,
              children: [
                TextButton(
                  onPressed: updates.later,
                  child: const Text('Remind tomorrow'),
                ),
                TextButton(
                  onPressed: updates.skip,
                  child: const Text('Skip this version'),
                ),
              ],
            ),
          ],
          const SizedBox(height: 8),
          const Text(
            'Downloads open in your browser. Stop recording and exit Nuvyrec before replacing its bundle. This version does not install updates or restart automatically.',
          ),
        ],
      );
    },
  );
}

class UpdateNotice extends StatelessWidget {
  const UpdateNotice({
    super.key,
    required this.updates,
    required this.onDetails,
  });
  final UpdateController updates;
  final VoidCallback onDetails;
  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: updates,
    builder: (context, _) {
      if (!updates.notifyUpdate) return const SizedBox.shrink();
      return Card(
        child: Padding(
          padding: const EdgeInsets.all(12),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                'Update ${updates.available!.tag} available',
                style: Theme.of(context).textTheme.titleSmall,
              ),
              Wrap(
                spacing: 4,
                children: [
                  TextButton(
                    onPressed: onDetails,
                    child: const Text('View update'),
                  ),
                  TextButton(
                    onPressed: updates.later,
                    child: const Text('Later'),
                  ),
                ],
              ),
            ],
          ),
        ),
      );
    },
  );
}
