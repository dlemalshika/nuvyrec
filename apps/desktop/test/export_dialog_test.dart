import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:recorder_desktop/media_jobs.dart';
import 'package:recorder_desktop/recording_library.dart';

void main() {
  for (final brightness in Brightness.values) {
    testWidgets('export presets fit 960 × 640 in ${brightness.name} theme', (
      tester,
    ) async {
      tester.view.physicalSize = const Size(960, 640);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final jobs = MediaJobs();
      addTearDown(jobs.dispose);
      await tester.pumpWidget(
        MaterialApp(
          theme: ThemeData(brightness: brightness),
          home: Builder(
            builder: (context) => Scaffold(
              body: TextButton(
                onPressed: () =>
                    showExportDialog(context, 'missing-recording.mp4', jobs),
                child: const Text('Open export'),
              ),
            ),
          ),
        ),
      );
      await tester.tap(find.text('Open export'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('MP4 remux · no quality loss'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Share / compress').last);
      await tester.pumpAndSettle();
      expect(find.text('YouTube / landscape'), findsOneWidget);
      await tester.tap(find.text('YouTube / landscape'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Vertical / stories').last);
      await tester.pumpAndSettle();
      expect(
        find.textContaining('black padding, no cropping.'),
        findsOneWidget,
      );
      await tester.runAsync(() async {
        await tester.tap(find.widgetWithText(FilledButton, 'Export'));
        await Future<void>.delayed(const Duration(milliseconds: 50));
      });
      await tester.pumpAndSettle();
      await tester.ensureVisible(
        find.textContaining('Recording file is missing'),
      );
      expect(find.textContaining('Recording file is missing'), findsOneWidget);
      expect(tester.takeException(), isNull);
      await tester.tap(find.widgetWithText(TextButton, 'Cancel'));
      await tester.pumpAndSettle();
      expect(find.text('Export recording'), findsNothing);
    });
  }
}
