import 'dart:io';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:recorder_desktop/media_jobs.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  final sdk = Directory(
    '../../.deps/ffmpeg/ffmpeg-8.1.2-full_build-shared/bin',
  ).absolute.path;
  test(
    'local exports preserve tracks, trim/crop accurately and never overwrite',
    () async {
      final temporary = await Directory.systemTemp.createTemp(
        'frame-export-test-',
      );
      final jobs = MediaJobs(toolsDirectory: sdk);
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(
            const MethodChannel('org.screenrecorder/host'),
            (_) async => null,
          );
      try {
        final input = '${temporary.path}/input.mkv';
        final generated = await Process.run('$sdk/ffmpeg.exe', [
          '-v',
          'error',
          '-f',
          'lavfi',
          '-i',
          'testsrc2=size=160x90:rate=15',
          '-f',
          'lavfi',
          '-i',
          'sine=frequency=440:sample_rate=48000',
          '-t',
          '2',
          '-c:v',
          'libx264',
          '-threads',
          '2',
          '-c:a',
          'aac',
          input,
        ]);
        expect(generated.exitCode, 0, reason: '${generated.stderr}');
        final remux = '${temporary.path}/remux.mp4';
        await jobs.export(input, remux);
        final info = await jobs.probe(remux);
        expect((info['streams'] as List).length, 2);
        final bytes = await File(remux).readAsBytes();
        await expectLater(jobs.export(input, remux), throwsStateError);
        expect(await File(remux).readAsBytes(), bytes);
        final cropped = '${temporary.path}/crop.mp4';
        await jobs.export(
          input,
          cropped,
          mode: 'crop',
          start: .4,
          duration: 1,
          crop: [0, 0, 80, 40],
        );
        final result = await jobs.probe(cropped);
        final video = (result['streams'] as List).cast<Map>().firstWhere(
          (s) => s['codec_type'] == 'video',
        );
        expect(video['width'], 80);
        expect(video['height'], 40);
        expect(
          double.parse((result['format'] as Map)['duration'] as String),
          closeTo(1, .15),
        );
        final decoded = await Process.run('$sdk/ffmpeg.exe', [
          '-v',
          'error',
          '-i',
          cropped,
          '-map',
          '0',
          '-f',
          'null',
          '-',
        ]);
        expect(decoded.exitCode, 0, reason: '${decoded.stderr}');
        expect(jobs.busy, false);
        expect(jobs.status, 'Export saved');
      } finally {
        jobs.dispose();
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
            .setMockMethodCallHandler(
              const MethodChannel('org.screenrecorder/host'),
              null,
            );
        await temporary.delete(recursive: true);
      }
    },
    skip: !Platform.isWindows || !File('$sdk/ffmpeg.exe').existsSync(),
  );
}
