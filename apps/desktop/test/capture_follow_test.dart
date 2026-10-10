import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:recorder_desktop/capture_preview.dart';
import 'package:recorder_desktop/controller.dart';
import 'package:recorder_desktop/feature_options.dart';
import 'package:recorder_desktop/recorder_app.dart';
import 'widget_test.dart' show FakeRecorder;

void main() {
  testWidgets('closed selected window never falls back to another app', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(1280, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      RecorderController.host,
      (call) async => null,
    );
    addTearDown(
      () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        RecorderController.host,
        null,
      ),
    );
    final recorder = FakeRecorder();
    for (final entry in [(2, 'Fixture A'), (3, 'Fixture B')]) {
      recorder.sources.add({
        'id': entry.$1,
        'name': entry.$2,
        'kind': 'window',
        'width': 640,
        'height': 400,
      });
    }
    await tester.pumpWidget(RecorderApp(controller: recorder));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Window'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Fixture A'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Fixture B').last);
    await tester.pumpAndSettle();
    recorder.sources.removeWhere((s) => s['id'] == 3);
    recorder.notifyLibraryChanged();
    await tester.pumpAndSettle();
    await tester.ensureVisible(find.text('Start recording'));
    await tester.tap(find.text('Start recording'));
    await tester.pump();
    expect(recorder.received, isNull);
    final startButton = tester.widget<FilledButton>(
      find.ancestor(
        of: find.text('Start recording'),
        matching: find.byType(FilledButton),
      ),
    );
    expect(startButton.onPressed, isNull);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
    recorder.dispose();
  });
  test('follow mouse preference survives restore', () {
    final options = FeatureOptions()..followMouse = true;
    expect((FeatureOptions()..restore(options.toJson())).followMouse, isTrue);
  });
  testWidgets('moving coordinates keep preview requests limited to the timer', (
    tester,
  ) async {
    var calls = 0;
    tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      RecorderController.host,
      (call) async {
        if (call.method == 'capturePreview') calls++;
        return null;
      },
    );
    addTearDown(
      () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        RecorderController.host,
        null,
      ),
    );
    Future<void> render(int left) => tester.pumpWidget(
      MaterialApp(
        home: CapturePreview(
          enabled: true,
          source: {
            'id': 1,
            'kind': 'display',
            'left': left,
            'top': 0,
            'width': 640,
            'height': 360,
            'frame_width': 640,
            'frame_height': 360,
          },
        ),
      ),
    );
    await render(0);
    await tester.pump();
    for (var i = 1; i < 10; i++) {
      await render(i * 10);
      await tester.pump(const Duration(milliseconds: 50));
    }
    expect(calls, 1);
    await tester.pump(const Duration(milliseconds: 600));
    expect(calls, 2);
    await tester.pumpWidget(const SizedBox());
  });
  testWidgets(
    'region follow flag is sent to the core and disabled for display mode',
    (tester) async {
      tester.view.physicalSize = const Size(1280, 900);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        RecorderController.host,
        (call) async => null,
      );
      addTearDown(
        () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
          RecorderController.host,
          null,
        ),
      );
      final recorder = FakeRecorder();
      await tester.pumpWidget(RecorderApp(controller: recorder));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Region'));
      await tester.pumpAndSettle();
      await tester.ensureVisible(find.text('Follow mouse'));
      await tester.tap(find.text('Follow mouse'));
      await tester.pumpAndSettle();
      await tester.ensureVisible(find.text('Start recording'));
      await tester.tap(find.text('Start recording'));
      await tester.pump();
      expect(recorder.received?['follow_mouse'], true);
      expect(recorder.received?['capture_cursor'], true);
      expect(recorder.received?['region'], [0, 0, 640, 360]);
      recorder.action('stop');
      await tester.pump();
      await tester.ensureVisible(find.text('Display'));
      await tester.tap(find.text('Display'));
      await tester.pumpAndSettle();
      await tester.ensureVisible(find.text('Start recording'));
      await tester.tap(find.text('Start recording'));
      await tester.pump();
      expect(recorder.received?['follow_mouse'], false);
      await tester.pumpWidget(const SizedBox());
      recorder.dispose();
    },
  );
}
