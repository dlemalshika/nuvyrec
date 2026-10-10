import 'dart:io';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:recorder_desktop/media_jobs.dart';
import 'package:recorder_desktop/export_presets.dart';

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
          'testsrc2=size=1600x900:rate=60',
          '-f',
          'lavfi',
          '-i',
          'sine=frequency=440:sample_rate=48000',
          '-f',
          'lavfi',
          '-i',
          'sine=frequency=880:sample_rate=48000',
          '-map',
          '0:v',
          '-map',
          '1:a',
          '-map',
          '2:a',
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
        expect((info['streams'] as List).length, 3);
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
        for (final preset in ExportPreset.values.where(
          (p) => p != ExportPreset.original,
        )) {
          final audio = switch (preset) {
            ExportPreset.youtube => ExportAudio.all,
            ExportPreset.square => ExportAudio.muted,
            _ => ExportAudio.first,
          };
          final output = '${temporary.path}/${preset.name}.mp4';
          await jobs.export(
            input,
            output,
            mode: 'preset',
            preset: preset,
            audio: audio,
            start: .2,
            duration: .3,
          );
          final encoded = await jobs.probe(output);
          final streams = (encoded['streams'] as List).cast<Map>();
          final v = streams.firstWhere((s) => s['codec_type'] == 'video');
          final expectedSize = switch (preset) {
            ExportPreset.youtube => [1600, 900],
            ExportPreset.vertical => [1080, 1920],
            ExportPreset.square => [1080, 1080],
            _ => [1280, 720],
          };
          expect([v['width'], v['height']], expectedSize);
          expect(v['avg_frame_rate'], '30/1');
          expect(v['sample_aspect_ratio'], '1:1');
          expect(v['pix_fmt'], 'yuv420p');
          expect(
            streams.where((s) => s['codec_type'] == 'audio').length,
            switch (audio) {
              ExportAudio.all => 2,
              ExportAudio.first => 1,
              ExportAudio.muted => 0,
            },
          );
          final checked = await Process.run('$sdk/ffmpeg.exe', [
            '-v',
            'error',
            '-i',
            output,
            '-map',
            '0',
            '-f',
            'null',
            '-',
          ]);
          expect(checked.exitCode, 0, reason: '${checked.stderr}');
          if (preset.padded) {
            final padding = await Process.run('$sdk/ffmpeg.exe', [
              '-v',
              'error',
              '-i',
              output,
              '-frames:v',
              '1',
              '-vf',
              'crop=2:2:0:0,format=rgb24',
              '-f',
              'rawvideo',
              'pipe:1',
            ], stdoutEncoding: null);
            expect(padding.exitCode, 0, reason: '${padding.stderr}');
            expect((padding.stdout as List<int>).length, 12);
            expect(
              (padding.stdout as List<int>).every((channel) => channel <= 5),
              true,
              reason: 'Social canvas must pad rather than stretch content',
            );
          }
        }
        final smallInput = '${temporary.path}/small.mkv';
        final smallSource = await Process.run('$sdk/ffmpeg.exe', [
          '-v',
          'error',
          '-i',
          input,
          '-t',
          '0.4',
          '-vf',
          'scale=160:90,fps=15',
          '-an',
          '-c:v',
          'libx264',
          '-threads',
          '2',
          smallInput,
        ]);
        expect(smallSource.exitCode, 0, reason: '${smallSource.stderr}');
        final smallOutput = '${temporary.path}/small.mp4';
        await jobs.export(
          smallInput,
          smallOutput,
          mode: 'preset',
          preset: ExportPreset.compact,
        );
        final smallInfo = await jobs.probe(smallOutput);
        final smallVideo = (smallInfo['streams'] as List).cast<Map>().first;
        expect([smallVideo['width'], smallVideo['height']], [160, 90]);
        expect(smallVideo['avg_frame_rate'], '15/1');
        await expectLater(
          jobs.export(input, '${temporary.path}/invalid.mp4', start: 3),
          throwsArgumentError,
        );
        await expectLater(
          jobs.export(
            input,
            '${temporary.path}/invalid.mp4',
            mode: 'remux',
            preset: ExportPreset.compact,
          ),
          throwsArgumentError,
        );
        expect(await File('${temporary.path}/invalid.mp4').exists(), false);
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
