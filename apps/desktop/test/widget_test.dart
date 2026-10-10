// This is a basic Flutter widget test.
//
// To perform an interaction with a widget in your test, use the WidgetTester
// utility in the flutter_test package. For example, you can send tap and scroll
// gestures. You can also use WidgetTester to find child widgets in the widget
// tree, read text, and verify that the values of widget properties are correct.

import 'package:flutter/material.dart';
import 'dart:io';
import 'dart:ui' as ui;
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:recorder_desktop/recorder_app.dart';
import 'package:recorder_desktop/controller.dart';
import 'package:recorder_desktop/workspace_view.dart';

class FakeRecorder extends RecorderController {
  Map<String, dynamic>? received;
  FakeRecorder() {
    sources = [
      {
        'id': 1,
        'kind': 'display',
        'name': 'Test display',
        'width': 1920,
        'height': 1080,
      },
    ];
    output = 'new-recording.mkv';
  }
  @override
  void start(Map<String, dynamic> config) {
    received = config;
    status = {
      'state': 'recording',
      'frames': 30,
      'seconds': 1,
      'encoder': 'libx264',
    };
    notifyListeners();
  }

  @override
  void action(String action) {
    status['state'] = action == 'pause'
        ? 'paused'
        : action == 'resume'
        ? 'recording'
        : 'stopped';
    notifyListeners();
  }
}

void main() {
  testWidgets(
    'start, pause, resume and stop dispatch valid native configuration',
    (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 720);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final recorder = FakeRecorder();
      Uint8List? preview;
      String savedTheme = 'dark';
      final previewCalls = <Map<Object?, Object?>>[];
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        RecorderController.host,
        (call) async {
          if (call.method == 'getTheme') return savedTheme;
          if (call.method == 'setTheme') {
            savedTheme = (call.arguments as Map)['theme'] as String;
            return null;
          }
          if (call.method == 'capturePreview') {
            previewCalls.add(Map<Object?, Object?>.from(call.arguments as Map));
            return preview;
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
      await tester.runAsync(() async {
        // A generated fixture avoids capturing or exposing real desktop content.
        final picture = ui.PictureRecorder();
        final canvas = Canvas(picture);
        canvas.drawRect(
          const Rect.fromLTWH(0, 0, 480, 270),
          Paint()..color = const Color(0xffe8eff5),
        );
        canvas.drawRect(
          const Rect.fromLTWH(0, 0, 480, 25),
          Paint()..color = const Color(0xff263a51),
        );
        for (var i = 0; i < 3; i++) {
          canvas.drawCircle(
            Offset(13 + i * 14, 12),
            3,
            Paint()
              ..color = [Colors.redAccent, Colors.amber, Colors.greenAccent][i],
          );
        }
        canvas.drawRect(
          const Rect.fromLTWH(0, 25, 90, 245),
          Paint()..color = const Color(0xffd7e2ec),
        );
        for (var i = 0; i < 5; i++) {
          canvas.drawRRect(
            RRect.fromRectAndRadius(
              Rect.fromLTWH(14, 48 + i * 22, 62, 7),
              const Radius.circular(3),
            ),
            Paint()..color = const Color(0xff92a5b8),
          );
        }
        canvas.drawRRect(
          RRect.fromRectAndRadius(
            const Rect.fromLTWH(114, 49, 204, 12),
            const Radius.circular(4),
          ),
          Paint()..color = const Color(0xff344b61),
        );
        for (var i = 0; i < 3; i++) {
          canvas.drawRRect(
            RRect.fromRectAndRadius(
              Rect.fromLTWH(114 + i * 110, 86, 94, 140),
              const Radius.circular(8),
            ),
            Paint()..color = Colors.white,
          );
          canvas.drawRRect(
            RRect.fromRectAndRadius(
              Rect.fromLTWH(125 + i * 110, 104, 70, 48),
              const Radius.circular(6),
            ),
            Paint()
              ..color = [
                const Color(0xff9ddfcd),
                const Color(0xffa8c7f0),
                const Color(0xffc8b5e9),
              ][i],
          );
          for (var j = 0; j < 3; j++) {
            canvas.drawRect(
              Rect.fromLTWH(125 + i * 110, 168 + j * 13, j == 2 ? 40 : 66, 5),
              Paint()..color = const Color(0xffd4dfe8),
            );
          }
        }
        final fixturePicture = picture.endRecording();
        final fixture = await fixturePicture.toImage(480, 270);
        preview = (await fixture.toByteData(
          format: ui.ImageByteFormat.png,
        ))!.buffer.asUint8List();
        fixture.dispose();
        fixturePicture.dispose();
        final windowsDirectory = Platform.environment['WINDIR'];
        if (windowsDirectory != null) {
          final font = File('$windowsDirectory/Fonts/segoeui.ttf');
          if (await font.exists()) {
            final loader = FontLoader('Segoe UI')
              ..addFont(
                Future.value(ByteData.sublistView(await font.readAsBytes())),
              );
            await loader.load();
          }
        }
        final icons = File(
          'build/windows/x64/runner/Release/data/flutter_assets/fonts/MaterialIcons-Regular.otf',
        );
        if (await icons.exists()) {
          final loader = FontLoader('MaterialIcons')
            ..addFont(
              Future.value(ByteData.sublistView(await icons.readAsBytes())),
            );
          await loader.load();
        }
      });
      final boundary = GlobalKey();
      await tester.pumpWidget(
        RepaintBoundary(
          key: boundary,
          child: RecorderApp(controller: recorder),
        ),
      );
      await tester.pumpAndSettle();
      await tester.runAsync(
        () => precacheImage(MemoryImage(preview!), boundary.currentContext!),
      );
      await tester.pumpAndSettle();
      expect(previewCalls.single['id'], 1);
      expect(find.byType(Image), findsOneWidget);
      expect(find.text('Maximum resolution'), findsNothing);
      await tester.runAsync(() async {
        final image =
            await (boundary.currentContext!.findRenderObject()
                    as RenderRepaintBoundary)
                .toImage(pixelRatio: 1);
        final bytes = await image.toByteData(format: ui.ImageByteFormat.png);
        final directory = Directory('../../artifacts');
        await directory.create(recursive: true);
        await File(
          '${directory.path}/desktop-ui.png',
        ).writeAsBytes(bytes!.buffer.asUint8List());
        image.dispose();
      });
      await tester.tap(find.text('Settings'));
      await tester.pumpAndSettle();
      expect(find.text('Recording settings'), findsOneWidget);
      expect(find.text('Maximum resolution'), findsOneWidget);
      await tester.tap(find.text('Light'));
      await tester.pumpAndSettle();
      expect(savedTheme, 'light');
      expect(
        Theme.of(tester.element(find.byType(RecorderWorkspace))).brightness,
        Brightness.light,
      );
      expect(
        Theme.of(tester.element(find.byType(Dialog))).brightness,
        Brightness.light,
      );
      await tester.tap(find.text('30 fps'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('60 fps').last);
      await tester.pumpAndSettle();
      await tester.runAsync(() async {
        final image =
            await (boundary.currentContext!.findRenderObject()
                    as RenderRepaintBoundary)
                .toImage(pixelRatio: 1);
        final bytes = await image.toByteData(format: ui.ImageByteFormat.png);
        await File(
          '../../artifacts/desktop-settings.png',
        ).writeAsBytes(bytes!.buffer.asUint8List());
        image.dispose();
      });
      await tester.tap(find.text('Done'));
      await tester.pumpAndSettle();
      await tester.runAsync(() async {
        final image =
            await (boundary.currentContext!.findRenderObject()
                    as RenderRepaintBoundary)
                .toImage(pixelRatio: 1);
        final bytes = await image.toByteData(format: ui.ImageByteFormat.png);
        await File(
          '../../artifacts/desktop-ui-light.png',
        ).writeAsBytes(bytes!.buffer.asUint8List());
        image.dispose();
      });
      await tester.tap(find.byTooltip('Turn preview off'));
      await tester.pump();
      final count = previewCalls.length;
      await tester.pump(const Duration(seconds: 2));
      expect(previewCalls.length, count);
      expect(find.text('Preview is off'), findsOneWidget);
      await tester.tap(find.byTooltip('Turn preview on'));
      await tester.pump();
      await tester.tap(find.text('Region'));
      await tester.pumpAndSettle();
      expect(previewCalls.last['left'], 0);
      expect(previewCalls.last['width'], 640);
      expect(previewCalls.last['height'], 360);
      await tester.tap(find.text('Display'));
      await tester.pumpAndSettle();
      await tester.ensureVisible(find.text('Start recording'));
      await tester.tap(find.text('Start recording'));
      await tester.pump();
      expect(recorder.received?['source_id'], 1);
      expect(recorder.received?['width'], 1920);
      expect(recorder.received?['height'], 1080);
      expect(recorder.received?['container'], 'mkv');
      expect(recorder.received?['fps'], 60);
      await tester.tap(find.text('Settings'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Dark'));
      await tester.pumpAndSettle();
      expect(recorder.state, 'recording');
      await tester.tap(find.text('Light'));
      await tester.pumpAndSettle();
      expect(recorder.state, 'recording');
      await tester.tap(find.text('Done'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Pause'));
      await tester.pump();
      expect(find.text('Resume'), findsOneWidget);
      await tester.tap(find.text('Resume'));
      await tester.pump();
      await tester.tap(find.text('Stop & save'));
      await tester.pump();
      expect(recorder.state, 'stopped');
      expect(tester.takeException(), isNull);
      tester.view.physicalSize = const Size(960, 640);
      await tester.pumpAndSettle();
      expect(find.text('Start recording'), findsOneWidget);
      expect(tester.takeException(), isNull);
      await tester.pumpWidget(const SizedBox());
      await tester.pumpWidget(RecorderApp(controller: recorder));
      await tester.pumpAndSettle();
      expect(
        Theme.of(tester.element(find.byType(RecorderWorkspace))).brightness,
        Brightness.light,
      );
      await tester.tap(find.text('Settings'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Dark'));
      await tester.pumpAndSettle();
      expect(savedTheme, 'dark');
      expect(
        Theme.of(tester.element(find.byType(RecorderWorkspace))).brightness,
        Brightness.dark,
      );
      await tester.tap(find.text('Done'));
      await tester.pumpAndSettle();
      await tester.pumpWidget(const SizedBox());
      recorder.dispose();
    },
  );
}
