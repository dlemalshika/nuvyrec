import 'dart:async';
import 'dart:typed_data';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:recorder_bridge/recorder_bridge.dart';
import 'package:recorder_desktop/controller.dart';
import 'package:recorder_desktop/webcam_preview.dart';
import 'package:recorder_desktop/recorder_app.dart';
import 'widget_test.dart' show FakeRecorder;

class CameraFixture extends FakeRecorder {
  int requests = 0, stops = 0;
  bool fail = false;
  Completer<WebcamPreviewFrame?>? pending;
  @override
  Future<WebcamPreviewFrame?> webcamPreviewFrame(
    Map<String, dynamic> webcam,
    bool lowResource,
  ) async {
    requests++;
    if (fail) throw StateError('Disconnected');
    if (pending != null) return pending!.future;
    return WebcamPreviewFrame(
      2,
      2,
      Uint8List.fromList(
        List.generate(
          4,
          (_) => requests.isOdd ? [255, 0, 0, 255] : [0, 0, 255, 255],
        ).expand((p) => p).toList(),
      ),
    );
  }

  @override
  void stopWebcamPreview() {
    stops++;
  }
}

void main() {
  testWidgets(
    'camera frames replace live image; off, lifecycle and error clear it',
    (tester) async {
      final recorder = CameraFixture();
      Widget preview(bool enabled, {bool eco = false}) => MaterialApp(
        home: Scaffold(
          body: WebcamPreview(
            recorder: recorder,
            webcam: const {'enabled': true, 'device': 'fixture', 'width': .25},
            enabled: enabled,
            lowResource: eco,
            canvas: const Rect.fromLTWH(0, 0, 640, 360),
          ),
        ),
      );
      await tester.pumpWidget(preview(true));
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 30)),
      );
      await tester.pump();
      expect(find.byKey(const Key('live-webcam-image')), findsOneWidget);
      final first = tester
          .widget<RawImage>(find.byKey(const Key('live-webcam-image')))
          .image;
      await tester.pump(const Duration(milliseconds: 200));
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 30)),
      );
      await tester.pump();
      expect(recorder.requests, 2);
      await tester.pump();
      expect(
        tester
            .widget<RawImage>(find.byKey(const Key('live-webcam-image')))
            .image,
        isNot(same(first)),
      );
      recorder.fail = true;
      await tester.pump(const Duration(milliseconds: 200));
      expect(find.text('Camera unavailable · check Settings'), findsOneWidget);
      expect(find.byKey(const Key('live-webcam-image')), findsNothing);
      await tester.pumpWidget(preview(false));
      final requests = recorder.requests;
      await tester.pump(const Duration(seconds: 1));
      expect(recorder.requests, requests);
      expect(recorder.stops, greaterThan(0));
      recorder.fail = false;
      await tester.pumpWidget(preview(true, eco: true));
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 30)),
      );
      await tester.pump();
      final beforeEcoTick = recorder.requests;
      await tester.pump(const Duration(milliseconds: 400));
      expect(recorder.requests, beforeEcoTick);
      await tester.pump(const Duration(milliseconds: 100));
      await tester.runAsync(
        () => Future<void>.delayed(const Duration(milliseconds: 30)),
      );
      await tester.pump();
      expect(recorder.requests, beforeEcoTick + 1);
      tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.hidden);
      await tester.pump();
      final hiddenRequests = recorder.requests;
      await tester.pump(const Duration(seconds: 1));
      expect(recorder.requests, hiddenRequests);
      expect(find.byKey(const Key('live-webcam-image')), findsNothing);
      tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.resumed);
      await tester.pumpWidget(const SizedBox());
      recorder.dispose();
    },
  );
  testWidgets('pending frame cannot reappear after webcam off', (tester) async {
    final recorder = CameraFixture()
      ..pending = Completer<WebcamPreviewFrame?>();
    Widget preview(bool enabled) => MaterialApp(
      home: WebcamPreview(
        recorder: recorder,
        webcam: const {'enabled': true, 'device': 'fixture'},
        enabled: enabled,
        canvas: const Rect.fromLTWH(0, 0, 640, 360),
      ),
    );
    await tester.pumpWidget(preview(true));
    await tester.pump(const Duration(seconds: 1));
    expect(recorder.requests, 1);
    await tester.pumpWidget(preview(false));
    recorder.pending!.complete(
      WebcamPreviewFrame(2, 2, Uint8List.fromList(List.filled(16, 255))),
    );
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 30)),
    );
    await tester.pump();
    expect(find.byKey(const Key('live-webcam-image')), findsNothing);
    await tester.pumpWidget(const SizedBox());
    recorder.dispose();
  });
  testWidgets('main webcam toggle persists and controls an enabled recording', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(960, 640);
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
    final recorder = CameraFixture()
      ..cameras = [
        {'id': 'fixture', 'name': 'Fixture camera'},
      ];
    await tester.pumpWidget(RecorderApp(controller: recorder));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('workspace-webcam-toggle')));
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 30)),
    );
    await tester.pump(const Duration(milliseconds: 350));
    expect(find.text('Webcam On'), findsOneWidget);
    expect(
      (recorder.preferences['features'] as Map)['webcam']['enabled'],
      true,
    );
    recorder.status = {
      'state': 'recording',
      'webcam_available': true,
      'webcam_visible': true,
    };
    recorder.notifyLibraryChanged();
    await tester.pump();
    await tester.tap(find.byKey(const Key('workspace-webcam-toggle')));
    await tester.pump();
    expect(recorder.status['webcam_visible'], false);
    expect(find.text('Webcam Off'), findsOneWidget);
    await tester.tap(find.byKey(const Key('workspace-webcam-toggle')));
    await tester.pump();
    expect(recorder.status['webcam_visible'], true);
    recorder.status = {'state': 'recording', 'webcam_available': false};
    recorder.notifyLibraryChanged();
    await tester.pump();
    expect(
      tester
          .widget<TextButton>(find.byKey(const Key('workspace-webcam-toggle')))
          .onPressed,
      isNull,
    );
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
    recorder.dispose();
  });
}
