import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:recorder_desktop/controller.dart';
import 'package:recorder_desktop/live_streaming.dart';
import 'package:recorder_desktop/recorder_app.dart';

class _LiveRecorder extends RecorderController {
  _LiveRecorder() {
    sources = [
      {
        'id': 1,
        'kind': 'display',
        'name': 'Generated source',
        'width': 3840,
        'height': 2160,
      },
    ];
    output = 'new-live-recording.mkv';
  }
  Map<String, dynamic>? requested;
  @override
  void begin(Map<String, dynamic> config, int delay) {
    requested = config;
    status = {'state': 'recording', 'streaming_active': true};
    notifyListeners();
  }
}

void main() {
  testWidgets(
    'workspace dispatches three destinations with a continuous bounded live profile',
    (tester) async {
      tester.view.physicalSize = const Size(960, 640);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        RecorderController.host,
        (call) async => call.method == 'getTheme' ? 'dark' : null,
      );
      addTearDown(
        () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
          RecorderController.host,
          null,
        ),
      );
      final recorder = _LiveRecorder();
      await tester.pumpWidget(RecorderApp(controller: recorder));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Live'));
      await tester.pumpAndSettle();
      for (var i = 0; i < 3; i++) {
        final check = find.byType(CheckboxListTile).at(i);
        await tester.ensureVisible(check);
        await tester.tap(check);
        await tester.pumpAndSettle();
        final fields = find.byType(TextField);
        await tester.ensureVisible(fields.at(i * 2));
        await tester.enterText(
          fields.at(i * 2),
          'rtmps://example.invalid/live',
        );
        await tester.ensureVisible(fields.at(i * 2 + 1));
        await tester.enterText(fields.at(i * 2 + 1), 'private-key-$i');
      }
      await tester.tap(find.text('Start live + record'));
      await tester.pumpAndSettle();
      final config = recorder.requested!;
      expect(config['streaming'], hasLength(3));
      expect(config['width'], 1280);
      expect(config['height'], 720);
      expect(config['fps'], 30);
      expect(config['bitrate'], 4000000);
      expect(config['mix_audio'], true);
      expect(config['adaptive_quality'], false);
      expect(config['segment_seconds'], 0);
      expect(find.text('Pause'), findsNothing);
      expect(recorder.preferences.toString(), isNot(contains('private-key')));
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
      recorder.dispose();
    },
  );
  test('RTMP credentials reject unsafe schemes without exposing secrets', () {
    final destination = LiveDestination('youtube', 'YouTube')
      ..server = 'rtmps://example.invalid/live'
      ..key = 'private-key';
    expect(destination.validate(), isNull);
    for (final server in [
      'file:///tmp/stream',
      'https://example.invalid',
      'rtmp://user:private-key@host/live',
      'rtmp://host/live?private-key',
      'rtmp://host/live timeout=1',
    ]) {
      destination.server = server;
      expect(destination.validate(), isNotNull);
      expect(destination.validate(), isNot(contains('private-key')));
    }
  });

  testWidgets(
    'keys are masked and live requires a valid selected destination',
    (tester) async {
      tester.view.physicalSize = const Size(960, 640);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final recorder = RecorderController();
      final target = LiveDestination('youtube', 'YouTube');
      var started = 0;
      await tester.pumpWidget(
        MaterialApp(
          home: Builder(
            builder: (context) => TextButton(
              onPressed: () => showDialog<void>(
                context: context,
                builder: (_) => LiveStreamingSetup(
                  destinations: [target],
                  recorder: recorder,
                  onStart: () => started++,
                ),
              ),
              child: const Text('Open live'),
            ),
          ),
        ),
      );
      await tester.tap(find.text('Open live'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Start live + record'));
      await tester.pumpAndSettle();
      expect(started, 0);
      expect(find.text('Select at least one platform.'), findsOneWidget);
      await tester.ensureVisible(find.byType(CheckboxListTile));
      await tester.tap(find.byType(CheckboxListTile));
      await tester.pumpAndSettle();
      final fields = find.byType(TextField);
      expect(tester.widget<TextField>(fields.at(1)).obscureText, true);
      await tester.ensureVisible(fields.at(0));
      await tester.enterText(fields.at(0), 'rtmps://example.invalid/live');
      await tester.ensureVisible(fields.at(1));
      await tester.enterText(fields.at(1), 'private-key');
      await tester.ensureVisible(find.text('Start live + record'));
      await tester.tap(find.text('Start live + record'));
      await tester.pumpAndSettle();
      expect(started, 1);
      expect(target.toRequest()['key'], 'private-key');
      expect(recorder.preferences, isEmpty);
      await tester.pumpWidget(const SizedBox());
      recorder.dispose();
    },
  );
}
