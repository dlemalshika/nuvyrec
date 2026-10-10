/// Explicit integration-test mode. Captures only this app's synthetic window,
/// never another window or the desktop; does not load/save user preferences.
library;

import 'dart:convert';
import 'dart:io';
import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'package:recorder_bridge/recorder_bridge.dart';
import 'controller.dart';
import 'creator_layers.dart';

class CreatorSmoke extends StatefulWidget {
  const CreatorSmoke({super.key, required this.directory});
  final String directory;
  @override
  State<CreatorSmoke> createState() => _CreatorSmokeState();
}

class _CreatorSmokeState extends State<CreatorSmoke> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) => run());
  }

  Future<void> run() async {
    final directory = Directory(widget.directory);
    final layers = CreatorLayers();
    NativeRecorder? native;
    var succeeded = false;
    try {
      await directory.create(recursive: true);
      final recorder = ui.PictureRecorder();
      final canvas = Canvas(recorder);
      canvas.drawCircle(
        const Offset(64, 32),
        28,
        Paint()..color = const Color(0xffffcc33),
      );
      final picture = recorder.endRecording();
      final logo = await picture.toImage(128, 64);
      picture.dispose();
      final png = await logo.toByteData(format: ui.ImageByteFormat.png);
      logo.dispose();
      final logoFile = File('${directory.path}/generated-logo.png');
      await logoFile.writeAsBytes(
        png!.buffer.asUint8List(png.offsetInBytes, png.lengthInBytes),
        flush: true,
      );
      await layers.restore([
        const OverlaySpec(
          kind: 'text',
          content: 'ආයුබෝවන් · Frame Tutorial',
          x: .1,
          y: .85,
          width: .5,
        ).toJson(),
        OverlaySpec(
          kind: 'image',
          content: logoFile.path,
          x: .85,
          y: .1,
          width: .15,
        ).toJson(),
      ]);
      if (!layers.ready) throw StateError('Fixture layers failed to render');
      native = NativeRecorder.open();
      native.command({'action': 'clear_layers'});
      final uploads = <Map<String, dynamic>>[];
      for (var i = 0; i < layers.entries.length; i++) {
        final entry = layers.entries[i];
        final image = entry.image!;
        final meta = <String, dynamic>{
          'id': i,
          'pixel_width': image.width,
          'pixel_height': image.height,
          'x': entry.spec.x,
          'y': entry.spec.y,
          'width': entry.spec.width,
          'opacity': entry.spec.opacity,
        };
        native.uploadLayer(meta, entry.rgba!);
        final bytesFile = File('${directory.path}/layer-$i.rgba');
        await bytesFile.writeAsBytes(entry.rgba!, flush: true);
        final encoded = await image.toByteData(format: ui.ImageByteFormat.png);
        await File('${directory.path}/layer-$i.png').writeAsBytes(
          encoded!.buffer.asUint8List(
            encoded.offsetInBytes,
            encoded.lengthInBytes,
          ),
          flush: true,
        );
        uploads.add({
          ...meta,
          'rgba_file': bytesFile.path,
          'content': entry.spec.content,
        });
      }
      final hwnd = await RecorderController.host.invokeMethod<int>(
        'creatorFixtureHandle',
      );
      final sources = (native.sources()['sources'] as List).whereType<Map>();
      final source = sources.singleWhere(
        (s) => s['kind'] == 'window' && s['id'] == hwnd,
      );
      final output = File('${directory.path}/creator-window.mkv');
      native.command({
        'action': 'start',
        'config': {
          'output': output.path,
          'container': 'mkv',
          'source_id': source['id'],
          'source_kind': 'window',
          'width': 640,
          'height': 360,
          'fps': 24,
          'bitrate': 2000000,
          'encoder': 'libx264',
          'layers_enabled': true,
          'capture_cursor': false,
          'max_seconds': 3,
          'performance_mode': 'eco',
        },
      });
      final deadline = DateTime.now().add(const Duration(seconds: 20));
      Map<String, dynamic> status = {};
      while (DateTime.now().isBefore(deadline)) {
        status = native.status();
        if (status['state'] == 'failed') {
          throw StateError(status['error'] as String);
        }
        if (status['state'] == 'stopped') break;
        await Future<void>.delayed(const Duration(milliseconds: 100));
      }
      if (status['state'] != 'stopped' ||
          (status['frames'] as num? ?? 0) < 24) {
        throw StateError('Owned-window recording did not complete');
      }
      await File('${directory.path}/result.json').writeAsString(
        jsonEncode({
          'ok': true,
          'output': output.path,
          'frames': status['frames'],
          'encoder': status['encoder'],
          'width': status['effective_width'],
          'height': status['effective_height'],
          'fps': status['effective_fps'],
          'uploads': uploads,
        }),
        flush: true,
      );
      succeeded = true;
    } catch (e) {
      await directory.create(recursive: true);
      await File('${directory.path}/result.json').writeAsString(
        jsonEncode({'ok': false, 'error': e.toString()}),
        flush: true,
      );
    } finally {
      if (native != null) {
        try {
          native.command({'action': 'stop'});
        } catch (_) {
          // Auto-stop may already have finished its worker before this poll.
        }
        final deadline = DateTime.now().add(const Duration(seconds: 10));
        while ([
              'preparing',
              'recording',
              'paused',
              'stopping',
            ].contains(native.status()['state']) &&
            DateTime.now().isBefore(deadline)) {
          await Future<void>.delayed(const Duration(milliseconds: 100));
        }
      }
      layers.dispose();
      await RecorderController.host.invokeMethod<void>('finishCreatorFixture', {
        'ok': succeeded,
      });
    }
  }

  @override
  Widget build(BuildContext context) => const MaterialApp(
    debugShowCheckedModeBanner: false,
    home: Scaffold(
      body: Row(
        children: [
          Expanded(
            child: ColoredBox(
              color: Color(0xffe01616),
              child: SizedBox.expand(),
            ),
          ),
          Expanded(
            child: ColoredBox(
              color: Color(0xff165de0),
              child: SizedBox.expand(),
            ),
          ),
        ],
      ),
    ),
  );
}
