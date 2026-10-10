import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:recorder_desktop/controller.dart';
import 'package:recorder_desktop/feature_options.dart';
import 'package:recorder_desktop/feature_settings.dart';

void main() {
  testWidgets(
    'camera selection and key controls persist and lock while recording',
    (tester) async {
      tester.view.physicalSize = const Size(960, 640);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final recorder = RecorderController()
        ..cameras = [
          {'id': 'fixture-camera', 'name': 'Test camera'},
        ];
      final options = FeatureOptions();
      late StateSetter change;
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: StatefulBuilder(
              builder: (context, setState) {
                change = setState;
                return SingleChildScrollView(
                  child: FeatureSettings(
                    options: options,
                    recorder: recorder,
                    onChanged: () => setState(() {}),
                    onPreset: (_) {},
                    onSavePreset: () {},
                    savedPresets: const [],
                    onSchedule: () {},
                  ),
                );
              },
            ),
          ),
        ),
      );
      await tester.tap(find.text('Webcam overlay & green screen'));
      await tester.pumpAndSettle();
      await tester.tap(find.byType(DropdownButtonFormField<String>).first);
      await tester.pumpAndSettle();
      await tester.tap(find.text('Test camera').last);
      await tester.pumpAndSettle();
      await tester.ensureVisible(find.text('Record webcam overlay'));
      await tester.tap(find.text('Record webcam overlay'));
      await tester.pumpAndSettle();
      await tester.ensureVisible(find.text('Remove green background'));
      await tester.tap(find.text('Remove green background'));
      await tester.pumpAndSettle();
      final restored = FeatureOptions()..restore(options.toJson());
      expect(restored.webcamEnabled, isTrue);
      expect(restored.webcamKey, isTrue);
      expect(restored.webcamDevice, 'fixture-camera');
      change(() => recorder.status = {'state': 'recording'});
      await tester.pumpAndSettle();
      final switches = tester.widgetList<SwitchListTile>(
        find.byType(SwitchListTile),
      );
      expect(switches.every((s) => s.onChanged == null), isTrue);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
      recorder.dispose();
    },
  );
}
