import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:recorder_desktop/controller.dart';
import 'package:recorder_desktop/creator_layers.dart';
import 'package:recorder_desktop/feature_options.dart';
import 'package:recorder_desktop/scene_studio.dart';

void main() {
  testWidgets(
    'a missing live-scene asset preserves the previous usable UI layout',
    (tester) async {
      final recorder = RecorderController();
      await tester.runAsync(
        () => recorder.creatorLayers.add(
          const OverlaySpec(kind: 'text', content: 'ආයුබෝවන් Frame'),
        ),
      );
      final previous = recorder.creatorLayers.entries.single.image;
      await tester.runAsync(() async {
        await expectLater(
          recorder.applyScene(
            overlays: [
              const OverlaySpec(
                kind: 'image',
                content: 'C:/screenrecorder/artifacts/missing-scene-logo.png',
              ).toJson(),
            ],
            webcam: FeatureOptions().webcam,
            layersEnabled: true,
          ),
          throwsStateError,
        );
      });
      expect(recorder.creatorLayers.ready, true);
      expect(
        identical(recorder.creatorLayers.entries.single.image, previous),
        true,
      );
      expect(recorder.sceneBusy, false);
      recorder.dispose();
    },
  );

  for (final brightness in Brightness.values) {
    testWidgets(
      'scene drafts fit 960x640 in ${brightness.name} and apply only visual settings',
      (tester) async {
        tester.view.physicalSize = const Size(960, 640);
        tester.view.devicePixelRatio = 1;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);
        final recorder = RecorderController();
        final options = FeatureOptions()
          ..performanceMode = 'eco'
          ..systemGain = .6;
        final scenes = <Map<String, dynamic>>[
          {
            'name': 'Clean scene',
            'overlays': [],
            'layers_enabled': false,
            'webcam': FeatureOptions().webcam,
          },
        ];
        await tester.pumpWidget(
          MaterialApp(
            theme: ThemeData(brightness: brightness),
            home: Scaffold(
              body: SceneStudio(
                recorder: recorder,
                options: options,
                scenes: scenes,
                onChanged: () {},
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
        await tester.tap(find.text('Clean scene'));
        await tester.pumpAndSettle();
        expect(find.text('Draft: Clean scene'), findsOneWidget);
        expect(options.layersEnabled, false);
        expect(options.performanceMode, 'eco');
        await tester.tap(find.text('Apply layout'));
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
        expect(options.systemGain, .6);
        expect(options.performanceMode, 'eco');
        recorder.status = {'state': 'recording', 'scenes_available': false};
        recorder.notifyLibraryChanged();
        await tester.pumpAndSettle();
        expect(
          tester
              .widget<FilledButton>(
                find.widgetWithText(FilledButton, 'Take live'),
              )
              .onPressed,
          isNull,
        );
        recorder.status = {'state': 'recording', 'scenes_available': true};
        recorder.notifyLibraryChanged();
        await tester.pumpAndSettle();
        expect(
          tester
              .widget<FilledButton>(
                find.widgetWithText(FilledButton, 'Take live'),
              )
              .onPressed,
          isNotNull,
        );
        recorder.status['state'] = 'paused';
        recorder.notifyLibraryChanged();
        await tester.pumpAndSettle();
        expect(
          tester
              .widget<FilledButton>(
                find.widgetWithText(FilledButton, 'Take live'),
              )
              .onPressed,
          isNull,
        );
        await tester.pumpWidget(const SizedBox());
        recorder.dispose();
      },
    );
  }
}
