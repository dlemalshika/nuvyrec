import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:recorder_desktop/controller.dart';
import 'package:recorder_desktop/feature_options.dart';
import 'package:recorder_desktop/feature_settings.dart';

void main() {
  testWidgets(
    'performance controls fit a small window and lock during capture',
    (tester) async {
      tester.view.physicalSize = const Size(960, 640);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final recorder = RecorderController()
        ..machine = {
          'logical_cores': 2,
          'total_memory_mib': 4096,
          'recommended_height': 720,
          'recommended_fps': 24,
        };
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
      await tester.tap(find.text('Performance & PC compatibility'));
      await tester.pumpAndSettle();
      await tester.tap(find.byType(DropdownButtonFormField<String>).first);
      await tester.pumpAndSettle();
      await tester.tap(find.text('Low resource · minimal recording cost').last);
      await tester.pumpAndSettle();
      expect(options.performanceMode, 'eco');
      final restored = FeatureOptions()..restore(options.toJson());
      expect(restored.performanceMode, 'eco');
      change(() => recorder.status = {'state': 'recording'});
      await tester.pumpAndSettle();
      final field = tester.widget<DropdownButtonFormField<String>>(
        find.byType(DropdownButtonFormField<String>).first,
      );
      expect(field.onChanged, isNull);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
      recorder.dispose();
    },
  );
}
