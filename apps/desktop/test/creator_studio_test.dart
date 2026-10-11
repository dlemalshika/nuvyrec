import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:recorder_desktop/creator_layers.dart';
import 'package:recorder_desktop/creator_studio.dart';
import 'package:recorder_desktop/feature_options.dart';
import 'package:recorder_desktop/controller.dart';
import 'package:recorder_desktop/recorder_app.dart';
import 'widget_test.dart' show FakeRecorder;

void main() {
  test('descriptions reject invalid text and bound saved geometry', () {
    expect(
      () => OverlaySpec.fromJson({'kind': 'text', 'content': ''}),
      throwsFormatException,
    );
    expect(
      () => OverlaySpec.fromJson({'kind': 'text', 'content': 'x' * 257}),
      throwsFormatException,
    );
    final spec = OverlaySpec.fromJson({
      'kind': 'text',
      'content': 'ආයුබෝවන්',
      'x': 20,
      'width': -2,
      'font_size': 900,
    });
    expect(spec.x, 1);
    expect(spec.width, .02);
    expect(spec.fontSize, 64);
    final features = FeatureOptions()
      ..layersEnabled = true
      ..shortcutOverlay = true
      ..shortcutSeconds = 5
      ..shortcutRight = true;
    final restored = FeatureOptions()..restore(features.toJson());
    expect(restored.shortcutOverlay, true);
    expect(restored.shortcutSeconds, 5);
    expect(restored.shortcutRight, true);
  });
  testWidgets(
    'text raster is bounded, placement edits reuse pixels and missing logos block readiness',
    (tester) async {
      final layers = CreatorLayers();
      await tester.runAsync(
        () => layers.add(
          const OverlaySpec(
            kind: 'text',
            content: 'ආයුබෝවන් Frame',
            plate: false,
          ),
        ),
      );
      expect(layers.ready, true);
      final first = layers.entries.single.image!;
      final bytes = layers.entries.single.rgba!;
      expect(first.width, 768);
      expect(first.height, lessThanOrEqualTo(256));
      expect(bytes.where((v) => v != 0), isNotEmpty);
      await layers.update(
        0,
        layers.entries[0].spec.copyWith(x: 1, y: 0, opacity: .5),
      );
      expect(identical(layers.entries.single.image, first), true);
      expect(identical(layers.entries.single.rgba, bytes), true);
      await tester.runAsync(
        () => layers.add(
          const OverlaySpec(
            kind: 'image',
            content:
                'C:/screenrecorder/artifacts/nonexistent-logo-for-test.png',
          ),
        ),
      );
      expect(layers.ready, false);
      expect(layers.entries.last.error, isNotNull);
      layers.remove(1);
      expect(layers.ready, true);
      layers.dispose();
    },
  );
  testWidgets('creator settings fit 960px and lock while recording', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(960, 640);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final recorder = RecorderController();
    final options = FeatureOptions();
    Future<void> render() => tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: SingleChildScrollView(
            child: CreatorStudio(
              recorder: recorder,
              options: options,
              onChanged: () {},
              layouts: const [],
              onSaveLayout: () {},
              onLoadLayout: (_) async {},
              onDeleteLayout: (_) {},
            ),
          ),
        ),
      ),
    );
    await render();
    await tester.tap(find.text('Text, logos & creator layouts'));
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
    await tester.ensureVisible(find.text('Show keyboard shortcuts'));
    await tester.tap(find.text('Show keyboard shortcuts'));
    await tester.pump();
    expect(options.shortcutOverlay, true);
    recorder.status = {'state': 'recording'};
    await render();
    await tester.pumpAndSettle();
    final button = tester.widget<OutlinedButton>(
      find.widgetWithText(OutlinedButton, 'Add text'),
    );
    expect(button.onPressed, isNull);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
    recorder.dispose();
  });
  testWidgets(
    'creator layouts restore Unicode layers and preserve automatic encoding',
    (tester) async {
      tester.view.physicalSize = const Size(1280, 900);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        RecorderController.host,
        (call) async {
          if (call.method == 'getPreferences') {
            return '{"features":{"layers_enabled":true,"shortcut_overlay":true},"overlays":[{"kind":"text","content":"ආයුබෝවන් Frame"}]}';
          }
          return null;
        },
      );
      addTearDown(
        () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
          RecorderController.host,
          null,
        ),
      );
      final recorder = FakeRecorder();
      await tester.pumpWidget(RecorderApp(controller: recorder));
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 200)),
      );
      await tester.pumpAndSettle();
      expect(recorder.creatorLayers.ready, true);
      await tester.ensureVisible(find.text('Start recording'));
      await tester.tap(find.text('Start recording'));
      await tester.pump();
      expect(recorder.received?['layers_enabled'], true);
      expect(recorder.received?['shortcut_overlay'], true);
      expect(recorder.received?['encoder'], 'auto');
      await tester.pumpWidget(const SizedBox());
      recorder.dispose();
    },
  );
}
