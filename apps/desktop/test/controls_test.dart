import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:recorder_desktop/controller.dart';
import 'package:recorder_desktop/annotation_preview.dart';

class TestRecorder extends RecorderController {
  int starts = 0;
  List<dynamic> annotations = [];
  @override
  void start(Map<String, dynamic> config) {
    starts++;
    status = {'state': 'recording'};
    notifyListeners();
  }

  @override
  void updateAnnotations(List<Map<String, dynamic>> items) {
    annotations = jsonDecode(jsonEncode(items)) as List;
  }
}

void main() {
  testWidgets('countdown cancellation never starts a delayed recording', (
    tester,
  ) async {
    final recorder = TestRecorder();
    recorder.begin({}, 3);
    await tester.pump(const Duration(seconds: 1));
    expect(recorder.countdown, 2);
    recorder.action('stop');
    await tester.pump(const Duration(seconds: 5));
    expect(recorder.starts, 0);
    recorder.begin({}, 3);
    await tester.pump(const Duration(seconds: 3));
    expect(recorder.starts, 1);
    expect(recorder.state, 'recording');
    recorder.dispose();
  });
  testWidgets(
    'preview arrows use normalized recording coordinates and clear live',
    (tester) async {
      final recorder = TestRecorder()
        ..status = {'state': 'recording', 'annotations_enabled': true};
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        RecorderController.host,
        (_) async => null,
      );
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SizedBox(
              width: 640,
              height: 400,
              child: AnnotationPreview(
                recorder: recorder,
                source: const {
                  'id': 1,
                  'kind': 'display',
                  'frame_width': 1920,
                  'frame_height': 1080,
                },
                enabled: false,
              ),
            ),
          ),
        ),
      );
      await tester.tap(find.byTooltip('arrow'));
      await tester.pump();
      final gesture = find.byType(GestureDetector).last;
      await tester.dragFrom(tester.getCenter(gesture), const Offset(90, 50));
      await tester.pump(const Duration(milliseconds: 60));
      expect(recorder.annotations.single['kind'], 'arrow');
      final points = recorder.annotations.single['points'] as List;
      expect(points.length, 2);
      for (final point in points) {
        for (final value in point as List) {
          expect(value, inInclusiveRange(0, 1));
        }
      }
      await tester.tap(find.byTooltip('Clear annotations'));
      await tester.pump(const Duration(milliseconds: 60));
      expect(recorder.annotations, isEmpty);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
      recorder.dispose();
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        RecorderController.host,
        null,
      );
    },
  );
}
